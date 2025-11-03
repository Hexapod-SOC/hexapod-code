#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod config;
pub mod hexapod;
pub mod demos;

// Workspace imports
use config::{TTS_URL, TMP_DIR, CONSTRAINTS, SERVO_PINS};
use movement::gaits::GAITS;
use audio::tts;

#[tokio::main]
async fn main() {
    println!("╔════════════════════════════════════════╗");
    println!("║   HEXAPOD ROBOT - EY                  ║");
    println!("╚════════════════════════════════════════╝\n");

    // Initialize text-to-speech
    println!("Initializing TTS...");
    tts::init(TTS_URL, TMP_DIR);
    tts::cleanup_cache(7).unwrap();
    tts::sayen("Hexapod initializing...").unwrap();

    // Create hexapod controller with tripod gait
    println!("Creating hexapod controller...");
    let mut hexapod = hexapod::Hexapod::new(
        SERVO_PINS,
        CONSTRAINTS,
        &GAITS[0], // Tripod gait
        None, // Use default stance
    );

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Display initial battery status
    hexapod.update(0.0); // Update to get initial battery reading
    let battery = hexapod.get_battery_status();
    if let Some(_) = battery.last_update {
        println!("Battery Status: {:.2}V / {:.2}A", battery.voltage, battery.current);
    } else {
        println!("Battery Status: Not available (monitoring disabled)");
    }
    
    println!("Hexapod ready!\n");

    // Run demonstration sequence
    tts::sayen("Starting demonstration sequence").unwrap();
    
    // Battery monitoring loop - check periodically during demos
    let battery_check_interval = tokio::time::Duration::from_secs(5);
    
    // Choose which demos to run:
    // Option 1: Run all demos
    //demos::run_all_demos(&mut hexapod).await;
    
    // Option 2: Run specific demos
    //demos::demo_body_tilt(&mut hexapod).await;
    demos::demo_tripod_walk(&mut hexapod, 10.0).await;
    //demos::demo_rotation(&mut hexapod, 4.0).await;
    //demos::demo_strafe(&mut hexapod, 4.0).await;
    //demos::demo_walk_with_tilt(&mut hexapod, 5.0).await;
    //demos::demo_combined_movement(&mut hexapod, 5.0).await;

    // Return to default stance
    hexapod.reset_to_default_stance();
    
    // Final battery status
    hexapod.update(0.0);
    let battery = hexapod.get_battery_status();
    if let Some(_) = battery.last_update {
        println!("\nFinal Battery Status: {:.2}V / {:.2}A", battery.voltage, battery.current);
    }

    println!("\n╔════════════════════════════════════════╗");
    println!("║   DEMO COMPLETE                       ║");
    println!("╚════════════════════════════════════════╝");
    
    tts::sayen("Demo complete. Standing by.").unwrap();
    
    println!("\nPress Ctrl+C to exit.");
    
    // Monitoring loop with periodic battery updates
    let mut last_battery_check = tokio::time::Instant::now();
    loop {
        hexapod.update(0.0);
        
        // Check for critical battery state
        if hexapod.is_battery_critical() {
            println!("\n⚠️  CRITICAL BATTERY STATE DETECTED!");
            println!("Initiating emergency system shutdown...");
            
            tts::sayen("Critical battery detected. Emergency shutdown initiated.").unwrap();
            
            // Wait a moment for TTS to complete
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            
            // Move to safe shutdown position to prevent servo strain
            // MG996R servos can draw up to 8A (all 18 = 144A!) if holding awkward angles
            println!("Moving to safe shutdown position...");
            hexapod.safe_shutdown_position();
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            
            // Execute system shutdown
            match hexapod.emergency_shutdown() {
                Ok(_) => {
                    println!("System shutdown command sent successfully.");
                    println!("System will power off shortly...");
                    println!("Servos will maintain safe position until power loss.");
                }
                Err(e) => {
                    eprintln!("Failed to execute shutdown command: {}", e);
                    eprintln!("Please shutdown manually to prevent battery damage!");
                }
            }
            
            // Wait for shutdown to complete
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            break;
        }
        
        // Display battery status every 5 seconds
        if last_battery_check.elapsed() >= battery_check_interval {
            let battery = hexapod.get_battery_status();
            let power_state = hexapod.get_power_state();
            
            if let Some(_) = battery.last_update {
                print!("\rBattery: {:.2}V / {:.2}A | ", battery.voltage, battery.current);
                
                match power_state {
                    devices::picoubec::PowerState::Normal => print!("Status: NORMAL ✓  "),
                    devices::picoubec::PowerState::LowBatteryWarning { timeout_seconds } => {
                        print!("Status: LOW BATTERY ⚠ ({}s) ", timeout_seconds);
                        
                        // Warn user when battery is low
                        if timeout_seconds <= 20 && timeout_seconds % 10 == 0 {
                            println!("\n⚠️  WARNING: Low battery! System will auto-shutdown in {}s", timeout_seconds);
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
        
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    }
}