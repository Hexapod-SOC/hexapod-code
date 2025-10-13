#[cfg(not(any(feature = "dummy", feature = "real")))]
compile_error!("You must enable either `dummy` or `real` feature for devices!");

#[cfg(feature = "dummy")]
pub mod servo_dummy;
#[cfg(feature = "real")]
pub mod servo_real;

#[cfg(feature = "dummy")]
pub use servo_dummy as servo;
#[cfg(feature = "real")]
pub use servo_real as servo;
