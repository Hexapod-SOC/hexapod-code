//! Example demonstrating how to use the PicoUbec battery monitoring system
//!
//! Run with: cargo run --example test_picoubec --features dummy
//! Or on real hardware: cargo run --example test_picoubec --features real

use devices::picoubec::{PicoUbecController, PowerState};
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== PicoUbec Battery Monitor Test ===\n");

    // Initialize the controller
    // On real hardware, use "/dev/ttyUSB0" or appropriate serial port
    let mut ubec = PicoUbecController::new("/dev/ttyUSB0");

    println!("Starting battery monitoring loop...");
    println!("Press Ctrl+C to exit\n");

    let mut loop_count = 0;

    loop {
        loop_count += 1;

        // Update battery status (non-blocking)
        let got_update = ubec.update();

        if got_update || loop_count % 4 == 0 {
            // Display status every update or every ~1 second
            let status = ubec.get_battery_status();
            let power_state = ubec.get_power_state();

            print!("\r"); // Clear line
            match status.last_update {
                Some(_) => {
                    print!(
                        "Battery: {:.2}V / {:.2}A | ",
                        status.voltage, status.current
                    );
                }
                None => {
                    print!("Battery: No data | ");
                }
            }

            match power_state {
                PowerState::Normal => print!("State: NORMAL ✓"),
                PowerState::LowBatteryWarning { timeout_seconds } => {
                    print!("State: WARNING ({}s remaining)", timeout_seconds)
                }
                PowerState::Critical => print!("State: CRITICAL ⚠"),
                PowerState::ShuttingDown { remaining_seconds } => {
                    print!("State: SHUTTING DOWN ({}s)", remaining_seconds)
                }
            }

            use std::io::{self, Write};
            io::stdout().flush().unwrap();
        }

        // Check for critical state
        if ubec.is_critical() {
            println!("\n\n⚠ CRITICAL STATE DETECTED!");
            println!("Battery voltage too low or system error.");
            println!("Exiting...");
            break;
        }

        // Uncomment to test shutdown command after 10 seconds
        // if loop_count == 40 {
        //     println!("\n\nSending shutdown command...");
        //     ubec.send_shutdown(30);
        // }

        // Sleep for 250ms (approximate UART update rate)
        thread::sleep(Duration::from_millis(250));
    }

    println!("\nMonitoring stopped.");
}
