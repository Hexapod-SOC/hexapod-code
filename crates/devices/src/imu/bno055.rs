use super::{Imu, ImuData};
use anyhow::{anyhow, Result};
use glam::{Quat, Vec3};
use linux_embedded_hal::I2cdev;
use std::thread;
use std::time::Duration;

// Register definitions
const BNO055_ID: u8 = 0xA0;
const BNO055_CHIP_ID_ADDR: u8 = 0x00;
const BNO055_OPR_MODE_ADDR: u8 = 0x3D;
const BNO055_UNIT_SEL_ADDR: u8 = 0x3B;
const BNO055_CALIB_STAT_ADDR: u8 = 0x35;
const BNO055_SYS_TRIGGER_ADDR: u8 = 0x3F;

// Data registers (LSB first)
const BNO055_EULER_H_LSB_ADDR: u8 = 0x1A; // Heading (Yaw)
const BNO055_QUATERNION_DATA_W_LSB_ADDR: u8 = 0x20;

// Operation modes
const OPERATION_MODE_CONFIG: u8 = 0x00;
const OPERATION_MODE_NDOF: u8 = 0x0C;

pub struct Bno055 {
    i2c: I2cdev,
    address: u16,
}

impl Bno055 {
    pub fn new(bus: u8, address: u16) -> Result<Self> {
        let path = format!("/dev/i2c-{}", bus);
        let i2c = I2cdev::new(&path).map_err(|e| anyhow!("Failed to open I2C bus {}: {}", path, e))?;
        
        // Use i2c.set_slave_address(address) if required by the crate, 
        // but typically write/read take address or I2cdev handles it if configured.
        // linux-embedded-hal's I2cdev usually implements I2c from embedded-hal.
        // But here we might just use direct methods or standard I2C.
        // Let's assume standard embedded-hal traits are not easily available without importing them.
        // We'll use the impl specific methods if available, or just standard I2C traits.
        // Actually, linux-embedded-hal 0.4 implements embedded-hal 0.2/1.0 traits.
        // To be safe, let's use linux-embedded-hal specific methods or use the trait.
        
        let mut sensor = Self { i2c, address };
        sensor.init()?;
        Ok(sensor)
    }

    fn init(&mut self) -> Result<()> {
        // I2cdev requires setting slave address usually
        self.i2c.set_slave_address(self.address)
            .map_err(|e| anyhow!("Failed to set I2C address: {}", e))?;

        // Check Chip ID
        let id = self.read_reg(BNO055_CHIP_ID_ADDR)?;
        if id != BNO055_ID {
            return Err(anyhow!("Invalid BNO055 Chip ID: {:#02x} (expected {:#02x})", id, BNO055_ID));
        }

        // Set Config Mode
        self.write_reg(BNO055_OPR_MODE_ADDR, OPERATION_MODE_CONFIG)?;
        thread::sleep(Duration::from_millis(25));

        // Reset (optional, keeping it simple for now)

        // Set Unit Selection (Orientation = Android/Windows which is Heading/Pitch/Roll?)
        // bit 7: Orient (0=Windows), bit 4: Temp (0=C), bit 2: Euler (0=Deg), bit 1: Gyro (0=Dps), bit 0: Accel (0=m/s2)
        // 0x00 = Windows, Celsius, Degrees, Dps, m/s2
        self.write_reg(BNO055_UNIT_SEL_ADDR, 0x00)?;

        // Set Operation Mode to NDOF
        self.write_reg(BNO055_OPR_MODE_ADDR, OPERATION_MODE_NDOF)?;
        thread::sleep(Duration::from_millis(25));

        Ok(())
    }

    fn write_reg(&mut self, reg: u8, value: u8) -> Result<()> {
        self.i2c.write(&[reg, value])
            .map_err(|e| anyhow!("I2C write error: {}", e))
    }

    fn read_reg(&mut self, reg: u8) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.i2c.write(&[reg])
            .map_err(|e| anyhow!("I2C write error during read: {}", e))?;
        self.i2c.read(&mut buf)
            .map_err(|e| anyhow!("I2C read error: {}", e))?;
        Ok(buf[0])
    }
    
    fn read_start_regs(&mut self, start_reg: u8, count: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; count];
        self.i2c.write(&[start_reg])
             .map_err(|e| anyhow!("I2C write error during burst read: {}", e))?;
        self.i2c.read(&mut buf)
             .map_err(|e| anyhow!("I2C read error during burst read: {}", e))?;
        Ok(buf)
    }
}

impl Imu for Bno055 {
    fn read_data(&mut self) -> Result<ImuData> {
        let calib = self.get_calibration_status()?;
        
        // Read Euler angles (6 bytes: Heading LSB/MSB, Roll LSB/MSB, Pitch LSB/MSB)
        // Note: BNO055 output order depends on config. Typical is Heading (Psi), Roll (Phi), Pitch (Theta)
        // Windows/Android mode might differ.
        // Default (0x00 unit sel): Heading = Z, Roll = Y, Pitch = X? No.
        // Docs say: H, R, P.
        // Let's read 6 bytes starting from BNO055_EULER_H_LSB_ADDR
        let euler_bytes = self.read_start_regs(BNO055_EULER_H_LSB_ADDR, 6)?;
        
        let h = i16::from_le_bytes([euler_bytes[0], euler_bytes[1]]);
        let r = i16::from_le_bytes([euler_bytes[2], euler_bytes[3]]);
        let p = i16::from_le_bytes([euler_bytes[4], euler_bytes[5]]);
        
        // Scale is 16 LSB = 1 degree by default? No, 1 degree = 16 LSB.
        // Wait, default is 16 LSB = 1 Degree. 
        let scale = 16.0;
        let heading = h as f32 / scale;
        let roll = r as f32 / scale;
        let pitch = p as f32 / scale;
        
        // Read Quaternion (8 bytes: W, X, Y, Z) - Wait, registers are W, X, Y, Z
        let quat_bytes = self.read_start_regs(BNO055_QUATERNION_DATA_W_LSB_ADDR, 8)?;
        let w = i16::from_le_bytes([quat_bytes[0], quat_bytes[1]]);
        let x = i16::from_le_bytes([quat_bytes[2], quat_bytes[3]]);
        let y = i16::from_le_bytes([quat_bytes[4], quat_bytes[5]]);
        let z = i16::from_le_bytes([quat_bytes[6], quat_bytes[7]]);
        
        // 1 Quaternion = 2^14 LSB = 16384
        let q_scale = 16384.0;
        let quat = Quat::from_xyzw(
            x as f32 / q_scale,
            y as f32 / q_scale,
            z as f32 / q_scale,
            w as f32 / q_scale,
        );

        Ok(ImuData {
            euler: Vec3::new(roll, pitch, heading),
            quat,
            calibration: calib.0, // Just using Sys calib for overall status
        })
    }

    fn get_calibration_status(&mut self) -> Result<(u8, u8, u8, u8)> {
        let val = self.read_reg(BNO055_CALIB_STAT_ADDR)?;
        let sys = (val >> 6) & 0x03;
        let gyr = (val >> 4) & 0x03;
        let acc = (val >> 2) & 0x03;
        let mag = val & 0x03;
        Ok((sys, gyr, acc, mag))
    }
}
