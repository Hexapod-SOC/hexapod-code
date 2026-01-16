//! Pure-Rust LD19 driver and BreezySLAM-compatible primitives for the hexapod.
//! The crate is split into `driver` (LD19 protocol), `slam` (core SLAM math),
//! `types` (shared data types), and `map` (occupancy grid utilities).

pub mod driver;
pub mod map;
pub mod pipeline;
pub mod slam;
pub mod types;

pub use driver::Ld19Parser;
pub use map::OccupancyGrid;
pub use pipeline::LidarSlam;
pub use slam::{BreezySLAM, SlamParams};
pub use types::{LaserScan, LidarPoint, Pose2D, PoseDelta};
