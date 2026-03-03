#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod main_utils;
pub mod api;
pub mod config;
pub mod demos;
pub mod hexapod;

use main_utils::*;
use crate::hexapod::{LegStances, ServoAngleTriplet, ServoAngleTweaks};
use audio::tts;
use config::{
    calibration_gait_configs_path, calibration_leg_stance_path, calibration_servo_tweaks_path,
    BATTERY_LOG_SERVER, CONSTRAINTS, LOAD_SAVED_SERVO_TWEAKS, SERVO_OFFSETS, SERVO_PINS, TMP_DIR, TTS_URL,
};
use glam::Vec3;
use hexmath::{GaitConfig, GaitType};
use lidar_slam::PoseDelta;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;


#[tokio::main]
async fn main() {
    println!("╔═══════════════════╗");
    println!("║   HEXAPOD ROBOT   ║");
    println!("╚═══════════════════╝\n");

    // Initialize text-to-speech
    println!("Initializing TTS...");
    tts::init(TTS_URL, TMP_DIR);
    tts::cleanup_cache(7).unwrap();
    tts::sayen("Hexapod initializing...").unwrap();

    // Create hexapod controller with ripple gait
    println!("Creating hexapod controller...");
    // Load saved default stance if available
    let saved_stance = load_saved_leg_stance();

    let mut hexapod = hexapod::Hexapod::new(
        SERVO_PINS,
        SERVO_OFFSETS,
        CONSTRAINTS,
        GaitType::Tripod, // Tripod gait
        saved_stance, // Use saved stance if present
    );

    let mut lidar_error: Option<String> = None;
    let lidar_handle = if config::LIDAR_SLAM_ENABLE {
        match devices::lidar::LidarSlamHandle::new(config::lidar_slam_config()) {
            Ok(handle) => {
                println!("LiDAR SLAM thread started on {}", config::LIDAR_SERIAL_PORT);
                Some(Arc::new(handle))
            }
            Err(err) => {
                let message = format!("Failed to start LiDAR SLAM: {err:?}");
                eprintln!("{message}");
                lidar_error = Some(message);
                None
            }
        }
    } else {
        lidar_error = Some("LiDAR SLAM disabled by config".to_string());
        None
    };

    if config::LIDAR_SLAM_ENABLE && config::LIDAR_IMU_FUSION_ENABLE {
        if let (Some(lidar), Some(imu)) = (lidar_handle.clone(), hexapod.get_imu()) {
            tokio::spawn(async move {
                let mut last_yaw: Option<f32> = None;
                let mut last_error = tokio::time::Instant::now()
                    .checked_sub(tokio::time::Duration::from_secs(5))
                    .unwrap_or_else(tokio::time::Instant::now);
                let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(
                    config::LIDAR_IMU_POLL_MS,
                ));
                loop {
                    interval.tick().await;
                    let yaw_deg = {
                        let mut imu = imu.lock().await;
                        match imu.read_data() {
                            Ok(data) if data.calibration > 0 => Some(data.euler.z),
                            Ok(_) => None,
                            Err(e) => {
                                if last_error.elapsed()
                                    >= tokio::time::Duration::from_secs(2)
                                {
                                    eprintln!("[LiDAR SLAM] IMU read error: {e}");
                                    last_error = tokio::time::Instant::now();
                                }
                                None
                            }
                        }
                    };

                    if let Some(yaw_deg) = yaw_deg {
                        let yaw_rad = yaw_deg.to_radians();
                        if let Some(prev) = last_yaw {
                            let mut dtheta = yaw_rad - prev;
                            if dtheta > std::f32::consts::PI {
                                dtheta -= 2.0 * std::f32::consts::PI;
                            } else if dtheta < -std::f32::consts::PI {
                                dtheta += 2.0 * std::f32::consts::PI;
                            }
                            dtheta = dtheta.clamp(
                                -config::LIDAR_IMU_MAX_DTHETA_RAD,
                                config::LIDAR_IMU_MAX_DTHETA_RAD,
                            );
                            lidar.update_odometry(PoseDelta {
                                forward: 0.0,
                                sideways: 0.0,
                                dtheta,
                            });
                        }
                        lidar.update_heading(yaw_rad);
                        last_yaw = Some(yaw_rad);
                    }
                }
            });
        }
    }

    let gps_controller = if config::GPS_ENABLE {
        let gps_port =
            std::env::var("GPS_PORT").unwrap_or_else(|_| config::GPS_SERIAL_PORT.to_string());
        let gps = devices::gps::GpsController::new(&gps_port);
        if gps.is_connected() {
            println!("GPS connected on {}", gps_port);
        } else {
            eprintln!("GPS not connected on {}", gps_port);
        }

        let gps_controller = Arc::new(tokio::sync::Mutex::new(gps));
        let gps_task = gps_controller.clone();

        tokio::spawn(async move {
            let mut last_log = tokio::time::Instant::now()
                .checked_sub(tokio::time::Duration::from_secs(2))
                .unwrap_or_else(tokio::time::Instant::now);
            loop {
                let (updated, position, fix) = {
                    let mut gps = gps_task.lock().await;
                    let updated = gps.update();
                    if updated {
                        (updated, Some(gps.get_position()), gps.has_fix())
                    } else {
                        (false, None, false)
                    }
                };

                if updated {
                    if last_log.elapsed() >= tokio::time::Duration::from_secs(2) {
                        if let Some(position) = position {
                            println!(
                                "GPS: {:.6}, {:.6} | Alt: {:.1}m | Spd: {:.2}km/h | Sats: {} | Fix: {}",
                                position.latitude,
                                position.longitude,
                                position.altitude,
                                position.speed_kmh,
                                position.satellites,
                                fix
                            );
                        }
                        last_log = tokio::time::Instant::now();
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }
        });

        Some(gps_controller)
    } else {
        None
    };

    // Load saved per-servo angle tweaks if enabled
    if LOAD_SAVED_SERVO_TWEAKS {
        if let Some(tweaks) = load_saved_servo_tweaks() {
            let tweaks_arc = hexapod.get_servo_angle_tweaks();
            let mut t = tweaks_arc.lock().await;
            *t = tweaks;
        }
    }

    // Load saved per-gait configs if available
    let saved_gait_configs = load_saved_gait_configs();
    if !saved_gait_configs.is_empty() {
        let gait_controller = hexapod.get_gait_controller();
        let mut gait = gait_controller.lock().await;
        for (gait_type, cfg) in saved_gait_configs {
            let config = gait_config_from_file(gait_type, &cfg);
            gait.set_gait_config_for(gait_type, config);
        }
    }

    // Stand up into the current default stance instead of resetting to a fixed pose
    hexapod.update(0.0).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Display initial battery status
    hexapod.update(0.0).await; // Update to get initial battery reading
    let battery = hexapod.get_battery_status().await;
    if let Some(_) = battery.last_update {
        println!(
            "Battery Status: {:.2}V / {:.2}A",
            battery.voltage, battery.current
        );
    } else {
        println!("Battery Status: Not available (monitoring disabled)");
    }

    println!("Hexapod ready!\n");

    if config::WEB_API_ENABLE {
        println!(
            "Starting API server on port {} (non-blocking)...",
            config::API_PORT
        );

        // Create API state from hexapod shared references
        let state = api::AppState::from_hexapod(
            hexapod.get_control(),
            hexapod.get_gait_controller(),
            hexapod.get_ubec_controller(),
            hexapod.get_servo_angle_tweaks(),
            lidar_handle.clone(),
            lidar_error.clone(),
            hexapod.get_imu(),
            gps_controller.clone(),
        );

        // Spawn API server in background task
        tokio::spawn(async move {
            if let Err(e) = api::run_server(state, config::API_PORT).await {
                eprintln!("API server error: {}", e);
            }
        });

        println!("API server started on http://0.0.0.0:{}", config::API_PORT);
    }

    // ── Battery voltage logger (push to PC) ──────────────────────────────────
    // Every 60 seconds POSTs { session_id, minute, voltage } JSON to the PC
    // server running battery_server.py.
    // Override the target URL at runtime:  BATTERY_LOG_URL=http://x.x.x.x:5555/battery
    {
        use std::time::SystemTime;

        let ubec_for_log = hexapod.get_ubec_controller();
        let server_url = std::env::var("BATTERY_LOG_URL")
            .unwrap_or_else(|_| config::BATTERY_LOG_SERVER.to_string());

        // Build a session ID from the startup timestamp so the PC can group rows per run
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let (y, mo, d, h, mi, s) = epoch_to_datetime(ts);
        let session_id = format!(
            "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
            y, mo, d, h, mi, s
        );

        println!("[BatteryLog] Pushing voltage every 60s → {}", server_url);
        println!("[BatteryLog] Session ID: {}", session_id);

        tokio::spawn(async move {
            let http = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("Failed to build HTTP client");

            let mut minute: u32 = 0;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                minute += 1;

                // Read latest voltage from the shared UBEC controller
                let voltage = {
                    let mut ubec = ubec_for_log.lock().await;
                    ubec.update();
                    ubec.get_battery_status().voltage
                };

                let body = serde_json::json!({
                    "session_id": session_id,
                    "minute": minute,
                    "voltage": voltage,
                });

                match http.post(&server_url).json(&body).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        println!("[BatteryLog] minute={} voltage={:.3}V ✓", minute, voltage);
                    }
                    Ok(resp) => {
                        eprintln!("[BatteryLog] Server returned {} at minute {}", resp.status(), minute);
                    }
                    Err(e) => {
                        eprintln!("[BatteryLog] Failed to send minute {}: {}", minute, e);
                    }
                }
            }
        });
    }
    // ─────────────────────────────────────────────────────────────────────────


    if config::WEB_PANEL_ENABLE {
        println!(
            "Starting web panel on port {} (non-blocking)...",
            config::WEB_PANEL_PORT
        );

        // Spawn web panel in background task
        tokio::spawn(async move {
            if let Err(e) = web_panel::run_panel(config::WEB_PANEL_PORT).await {
                eprintln!("Web panel error: {}", e);
            }
        });

        println!(
            "Web panel started on http://0.0.0.0:{}",
            config::WEB_PANEL_PORT
        );
    }

    if config::WEB_API_ENABLE || config::WEB_PANEL_ENABLE {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        println!();
    }

    // Start AI Python module as subprocess
    let ai_child = if config::AI_ENABLE {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let ai_dir = exe_dir.join(config::AI_SCRIPT_DIR);
        let ai_script = ai_dir.join("main.py");

        if ai_script.exists() {
            println!("Starting AI module from {:?}...", ai_script);

            match std::process::Command::new("python3")
                .arg("main.py")
                .current_dir(&ai_dir)
                .env("HEXAPOD_API_BASE", format!("http://127.0.0.1:{}/api", config::API_PORT))
                .env("AI_CHAT_PORT", config::AI_CHAT_PORT.to_string())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
            {
                Ok(mut child) => {
                    println!("AI module started (PID: {})", child.id());

                    // Stream stdout
                    if let Some(stdout) = child.stdout.take() {
                        tokio::spawn(async move {
                            use tokio::io::{AsyncBufReadExt, BufReader};
                            let reader = BufReader::new(tokio::process::ChildStdout::from_std(stdout).unwrap());
                            let mut lines = reader.lines();
                            while let Ok(Some(line)) = lines.next_line().await {
                                println!("[AI] {}", line);
                            }
                        });
                    }
                    // Stream stderr
                    if let Some(stderr) = child.stderr.take() {
                        tokio::spawn(async move {
                            use tokio::io::{AsyncBufReadExt, BufReader};
                            let reader = BufReader::new(tokio::process::ChildStderr::from_std(stderr).unwrap());
                            let mut lines = reader.lines();
                            while let Ok(Some(line)) = lines.next_line().await {
                                eprintln!("[AI] {}", line);
                            }
                        });
                    }

                    Some(child)
                }
                Err(e) => {
                    eprintln!("Failed to start AI module: {}", e);
                    None
                }
            }
        } else {
            eprintln!("AI script not found at {:?} — skipping AI module", ai_script);
            None
        }
    } else {
        None
    };

    println!("Hexapod ready!\n");

    // Run demonstration sequence
    //tts::sayen("Starting demonstration sequence").unwrap();

    // Battery monitoring loop - check periodically during demos
    let battery_check_interval = tokio::time::Duration::from_secs(5);

    // Choose which demos to run:
    // Option 1: Run all demos
    //demos::run_all_demos(&mut hexapod).await;

    // Option 2: Run specific demos
    //demos::demo_body_tilt(&mut hexapod).await;
    //    demos::demo_tripod_walk(&mut hexapod, 10.0).await;
    //demos::demo_rotation(&mut hexapod, 4.0).await;
    //demos::demo_strafe(&mut hexapod, 4.0).await;
    //demos::demo_walk_with_tilt(&mut hexapod, 5.0).await;
    //demos::demo_combined_movement(&mut hexapod, 5.0).await;

    // Return to default stance
    //hexapod.reset_to_default_stance().await;

    // Final battery status
    hexapod.update(0.0).await;
    let battery = hexapod.get_battery_status().await;
    if let Some(_) = battery.last_update {
        println!(
            "\nFinal Battery Status: {:.2}V / {:.2}A",
            battery.voltage, battery.current
        );
    }

    println!("\n╔════════════════════════════════════════╗");
    println!("║   DEMO COMPLETE                       ║");
    println!("╚════════════════════════════════════════╝");

    //tts::sayen("Demo complete. Standing by.").unwrap();

    println!("\nPress Ctrl+C to exit.");

    // Main control loop - updates at ~20Hz
    // This is where hexapod.update() reads the control state and applies it
    let update_interval = tokio::time::Duration::from_millis(50); // 20 Hz
    let mut interval = tokio::time::interval(update_interval);
    let mut last_battery_check = tokio::time::Instant::now();

    loop {
        interval.tick().await;

        // Main update - reads control state and applies movement
        hexapod.update(0.05).await; // 50ms timestep

        // Check for critical battery state
        if hexapod.is_battery_critical().await {
            println!(
                "\n⚠️  CRITICAL BATTERY STATE DETECTED!\nInitiating emergency system shutdown..."
            );
            tts::sayen_blocking("Critical battery detected. Emergency shutdown initiated.")
                .unwrap();

            // Move to safe shutdown position to prevent servo strain
            // MG996R servos can draw up to 8A (all 18 = 144A!) if holding null/awkward angles
            println!("Moving to safe shutdown position...");
            hexapod.safe_shutdown_position().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

            // Execute system shutdown
            match hexapod.emergency_shutdown() {
                Ok(_) => {
                    println!(
                        "System shutdown command sent successfully.\nSystem will power off shortly...\nServos will maintain safe position until power loss."
                    );
                }
                Err(e) => {
                    eprintln!(
                        "Failed to execute shutdown command: {}\nPlease shutdown manually to prevent battery damage!",
                        e
                    );
                }
            }

            // Wait for shutdown to complete
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            break;
        }

        // Display battery status every 5 seconds
        if last_battery_check.elapsed() >= battery_check_interval {
            let battery = hexapod.get_battery_status().await;
            let power_state = hexapod.get_power_state().await;

            if let Some(_) = battery.last_update {
                print!(
                    "\rBattery: {:.2}V / {:.2}A | ",
                    battery.voltage, battery.current
                );

                match power_state {
                    devices::picoubec::PowerState::Normal => print!("Status: NORMAL ✓  "),
                    devices::picoubec::PowerState::LowBatteryWarning { timeout_seconds } => {
                        print!("Status: LOW BATTERY ⚠ ({}s) ", timeout_seconds);

                        // Warn user when battery is low
                        if timeout_seconds <= 20 && timeout_seconds % 10 == 0 {
                            println!(
                                "\n⚠️  WARNING: Low battery! System will auto-shutdown in {}s",
                                timeout_seconds
                            );
                        }
                    }
                    devices::picoubec::PowerState::Critical => print!("Status: CRITICAL ❌"),
                    devices::picoubec::PowerState::ShuttingDown { remaining_seconds } => {
                        print!("Status: SHUTDOWN ({}s) ", remaining_seconds);
                    }
                }

                use std::io::{self, Write};
                io::stdout().flush().unwrap();
            }

            last_battery_check = tokio::time::Instant::now();
        }
    }
}
