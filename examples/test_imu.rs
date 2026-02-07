use devices::imu_driver;
use devices::imu::Imu;
use std::{thread, time::Duration};

const IMU_I2C_BUS: u8 = 1;
const IMU_I2C_ADR: u16 = 0x28;

fn main() -> anyhow::Result<()> {
    println!("Initializing IMU on Bus {} Address {:#x}...", IMU_I2C_BUS, IMU_I2C_ADR);
    
    // Note: This relies on feature configuration.
    // If run as `cargo run --example test_imu --features real`, it uses Bno055.
    // If run as default (dummy), it uses Dummy.
    
    let mut imu = imu_driver::new(IMU_I2C_BUS, IMU_I2C_ADR)?;
    
    println!("IMU Initialized!");
    println!("Reading data loop... (Ctrl+C to stop)");
    
    loop {
        match imu.read_data() {
            Ok(data) => {
                let calib = imu.get_calibration_status()?;
                println!(
                    "Euler: R:{:.2} P:{:.2} Y:{:.2} | Calib(S/G/A/M): {}/{}/{}/{}", 
                    data.euler.x, data.euler.y, data.euler.z,
                    calib.0, calib.1, calib.2, calib.3
                );
            }
            Err(e) => {
                eprintln!("Error reading IMU: {}", e);
            }
        }
        
        thread::sleep(Duration::from_millis(100));
    }
}
