//! Example: Read and display LiDAR data
//!
//! This example demonstrates how to use the LD19 LiDAR driver to read and display
//! point cloud data from the sensor.
//!
//! # Usage
//!
//! Run with dummy features (no hardware):
//! ```bash
//! cargo run --example lidar_test --features dummy
//! ```
//!
//! Run with real hardware:
//! ```bash
//! cargo run --example lidar_test --features real
//! ```

use devices::lidar::{LidarDriver, Point};
use std::thread;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    println!("LD19 LiDAR Driver Test");
    println!("======================\n");

    // Configure the serial port (adjust as needed)
    let port = "/dev/ttyUSB0";

    println!("Opening LiDAR on port: {}", port);
    let mut driver = LidarDriver::new(port)?;

    println!("Starting LiDAR...");
    driver.start()?;

    println!("Waiting for data...\n");

    let mut frame_count = 0;
    let max_frames = 50; // Display 50 frames then exit

    while frame_count < max_frames {
        if driver.is_frame_ready() {
            if let Some(cloud) = driver.get_point_cloud() {
                frame_count += 1;

                println!("Frame #{}", frame_count);
                println!("  Timestamp: {} ms", cloud.timestamp);
                println!("  Speed: {:.2} Hz", cloud.frequency());
                println!("  Total points: {}", cloud.points.len());
                println!("  Valid points: {}", cloud.valid_count());

                // Display statistics
                if let Some(stats) = calculate_statistics(&cloud.points) {
                    println!(
                        "  Distance range: {} - {} mm",
                        stats.min_dist, stats.max_dist
                    );
                    println!("  Average distance: {:.0} mm", stats.avg_dist);
                    println!("  Average intensity: {:.1}", stats.avg_intensity);
                }

                // Display obstacles in front (±30 degrees from forward)
                let front_points = cloud.points_in_range(345.0, 15.0);
                if !front_points.is_empty() {
                    let closest_front = front_points
                        .iter()
                        .filter(|p| p.is_valid())
                        .min_by_key(|p| p.distance);

                    if let Some(point) = closest_front {
                        println!(
                            "  Closest obstacle ahead: {} mm at {:.1}°",
                            point.distance, point.angle
                        );
                    }
                }

                println!("  Errors: {}", driver.get_error_count());
                println!();
            }
        }

        thread::sleep(Duration::from_millis(50));
    }

    println!("Stopping LiDAR...");
    driver.stop()?;

    println!("Done!");
    Ok(())
}

struct Statistics {
    min_dist: u16,
    max_dist: u16,
    avg_dist: f32,
    avg_intensity: f32,
}

fn calculate_statistics(points: &[Point]) -> Option<Statistics> {
    let valid_points: Vec<_> = points.iter().filter(|p| p.is_valid()).collect();

    if valid_points.is_empty() {
        return None;
    }

    let min_dist = valid_points.iter().map(|p| p.distance).min().unwrap();

    let max_dist = valid_points.iter().map(|p| p.distance).max().unwrap();

    let sum_dist: u32 = valid_points.iter().map(|p| p.distance as u32).sum();
    let avg_dist = sum_dist as f32 / valid_points.len() as f32;

    let sum_intensity: u32 = valid_points.iter().map(|p| p.intensity as u32).sum();
    let avg_intensity = sum_intensity as f32 / valid_points.len() as f32;

    Some(Statistics {
        min_dist,
        max_dist,
        avg_dist,
        avg_intensity,
    })
}
