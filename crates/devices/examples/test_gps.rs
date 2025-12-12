use devices::gps::{FixQuality, GpsController};
use std::thread;
use std::time::Duration;

fn main() {
    println!("GPS Test Program");
    println!("================\n");

    // Initialize GPS controller
    // For real hardware, use something like "/dev/ttyUSB0" or "/dev/ttyAMA0"
    let port = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/dev/ttyUSB0".to_string());

    println!("Attempting to connect to GPS on {}...", port);
    let mut gps = GpsController::new(&port);

    if gps.is_connected() {
        println!("✓ GPS connected successfully\n");
    } else {
        #[cfg(feature = "dummy")]
        println!("✓ Running in dummy mode\n");
        #[cfg(feature = "real")]
        println!("✗ GPS connection failed (continuing anyway)\n");
    }

    println!("Reading GPS data (Press Ctrl+C to exit)...\n");

    let mut last_valid_fix = false;
    let mut update_count = 0;

    loop {
        // Update GPS data
        let data_received = gps.update();

        if data_received {
            update_count += 1;
            let position = gps.get_position();
            let has_fix = gps.has_fix();

            // Print status change
            if has_fix != last_valid_fix {
                if has_fix {
                    println!("✓ GPS FIX ACQUIRED");
                } else {
                    println!("✗ GPS FIX LOST");
                }
                last_valid_fix = has_fix;
            }

            // Print position data every 10 updates (reduce spam)
            if update_count % 10 == 0 {
                println!("\n--- GPS Data ---");
                println!(
                    "Position:    {:.6}°, {:.6}°",
                    position.latitude, position.longitude
                );
                println!("Altitude:    {:.2} m", position.altitude);
                println!("Speed:       {:.2} km/h", position.speed_kmh);
                if let Some(heading) = position.heading {
                    println!("Heading:     {:.1}°", heading);
                }
                println!("Satellites:  {}", position.satellites);
                println!("Fix Quality: {:?}", position.fix_quality);

                if let Some(last_update) = position.last_update {
                    println!(
                        "Last Update: {:.2}s ago",
                        last_update.elapsed().as_secs_f32()
                    );
                }

                // Print fix quality details
                match position.fix_quality {
                    FixQuality::NoFix => println!("Status:      ✗ No Fix"),
                    FixQuality::GpsFix => println!("Status:      ✓ GPS Fix"),
                    FixQuality::DifferentialFix => println!("Status:      ✓✓ Differential GPS"),
                    FixQuality::PpsFix => println!("Status:      ✓✓ PPS Fix"),
                    FixQuality::RtkFixed => println!("Status:      ✓✓✓ RTK Fixed"),
                    FixQuality::RtkFloat => println!("Status:      ✓✓ RTK Float"),
                    FixQuality::Estimated => println!("Status:      ~ Estimated"),
                    FixQuality::Manual => println!("Status:      M Manual"),
                    FixQuality::Simulation => println!("Status:      S Simulation"),
                }
                println!("---------------");
            }
        }

        // Small delay to prevent busy-waiting
        thread::sleep(Duration::from_millis(100));
    }
}
