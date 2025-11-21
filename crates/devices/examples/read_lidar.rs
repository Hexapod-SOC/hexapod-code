                                                                                            //! LiDAR Reading Test
//!
//! This example continuously reads data from the LD19 LiDAR and displays
//! real-time information about the point clouds being received.
//!
//! # Hardware Setup
//!
//! Connect the LD19 LiDAR to your Raspberry Pi:
//! - VCC (red) -> 5V (Pin 2 or 4)
//! - GND (black) -> GND (Pin 6)
//! - TX (green) -> RX/GPIO15 (Pin 10)
//!
//! # Usage
//!
//! On Raspberry Pi with real hardware:
//! ```bash
//! cargo run --example read_lidar --features real
//! ```
//!
//! For testing on PC (dummy mode):
//! ```bash
//! cargo run --example read_lidar --features dummy
//! ```

use devices::lidar::LidarDriver;
use std::thread;
use std::time::{Duration, Instant};

fn main() -> anyhow::Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           LD19 LiDAR Real-Time Data Reader                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Configure the serial port
    let port = "/dev/ttyUSB0";
    
    println!("🔌 Connecting to LiDAR on port: {}", port);
    let mut driver = match LidarDriver::new(port) {
        Ok(d) => {
            println!("✓ Successfully opened serial port");
            d
        }
        Err(e) => {
            eprintln!("✗ Failed to open serial port: {}", e);
            eprintln!();
            eprintln!("Troubleshooting tips:");
            eprintln!("  1. Check if LiDAR is connected");
            eprintln!("  2. Verify port name (try /dev/ttyAMA0 or /dev/ttyS0)");
            eprintln!("  3. Check permissions: sudo usermod -a -G dialout $USER");
            eprintln!("  4. Make sure LiDAR has 5V power supply");
            return Err(e);
        }
    };
    
    println!("🚀 Starting LiDAR data collection...");
    driver.start()?;
    println!("✓ LiDAR is now running");
    println!();

    let start_time = Instant::now();
    let mut frame_count = 0;
    let mut total_points = 0;
    let mut last_status_time = Instant::now();
    let mut last_print_time = Instant::now();

    println!("📊 Reading data... (Press Ctrl+C to stop)");
    println!("─────────────────────────────────────────────────────────────");
    println!();

    loop {
        if driver.is_frame_ready() {
            if let Some(cloud) = driver.get_point_cloud() {
                frame_count += 1;
                total_points += cloud.points.len();
                
                let elapsed = start_time.elapsed().as_secs_f64();
                let fps = frame_count as f64 / elapsed;
                
                // Only update display every 100ms to avoid overwhelming terminal
                if last_print_time.elapsed() >= Duration::from_millis(100) {
                    // Clear previous lines and print new status (single write)
                    print!("\x1B[2K\rFrame #{:<4} │ Speed: {:.2} Hz │ Points: {:<4}/{:<4} │ Timestamp: {:<6} ms │ FPS: {:.1} │ Errors: {}", 
                           frame_count, cloud.frequency(), cloud.valid_count(), 
                           cloud.points.len(), cloud.timestamp, fps, driver.get_error_count());
                    
                    // Flush to ensure it appears
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                    
                    last_print_time = Instant::now();
                }
                
                // Print detailed info every 2 seconds
                if last_status_time.elapsed() >= Duration::from_secs(2) {
                    println!();
                    println!();
                    print_detailed_stats(&cloud);
                    print_obstacle_info(&cloud);
                    println!();
                    last_status_time = Instant::now();
                }
            }
        }
        
        // Sleep very briefly - we want to be responsive
        thread::sleep(Duration::from_millis(5));
    }
}

fn print_detailed_stats(cloud: &devices::lidar::PointCloud) {
    let valid_points: Vec<_> = cloud.valid_points().collect();
    
    if valid_points.is_empty() {
        println!("  ⚠️  No valid points in this frame");
        return;
    }

    let min_dist = valid_points.iter().map(|p| p.distance).min().unwrap();
    let max_dist = valid_points.iter().map(|p| p.distance).max().unwrap();
    let sum_dist: u32 = valid_points.iter().map(|p| p.distance as u32).sum();
    let avg_dist = sum_dist as f32 / valid_points.len() as f32;
    
    let sum_intensity: u32 = valid_points.iter().map(|p| p.intensity as u32).sum();
    let avg_intensity = sum_intensity as f32 / valid_points.len() as f32;

    println!("  📏 Distance Statistics:");
    println!("     Min: {:>6} mm  │  Max: {:>6} mm  │  Avg: {:>6.0} mm", 
             min_dist, max_dist, avg_dist);
    println!("  💡 Average Intensity: {:.1}", avg_intensity);
}

fn print_obstacle_info(cloud: &devices::lidar::PointCloud) {
    // Check different directions
    let directions = [
        ("Front", 0.0, 15.0),
        ("Front-Right", 45.0, 15.0),
        ("Right", 90.0, 15.0),
        ("Back-Right", 135.0, 15.0),
        ("Back", 180.0, 15.0),
        ("Back-Left", 225.0, 15.0),
        ("Left", 270.0, 15.0),
        ("Front-Left", 315.0, 15.0),
    ];

    println!("  🎯 Obstacles by Direction:");
    
    // Build output string first, then print once
    let mut output = String::with_capacity(256);
    
    for (i, (name, angle, tolerance)) in directions.iter().enumerate() {
        if let Some(point) = cloud.closest_in_direction(*angle, *tolerance) {
            let distance_cm = point.distance as f32 / 10.0;
            let symbol = if distance_cm < 20.0 {
                "🔴"
            } else if distance_cm < 50.0 {
                "🟡"
            } else {
                "🟢"
            };
            
            output.push_str(&format!("     {} {:>11}: {:>6.1} cm  │  ", symbol, name, distance_cm));
        } else {
            output.push_str(&format!("     ⚫ {:>11}: No data  │  ", name));
        }
        
        // New line every 2 items
        if i == 1 || i == 3 || i == 5 || i == 7 {
            output.push('\n');
        }
    }
    
    print!("{}", output);
    println!();
}
