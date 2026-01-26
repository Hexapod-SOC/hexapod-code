use super::{Imu, ImuData};
use anyhow::{anyhow, Result};
use bno055::{Bno055 as Bno055Driver, BNO055OperationMode};
use glam::{Quat, Vec3};
use linux_embedded_hal::{Delay, I2cdev};

pub struct Bno055 {
    driver: Bno055Driver<I2cdev>,
}

impl Bno055 {
    pub fn new(bus: u8, _address: u16) -> Result<Self> {
        let path = format!("/dev/i2c-{}", bus);
        let i2c = I2cdev::new(&path)
            .map_err(|e| anyhow!("Failed to open I2C bus {}: {}", path, e))?;
        
        // Bno055 driver initialization
        // Note: The crate might use default address (0x28 or 0x29).
        // If address is different, we check if we can pass it or if we rely on default.
        // Assuming we use default or the one passed (0x28 usually).
        // Typically BNO055 has 0x28 (COM3 low) or 0x29 (COM3 high).
        
        let mut driver = Bno055Driver::new(i2c);
        
        // Initialize
        driver.init(&mut Delay)
            .map_err(|e| anyhow!("Failed to init BNO055: {:?}", e))?;
            
        // Set mode to NDOF (Nine Degrees Of Freedom)
        driver.set_mode(BNO055OperationMode::NDOF, &mut Delay)
             .map_err(|e| anyhow!("Failed to set BNO055 mode: {:?}", e))?;
             
        Ok(Self { driver })
    }
}

impl Imu for Bno055 {
    fn read_data(&mut self) -> Result<ImuData> {
        // Read Euler angles
        let euler = self.driver.euler_angles()
            .map_err(|e| anyhow!("Failed to read euler angles: {:?}", e))?;
            
        // Read Quaternion
        let quat = self.driver.quaternion()
            .map_err(|e| anyhow!("Failed to read quaternion: {:?}", e))?;
            
        // Read Calibration Status
        let calib = self.driver.get_calibration_status()
            .map_err(|e| anyhow!("Failed to read calibration: {:?}", e))?;
            
        // Convert to Glam types
        // mint::EulerAngles has fields a, b, c. 
        // Based on bno055 crate source: from([roll, pitch, heading]) -> a=roll, b=pitch, c=heading
        // ImuData expects Roll, Pitch, Yaw
        
        Ok(ImuData {
            euler: Vec3::new(euler.a, euler.b, euler.c),
            
            // mint::Quaternion has v: Vector3{x,y,z} and s (scalar/w)
            quat: Quat::from_xyzw(quat.v.x, quat.v.y, quat.v.z, quat.s),
            calibration: calib.sys, // Overall system calibration
        })
    }

    fn get_calibration_status(&mut self) -> Result<(u8, u8, u8, u8)> {
        let calib = self.driver.get_calibration_status()
             .map_err(|e| anyhow!("Failed to read calibration: {:?}", e))?;
             
        Ok((calib.sys, calib.gyr, calib.acc, calib.mag))
    }
}
