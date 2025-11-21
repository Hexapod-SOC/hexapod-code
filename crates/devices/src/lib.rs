#[cfg(not(any(feature = "dummy", feature = "real")))]
compile_error!("You must enable either `dummy` or `real` feature for devices!");

#[cfg(feature = "dummy")]
pub mod servo_dummy;
#[cfg(feature = "real")]
pub mod servo_real;

#[cfg(feature = "dummy")]
pub mod picoubec_dummy;
#[cfg(feature = "real")]
pub mod picoubec_real;

#[cfg(feature = "dummy")]
pub mod gps_dummy;
#[cfg(feature = "real")]
pub mod gps_real;

#[cfg(feature = "dummy")]
pub use servo_dummy as servo;
#[cfg(feature = "real")]
pub use servo_real as servo;

#[cfg(feature = "dummy")]
pub use picoubec_dummy as picoubec;
#[cfg(feature = "real")]
pub use picoubec_real as picoubec;

#[cfg(feature = "dummy")]
pub use gps_dummy as gps;
#[cfg(feature = "real")]
pub use gps_real as gps;

// LiDAR module (works with both dummy and real features)
pub mod lidar;
