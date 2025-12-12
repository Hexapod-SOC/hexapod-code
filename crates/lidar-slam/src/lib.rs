//! 2D SLAM (Simultaneous Localization and Mapping) for LD19 LiDAR
//!
//! This crate provides SLAM capabilities for a hexapod robot using a 2D LiDAR sensor.
//!
//! # Features
//!
//! - **Scan Matching**: ICP (Iterative Closest Point) algorithm for pose estimation
//! - **Occupancy Grid**: 2D grid map for environment representation
//! - **Pose Tracking**: Robot pose estimation with gyro support (placeholder for when connected)
//! - **Web Visualization**: Real-time map and pose visualization
//!
//! # Example
//!
//! ```no_run
//! use lidar_slam::{SlamProcessor, SlamConfig};
//! use devices::lidar::LidarDriver;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let mut lidar = LidarDriver::new("/dev/ttyUSB0")?;
//! lidar.start()?;
//!
//! let config = SlamConfig::default();
//! let mut slam = SlamProcessor::new(config);
//!
//! if let Some(cloud) = lidar.get_point_cloud() {
//!     slam.process_scan(&cloud);
//!     let pose = slam.current_pose();
//!     let map = slam.get_map();
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Gyro Integration
//!
//! The SLAM processor supports gyro data for improved orientation estimation.
//! Currently gyro is forced to 0 (disconnected), but the API is ready for integration.

pub mod icp;
pub mod occupancy_grid;
pub mod slam;
pub mod types;
pub mod web;

pub use occupancy_grid::{CellState, OccupancyGrid};
pub use slam::{SlamBuilder, SlamConfig, SlamProcessor};
pub use types::{Point2D, Pose2D, Scan2D, Transform2D};
pub use web::SlamWebServer;
