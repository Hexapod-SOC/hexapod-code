
use anyhow::Result;
use glam::{Vec3, Quat};

#[cfg(feature = "real")]
pub mod bno055;
#[cfg(feature = "dummy")]
pub mod dummy;

#[derive(Debug, Clone, Copy, Default)]
pub struct ImuData {
    pub euler: Vec3, // Roll, Pitch, Yaw in degrees
    pub quat: Quat,
    pub calibration: u8, // 0-3 (3 = fully calibrated)
}

pub trait Imu: Send + Sync {
    /// Read current orientation data
    fn read_data(&mut self) -> Result<ImuData>;
    
    /// Get current calibration status (sys, gyro, accel, mag)
    fn get_calibration_status(&mut self) -> Result<(u8, u8, u8, u8)>;
}

