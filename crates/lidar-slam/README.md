# LiDAR SLAM for Hexapod

A 2D SLAM (Simultaneous Localization and Mapping) implementation for the LD19 LiDAR sensor on a hexapod robot.

## Features

- **ICP Scan Matching**: Iterative Closest Point algorithm for accurate pose estimation
- **Occupancy Grid Map**: 2D grid representation of the environment
- **Real-time Web Visualization**: Interactive map and trajectory display
- **Gyro Support**: Ready for gyro integration (currently forced to 0 as disconnected)
- **Future Camera Support**: API prepared for visual SLAM input

## Usage

### Basic SLAM

```rust
use lidar_slam::{SlamProcessor, SlamConfig};
use devices::lidar::LidarDriver;

let mut lidar = LidarDriver::new("/dev/ttyUSB0")?;
lidar.start()?;

let config = SlamConfig::default();
let mut slam = SlamProcessor::new(config);

loop {
    if let Some(cloud) = lidar.get_point_cloud() {
        slam.process_scan(&cloud);
        
        let pose = slam.current_pose();
        println!("Robot at: ({}, {}) θ={}°", 
            pose.x, pose.y, pose.theta.to_degrees());
    }
}
```

### With Web Visualization

```rust
use lidar_slam::{SlamProcessor, SlamWebServer, SlamBuilder};

let slam = SlamBuilder::new()
    .with_grid_resolution(50.0)   // 5cm cells
    .with_grid_size(400, 400)     // 20m x 20m map
    .with_max_range(8000.0)       // 8m max range
    .build();

let slam = Arc::new(Mutex::new(slam));
let server = SlamWebServer::new(slam.clone());

// Run server on port 3002
let app = server.build_router();
axum::serve(listener, app).await?;
```

## Running the Example

With real hardware on Raspberry Pi:
```bash
cargo run --example slam_web --features real -p lidar-slam
```

For testing (dummy mode):
```bash
cargo run --example slam_web --features dummy -p lidar-slam
```

Then open http://localhost:3002 in your browser.

## Configuration

### SlamConfig

| Parameter | Default | Description |
|-----------|---------|-------------|
| `grid.resolution` | 50.0 | Cell size in mm (5cm) |
| `grid.width/height` | 400 | Map size in cells (20m x 20m) |
| `max_range` | 8000.0 | Maximum scan range in mm |
| `min_update_distance` | 50.0 | Min travel for map update |
| `min_update_rotation` | 0.05 | Min rotation for map update |
| `use_gyro` | false | Use gyro for orientation |
| `lidar_height` | 100.0 | LiDAR height from ground |

### IcpConfig

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_iterations` | 50 | Max ICP iterations |
| `translation_threshold` | 1.0 | Convergence threshold (mm) |
| `rotation_threshold` | 0.001 | Convergence threshold (rad) |
| `max_correspondence_dist` | 500.0 | Max match distance (mm) |
| `min_correspondences` | 10 | Min matches required |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      SLAM Processor                         │
│  ┌──────────┐  ┌──────────┐  ┌─────────────────────────┐   │
│  │   ICP    │  │ Occupancy│  │     Pose Estimator      │   │
│  │ Matcher  │→ │   Grid   │← │ (velocity + gyro pred)  │   │
│  └──────────┘  └──────────┘  └─────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
         ↑                              ↓
    LiDAR Scan                    Robot Pose
    (PointCloud)                  + Map Data
         ↑                              ↓
┌─────────────┐              ┌──────────────────┐
│ LidarDriver │              │ Web Visualization│
│   (LD19)    │              │   (WebSocket)    │
└─────────────┘              └──────────────────┘
```

## Gyro Integration

When the gyro is connected, enable it:

```rust
let config = SlamConfig {
    use_gyro: true,
    ..Default::default()
};

// Update gyro readings
slam.update_gyro(GyroData {
    roll: 0.0,
    pitch: 0.0,
    yaw: imu_yaw,
    timestamp: now,
});
```

## Future Camera SLAM

The architecture is prepared for camera input:

```rust
// Point3D for 3D mapping
let point_3d = Point3D::from_2d_with_height(&point_2d, lidar_height);

// Project back to 2D for current map
let point_2d = point_3d.to_2d();
```

## Web UI Features

- **Real-time Map Display**: Occupancy grid with free/occupied/unknown cells
- **Robot Visualization**: Hexagon-shaped robot icon with heading indicator  
- **Trajectory Path**: Historical path in cyan
- **Current Scan**: Red dots showing latest LiDAR points
- **LiDAR View**: Local polar view of current scan
- **Statistics**: Scan count, match quality, map updates
- **Controls**: Reset, zoom, pan, auto-center toggle

## License

MIT
