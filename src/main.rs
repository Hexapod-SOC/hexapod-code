#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod api;
pub mod config;
pub mod demos;
pub mod hexapod;

// Workspace imports
use crate::hexapod::{LegStances, ServoAngleTriplet, ServoAngleTweaks};
use audio::tts;
use config::CALIBRATION_LEG_STANCE_FILE;
use config::{
    CALIBRATION_SERVO_TWEAKS_FILE, CONSTRAINTS, LOAD_SAVED_SERVO_TWEAKS, SERVO_OFFSETS,
    SERVO_PINS, TMP_DIR, TTS_URL,
};
use glam::Vec3;
use hexmath::GaitType;
use std::sync::Arc;

fn load_saved_servo_tweaks() -> Option<ServoAngleTweaks> {
    let path = std::path::Path::new(CALIBRATION_SERVO_TWEAKS_FILE);
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    #[derive(serde::Deserialize)]
    struct FileTweaks {
        left_front: [f32; 3],
        left_middle: [f32; 3],
        left_back: [f32; 3],
        right_front: [f32; 3],
        right_middle: [f32; 3],
        right_back: [f32; 3],
    }
    let parsed: FileTweaks = serde_json::from_str(&content).ok()?;
    Some(ServoAngleTweaks {
        left_front: ServoAngleTriplet {
            coxa: parsed.left_front[0],
            femur: parsed.left_front[1],
            tibia: parsed.left_front[2],
        },
        left_middle: ServoAngleTriplet {
            coxa: parsed.left_middle[0],
            femur: parsed.left_middle[1],
            tibia: parsed.left_middle[2],
        },
        left_back: ServoAngleTriplet {
            coxa: parsed.left_back[0],
            femur: parsed.left_back[1],
            tibia: parsed.left_back[2],
        },
        right_front: ServoAngleTriplet {
            coxa: parsed.right_front[0],
            femur: parsed.right_front[1],
            tibia: parsed.right_front[2],
        },
        right_middle: ServoAngleTriplet {
            coxa: parsed.right_middle[0],
            femur: parsed.right_middle[1],
            tibia: parsed.right_middle[2],
        },
        right_back: ServoAngleTriplet {
            coxa: parsed.right_back[0],
            femur: parsed.right_back[1],
            tibia: parsed.right_back[2],
        },
    })
}
fn load_saved_leg_stance() -> Option<LegStances> {
    let path = std::path::Path::new(CALIBRATION_LEG_STANCE_FILE);
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    #[derive(serde::Deserialize)]
    struct FileStance {
        left_front: [f32; 3],
        left_middle: [f32; 3],
        left_back: [f32; 3],
        right_front: [f32; 3],
        right_middle: [f32; 3],
        right_back: [f32; 3],
    }
    let parsed: FileStance = serde_json::from_str(&content).ok()?;
    Some(LegStances {
        left_front: Vec3::from_array(parsed.left_front),
        left_middle: Vec3::from_array(parsed.left_middle),
        left_back: Vec3::from_array(parsed.left_back),
        right_front: Vec3::from_array(parsed.right_front),
        right_middle: Vec3::from_array(parsed.right_middle),
        right_back: Vec3::from_array(parsed.right_back),
    })
}

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

    // Create hexapod controller with tripod gait
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

    let lidar_handle = if config::LIDAR_SLAM_ENABLE {
        match devices::lidar::LidarSlamHandle::new(config::lidar_slam_config()) {
            Ok(handle) => {
                println!("LiDAR SLAM thread started on {}", config::LIDAR_SERIAL_PORT);
                Some(Arc::new(handle))
            }
            Err(err) => {
                eprintln!("Failed to start LiDAR SLAM: {err:?}");
                None
            }
        }
    } else {
        None
    };

    if config::GPS_ENABLE {
        let gps_port =
            std::env::var("GPS_PORT").unwrap_or_else(|_| config::GPS_SERIAL_PORT.to_string());
        let mut gps = devices::gps::GpsController::new(&gps_port);
        if gps.is_connected() {
            println!("GPS connected on {}", gps_port);
        } else {
            eprintln!("GPS not connected on {}", gps_port);
        }

        tokio::spawn(async move {
            let mut last_log = tokio::time::Instant::now()
                .checked_sub(tokio::time::Duration::from_secs(2))
                .unwrap_or_else(tokio::time::Instant::now);
            loop {
                if gps.update() {
                    if last_log.elapsed() >= tokio::time::Duration::from_secs(2) {
                        let position = gps.get_position();
                        let fix = gps.has_fix();
                        println!(
                            "GPS: {:.6}, {:.6} | Alt: {:.1}m | Spd: {:.2}km/h | Sats: {} | Fix: {}",
                            position.latitude,
                            position.longitude,
                            position.altitude,
                            position.speed_kmh,
                            position.satellites,
                            fix
                        );
                        last_log = tokio::time::Instant::now();
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }
        });
    }

    // Load saved per-servo angle tweaks if enabled
    if LOAD_SAVED_SERVO_TWEAKS {
        if let Some(tweaks) = load_saved_servo_tweaks() {
            let tweaks_arc = hexapod.get_servo_angle_tweaks();
            let mut t = tweaks_arc.lock().await;
            *t = tweaks;
        }
    }

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
            hexapod.get_imu(),
        );

        // Spawn API server in background task
        tokio::spawn(async move {
            if let Err(e) = api::run_server(state, config::API_PORT).await {
                eprintln!("API server error: {}", e);
            }
        });

        println!("API server started on http://0.0.0.0:{}", config::API_PORT);
    }

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
