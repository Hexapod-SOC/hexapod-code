use super::SerialInterface;
use anyhow::Result;
use lidar_slam::{LaserScan, LidarSlam as SlamPipeline, OccupancyGrid, Pose2D, PoseDelta, SlamParams};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Configuration for the threaded SLAM runner.
#[derive(Clone)]
pub struct LidarSlamConfig {
    pub port: String,
    pub read_buffer_len: usize,
    pub idle_sleep: Duration,
    pub slam_params: SlamParams,
}

impl Default for LidarSlamConfig {
    fn default() -> Self {
        Self {
            port: "/dev/ttyUSB0".into(),
            read_buffer_len: 4096,
            idle_sleep: Duration::from_millis(5),
            slam_params: SlamParams::default(),
        }
    }
}

/// Snapshot of the latest SLAM results.
#[derive(Clone, Default)]
pub struct SlamSnapshot {
    pub frame: u64,
    pub pose: Pose2D,
    pub timestamp_ns: u64,
    pub map: Option<Arc<OccupancyGrid>>,
    pub last_scan: Option<Arc<LaserScan>>,
}

impl SlamSnapshot {
    pub fn has_map(&self) -> bool {
        self.map.is_some()
    }
}

/// Handle to the background SLAM ingestion thread.
pub struct LidarSlamHandle {
    serial: Arc<Mutex<SerialInterface>>,
    state: Arc<RwLock<SlamSnapshot>>,
    odometry: Arc<RwLock<Option<PoseDelta>>>,
    running: Arc<AtomicBool>,
    join_handle: Mutex<Option<JoinHandle<()>>>,
}

impl LidarSlamHandle {
    pub fn new(config: LidarSlamConfig) -> Result<Self> {
        let serial = Arc::new(Mutex::new(SerialInterface::new(&config.port)?));
        let state = Arc::new(RwLock::new(SlamSnapshot::default()));
        let odometry = Arc::new(RwLock::new(None));
        let running = Arc::new(AtomicBool::new(true));

        let serial_thread = Arc::clone(&serial);
        let state_thread = Arc::clone(&state);
        let odom_thread = Arc::clone(&odometry);
        let running_thread = Arc::clone(&running);
        let buffer_len = config.read_buffer_len.max(1024);
        let idle_sleep = config.idle_sleep;
        let params = config.slam_params.clone();

        let join_handle = thread::Builder::new()
            .name("lidar-slam".into())
            .spawn(move || {
                let mut slam = SlamPipeline::new(params);
                let mut buffer = vec![0u8; buffer_len];
                let mut frame_counter = 0u64;

                while running_thread.load(Ordering::SeqCst) {
                    let read_bytes = {
                        let mut serial = match serial_thread.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => {
                                eprintln!("[LiDAR SLAM] Serial mutex poisoned");
                                poisoned.into_inner()
                            }
                        };
                        match serial.read(&mut buffer) {
                            Ok(n) => n,
                            Err(err) => {
                                eprintln!("[LiDAR SLAM] serial read error: {err:?}");
                                thread::sleep(Duration::from_millis(50));
                                continue;
                            }
                        }
                    };

                    if read_bytes == 0 {
                        thread::sleep(idle_sleep);
                        continue;
                    }

                    let odom_delta = {
                        let mut guard = match odom_thread.write() {
                            Ok(g) => g,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        guard.take()
                    };

                    match slam.ingest(&buffer[..read_bytes], odom_delta) {
                        Ok(updates) => {
                            if updates.is_empty() {
                                continue;
                            }

                            for (pose, scan) in updates {
                                frame_counter += 1;
                                let timestamp_ns = scan.timestamp_ns;
                                let scan_arc = Arc::new(scan);
                                let map_arc = Arc::new(slam.map().clone());
                                let snapshot = SlamSnapshot {
                                    frame: frame_counter,
                                    pose,
                                    timestamp_ns,
                                    map: Some(map_arc),
                                    last_scan: Some(scan_arc),
                                };
                                let mut writer = match state_thread.write() {
                                    Ok(w) => w,
                                    Err(poisoned) => poisoned.into_inner(),
                                };
                                *writer = snapshot;
                            }
                        }
                        Err(err) => {
                            eprintln!("[LiDAR SLAM] packet parse error: {err:?}");
                        }
                    }
                }
            })?;

        Ok(Self {
            serial,
            state,
            odometry,
            running,
            join_handle: Mutex::new(Some(join_handle)),
        })
    }

    /// Retrieve the latest SLAM snapshot.
    pub fn latest(&self) -> SlamSnapshot {
        self.state.read().map(|r| r.clone()).unwrap_or_default()
    }

    /// Provide an odometry delta that will be applied to the next SLAM update.
    pub fn update_odometry(&self, delta: PoseDelta) {
        if let Ok(mut guard) = self.odometry.write() {
            *guard = Some(delta);
        }
    }

    /// Stop the background thread and close the serial port.
    pub fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.join_handle.lock() {
            if let Some(handle) = guard.take() {
                handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("Failed to join SLAM thread"))?;
            }
        }
        Ok(())
    }

    /// Access to the underlying serial interface (advanced use).
    pub fn serial(&self) -> Arc<Mutex<SerialInterface>> {
        Arc::clone(&self.serial)
    }
}

impl Drop for LidarSlamHandle {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
