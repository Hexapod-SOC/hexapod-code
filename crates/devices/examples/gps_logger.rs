use devices::gps::{FixQuality, GpsController};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    println!("GPS Logger");
    println!("==========\n");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).map(|s| s.as_str()).unwrap_or("/dev/ttyUSB0");
    let log_file = args.get(2).map(|s| s.as_str()).unwrap_or("gps_log.csv");

    println!("GPS Port: {}", port);
    println!("Log File: {}", log_file);
    println!();

    // Initialize GPS controller
    let mut gps = GpsController::new(port);

    if !gps.is_connected() {
        #[cfg(feature = "real")]
        {
            eprintln!("Error: Failed to connect to GPS");
            std::process::exit(1);
        }
    }

    // Create log file
    let file = File::create(log_file).expect("Failed to create log file");
    let mut writer = BufWriter::new(file);

    // Write CSV header
    writeln!(
        writer,
        "timestamp,latitude,longitude,altitude,speed_kmh,heading,satellites,fix_quality,has_fix"
    )
    .expect("Failed to write header");

    println!("Logging GPS data... (Press Ctrl+C to stop)\n");

    let mut log_count = 0;
    let start_time = SystemTime::now();

    loop {
        // Update GPS data
        if gps.update() {
            let position = gps.get_position();
            let has_fix = gps.has_fix();

            // Get timestamp
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64();

            // Write to log file
            let fix_quality_str = match position.fix_quality {
                FixQuality::NoFix => "NoFix",
                FixQuality::GpsFix => "GpsFix",
                FixQuality::DifferentialFix => "DifferentialFix",
                FixQuality::PpsFix => "PpsFix",
                FixQuality::RtkFixed => "RtkFixed",
                FixQuality::RtkFloat => "RtkFloat",
                FixQuality::Estimated => "Estimated",
                FixQuality::Manual => "Manual",
                FixQuality::Simulation => "Simulation",
            };

            writeln!(
                writer,
                "{},{},{},{},{},{},{},{},{}",
                timestamp,
                position.latitude,
                position.longitude,
                position.altitude,
                position.speed_kmh,
                position
                    .heading
                    .map(|h| h.to_string())
                    .unwrap_or_else(|| "".to_string()),
                position.satellites,
                fix_quality_str,
                has_fix
            )
            .expect("Failed to write log entry");

            log_count += 1;

            // Print status every 50 entries
            if log_count % 50 == 0 {
                let elapsed = start_time.elapsed().unwrap().as_secs();
                println!(
                    "[{}] Logged {} entries | Position: {:.6}°, {:.6}° | Sats: {} | Fix: {:?}",
                    elapsed,
                    log_count,
                    position.latitude,
                    position.longitude,
                    position.satellites,
                    position.fix_quality
                );

                // Flush to ensure data is written
                writer.flush().expect("Failed to flush log file");
            }
        }

        // Small delay
        thread::sleep(Duration::from_millis(100));
    }
}
