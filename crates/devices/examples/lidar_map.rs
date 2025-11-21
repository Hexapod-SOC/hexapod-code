//! LiDAR Real-Time ASCII Visualization
//!
//! This example displays a live ASCII art visualization of the LiDAR scan data,
//! allowing you to see obstacles around the sensor in real-time.
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
//! cargo run --example lidar_map --features real
//! ```
//!
//! For testing on PC (dummy mode):
//! ```bash
//! cargo run --example lidar_map --features dummy
//! ```

use devices::lidar::LidarDriver;
use std::thread;
use std::time::{Duration, Instant};

const MAP_WIDTH: usize = 80;
const MAP_HEIGHT: usize = 40;
const MAX_RANGE_MM: f32 = 4000.0; // 4 meters max display range

fn main() -> anyhow::Result<()> {
    println!("\x1B[2J\x1B[H"); // Clear screen and move to top
    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║              LD19 LiDAR Real-Time ASCII Map Visualization                    ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");
    println!();

    let port = "/dev/ttyUSB0";
    
    println!("🔌 Connecting to LiDAR on port: {}", port);
    let mut driver = match LidarDriver::new(port) {
        Ok(d) => {
            println!("✓ Successfully opened serial port");
            d
        }
        Err(e) => {
            eprintln!("✗ Failed to open serial port: {}", e);
            return Err(e);
        }
    };
    
    println!("🚀 Starting LiDAR...");
    driver.start()?;
    println!("✓ LiDAR is running");
    println!();
    
    thread::sleep(Duration::from_millis(500)); // Give it time to start

    let mut frame_count = 0;
    let start_time = Instant::now();
    let mut last_frame_time = Instant::now();

    loop {
        if driver.is_frame_ready() {
            if let Some(cloud) = driver.get_point_cloud() {
                frame_count += 1;
                let frame_time = last_frame_time.elapsed();
                last_frame_time = Instant::now();
                
                let elapsed = start_time.elapsed().as_secs_f64();
                let avg_fps = frame_count as f64 / elapsed;
                let instant_fps = 1000.0 / frame_time.as_millis() as f64;
                
                // Clear screen and draw new frame
                print!("\x1B[H"); // Move to top without clearing (reduces flicker)
                
                draw_map(&cloud);
                
                println!();
                println!("╔══════════════════════════════════════════════════════════════════════════════╗");
                println!("║ Frame: {:<6} │ Speed: {:.1} Hz │ Points: {:<4}/{:<4} │ FPS: {:.1} │ Inst: {:.1} Hz ║",
                         frame_count, cloud.frequency(), cloud.valid_count(), 
                         cloud.points.len(), avg_fps, instant_fps);
                println!("║ Errors: {:<4} │ Max Range: {} m │ Legend: * obstacle  . far  + sensor      ║",
                         driver.get_error_count(), (MAX_RANGE_MM / 1000.0) as u32);
                println!("╚══════════════════════════════════════════════════════════════════════════════╝");
            }
        }
        
        thread::sleep(Duration::from_millis(10));
    }
}

fn draw_map(cloud: &devices::lidar::PointCloud) {
    // Create the map grid
    let mut grid = vec![vec![' '; MAP_WIDTH]; MAP_HEIGHT];
    
    // Calculate center point (sensor location)
    let center_x = MAP_WIDTH / 2;
    let center_y = MAP_HEIGHT / 2;
    
    // Place sensor marker
    grid[center_y][center_x] = '+';
    
    // Plot each valid point
    for point in cloud.valid_points() {
        if point.distance == 0 {
            continue;
        }
        
        let distance_mm = point.distance as f32;
        if distance_mm > MAX_RANGE_MM {
            continue;
        }
        
        // Convert polar to cartesian (with map scaling)
        let angle_rad = point.angle_radians();
        
        // Scale: pixels per mm
        let scale = (MAP_WIDTH.min(MAP_HEIGHT) as f32 / 2.0) / MAX_RANGE_MM;
        
        let x = (distance_mm * angle_rad.sin() * scale) as i32;
        let y = (distance_mm * angle_rad.cos() * scale) as i32;
        
        // Map coordinates (invert Y for screen coordinates)
        let map_x = center_x as i32 + x;
        let map_y = center_y as i32 - y; // Invert Y axis
        
        // Check bounds
        if map_x >= 0 && map_x < MAP_WIDTH as i32 && map_y >= 0 && map_y < MAP_HEIGHT as i32 {
            let map_x = map_x as usize;
            let map_y = map_y as usize;
            
            // Choose character based on distance
            let char = if distance_mm < 500.0 {
                '█' // Very close
            } else if distance_mm < 1000.0 {
                '▓' // Close
            } else if distance_mm < 2000.0 {
                '▒' // Medium
            } else if distance_mm < 3000.0 {
                '░' // Far
            } else {
                '·' // Very far
            };
            
            grid[map_y][map_x] = char;
        }
    }
    
    // Draw border and cardinal directions
    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    
    // Print the grid
    for (y, row) in grid.iter().enumerate() {
        print!("║");
        
        for (x, &ch) in row.iter().enumerate() {
            // Add cardinal direction markers
            if y == 0 && x == center_x {
                print!("N"); // North (forward)
            } else if y == MAP_HEIGHT - 1 && x == center_x {
                print!("S"); // South (backward)
            } else if x == 0 && y == center_y {
                print!("W"); // West (left)
            } else if x == MAP_WIDTH - 1 && y == center_y {
                print!("E"); // East (right)
            } else {
                print!("{}", ch);
            }
        }
        
        println!("║");
    }
    
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");
}
