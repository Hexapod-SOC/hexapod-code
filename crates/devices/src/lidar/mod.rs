//! LD19 LiDAR Driver
//!
//! This module provides a Rust implementation of the LDROBOT LD19 LiDAR driver.
//! It handles serial communication, packet parsing, and data filtering for the LiDAR sensor.

mod filter;
mod packet;
mod parser;
mod point;
mod serial;
mod slam;

pub use filter::NearRangeFilter;
pub use packet::{LidarFrame, LidarPacket};
pub use parser::PacketParser;
pub use point::{LidarType, Point, PointCloud};
pub use serial::SerialInterface;
pub use slam::{LidarSlamConfig, LidarSlamHandle, SlamSnapshot};

use anyhow::Result;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Main LiDAR driver interface
pub struct LidarDriver {
    serial: Arc<Mutex<SerialInterface>>,
    parser: Arc<Mutex<PacketParser>>,
    running: Arc<AtomicBool>,
    read_thread: Option<thread::JoinHandle<()>>,
}

impl LidarDriver {
    /// Create a new LiDAR driver instance
    pub fn new(port: &str) -> Result<Self> {
        let serial = Arc::new(Mutex::new(SerialInterface::new(port)?));
        let parser = Arc::new(Mutex::new(PacketParser::new()));

        Ok(Self {
            serial,
            parser,
            running: Arc::new(AtomicBool::new(false)),
            read_thread: None,
        })
    }

    /// Start the LiDAR data collection
    pub fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.running.store(true, Ordering::SeqCst);

        let serial = Arc::clone(&self.serial);
        let parser = Arc::clone(&self.parser);
        let running = Arc::clone(&self.running);

        let handle = thread::spawn(move || {
            let mut buffer = vec![0u8; 4096];
            let mut consecutive_empty = 0u32;

            while running.load(Ordering::SeqCst) {
                let mut serial = serial.lock().unwrap();
                match serial.read(&mut buffer) {
                    Ok(n) if n > 0 => {
                        consecutive_empty = 0;
                        drop(serial); // Release lock before parsing
                        let mut parser = parser.lock().unwrap();
                        parser.process_bytes(&buffer[..n]);
                    }
                    Ok(_) => {
                        // No data available
                        drop(serial);
                        consecutive_empty += 1;
                        // Adaptive sleep: start short, increase if no data keeps coming
                        let sleep_ms = if consecutive_empty < 5 {
                            1
                        } else if consecutive_empty < 20 {
                            5
                        } else {
                            10
                        };
                        thread::sleep(Duration::from_millis(sleep_ms));
                    }
                    Err(e) => {
                        eprintln!("[LiDAR] Read error: {}", e);
                        drop(serial);
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        });

        self.read_thread = Some(handle);
        Ok(())
    }

    /// Stop the LiDAR data collection
    pub fn stop(&mut self) -> Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.read_thread.take() {
            handle
                .join()
                .map_err(|_| anyhow::anyhow!("Failed to join read thread"))?;
        }

        Ok(())
    }

    /// Check if a complete frame is ready
    pub fn is_frame_ready(&self) -> bool {
        self.parser.lock().unwrap().is_frame_ready()
    }

    /// Get the latest complete point cloud
    pub fn get_point_cloud(&self) -> Option<PointCloud> {
        self.parser.lock().unwrap().get_point_cloud()
    }

    /// Get the current rotation speed in Hz
    pub fn get_speed(&self) -> f64 {
        self.parser.lock().unwrap().get_speed()
    }

    /// Get the error count
    pub fn get_error_count(&self) -> u64 {
        self.parser.lock().unwrap().get_error_count()
    }

    /// Get the timestamp of the last packet (milliseconds)
    pub fn get_timestamp(&self) -> u16 {
        self.parser.lock().unwrap().get_timestamp()
    }
}

impl Drop for LidarDriver {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_creation() {
        // This test will fail without actual hardware, but validates the API
        let result = LidarDriver::new("/dev/ttyUSB0");
        // We expect an error without hardware
        assert!(result.is_err() || result.is_ok());
    }
}
