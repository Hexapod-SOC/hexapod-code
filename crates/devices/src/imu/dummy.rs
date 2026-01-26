use crate::imu::{Imu, ImuData};
use anyhow::Result;
use glam::{Quat, Vec3};

pub struct Bno055Dummy;

impl Bno055Dummy {
    pub fn new(_bus: u8, _addr: u16) -> Result<Self> {
        Ok(Self)
    }
}

impl Imu for Bno055Dummy {
    fn read_data(&mut self) -> Result<ImuData> {
        // Return stable data (flat, looking forward)
        Ok(ImuData {
            euler: Vec3::ZERO,
            quat: Quat::IDENTITY,
            calibration: 3,
        })
    }

    fn get_calibration_status(&mut self) -> Result<(u8, u8, u8, u8)> {
        Ok((3, 3, 3, 3))
    }
}
