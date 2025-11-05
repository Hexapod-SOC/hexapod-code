use linux_embedded_hal::I2cdev;
use pwm_pca9685::{Address, Channel, Pca9685};
use std::env;
use std::thread;
use std::time::Duration;

// Servo pulse width constants for 60Hz (prescale 100)
const SERVO_MIN: u16 = 246; // 0 degrees (1000µs)
const SERVO_MAX: u16 = 492; // 180 degrees (2000µs)

/// Convert angle (0-180 degrees) to PWM value (246-492)
fn angle_to_pwm(angle: f32) -> u16 {
    let angle = angle.clamp(0.0, 180.0);
    let range = (SERVO_MAX - SERVO_MIN) as f32;
    let pwm = SERVO_MIN as f32 + (angle / 180.0) * range;
    pwm as u16
}

/// Convert pin number to PCA9685 Channel
fn pin_to_channel(pin: u8) -> Channel {
    match pin {
        0 => Channel::C0,
        1 => Channel::C1,
        2 => Channel::C2,
        3 => Channel::C3,
        4 => Channel::C4,
        5 => Channel::C5,
        6 => Channel::C6,
        7 => Channel::C7,
        8 => Channel::C8,
        9 => Channel::C9,
        10 => Channel::C10,
        11 => Channel::C11,
        12 => Channel::C12,
        13 => Channel::C13,
        14 => Channel::C14,
        15 => Channel::C15,
        _ => panic!("Invalid pin number: {}", pin),
    }
}

/// Set all channels on a PCA board to a specific PWM value
fn set_all_channels(pca: &mut Pca9685<I2cdev>, pwm_value: u16) {
    for pin in 0..16 {
        let channel = pin_to_channel(pin);
        pca.set_channel_on(channel, 0).expect("Failed to set channel on");
        pca.set_channel_off(channel, pwm_value).expect("Failed to set channel off");
    }
}

fn main() {
    println!("=== Servo Patch - Boot-time Servo Initialization ===");
    println!("This tool sets all servos to a safe position on system startup.");
    println!("Purpose: Prevent MG996R servos from overheating at null angle.\n");

    // Read configuration from environment variables
    let default_angle = env::var("SERVO_PATCH_ANGLE")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(90.0);

    let default_pwm = env::var("SERVO_PATCH_PWM")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or_else(|| angle_to_pwm(default_angle));

    let left_board_addr = env::var("SERVO_PATCH_LEFT_ADDR")
        .ok()
        .and_then(|s| u8::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x40);

    let right_board_addr = env::var("SERVO_PATCH_RIGHT_ADDR")
        .ok()
        .and_then(|s| u8::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x41);

    let enable_left = env::var("SERVO_PATCH_ENABLE_LEFT")
        .ok()
        .map(|s| s == "1" || s.to_lowercase() == "true")
        .unwrap_or(true);

    let enable_right = env::var("SERVO_PATCH_ENABLE_RIGHT")
        .ok()
        .map(|s| s == "1" || s.to_lowercase() == "true")
        .unwrap_or(true);

    let delay_ms = env::var("SERVO_PATCH_DELAY_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(100);

    // Display configuration
    println!("Configuration:");
    println!("  PWM Value: {} (~{:.1}°)", default_pwm, 
             ((default_pwm - SERVO_MIN) as f32 / (SERVO_MAX - SERVO_MIN) as f32) * 180.0);
    println!("  Left Board: 0x{:02X} ({})", left_board_addr, if enable_left { "enabled" } else { "disabled" });
    println!("  Right Board: 0x{:02X} ({})", right_board_addr, if enable_right { "enabled" } else { "disabled" });
    println!("  Delay: {}ms between boards", delay_ms);
    println!();

    // Initialize and set left board
    if enable_left {
        match I2cdev::new("/dev/i2c-1") {
            Ok(i2c) => {
                match Pca9685::new(i2c, Address::from(left_board_addr)) {
                    Ok(mut pca_left) => {
                        println!("✓ Initializing left board (0x{:02X})...", left_board_addr);
                        pca_left.set_prescale(100).expect("Failed to set prescale");
                        pca_left.enable().expect("Failed to enable PCA9685");
                        
                        set_all_channels(&mut pca_left, default_pwm);
                        println!("✓ Set all channels on left board to PWM {}", default_pwm);
                        
                        if delay_ms > 0 {
                            thread::sleep(Duration::from_millis(delay_ms));
                        }
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to initialize left board: {:?}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("✗ Failed to open I2C device for left board: {:?}", e);
            }
        }
    } else {
        println!("⊘ Left board disabled");
    }

    // Initialize and set right board
    if enable_right {
        match I2cdev::new("/dev/i2c-1") {
            Ok(i2c) => {
                match Pca9685::new(i2c, Address::from(right_board_addr)) {
                    Ok(mut pca_right) => {
                        println!("✓ Initializing right board (0x{:02X})...", right_board_addr);
                        pca_right.set_prescale(100).expect("Failed to set prescale");
                        pca_right.enable().expect("Failed to enable PCA9685");
                        
                        set_all_channels(&mut pca_right, default_pwm);
                        println!("✓ Set all channels on right board to PWM {}", default_pwm);
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to initialize right board: {:?}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("✗ Failed to open I2C device for right board: {:?}", e);
            }
        }
    } else {
        println!("⊘ Right board disabled");
    }

    println!("\n✓ Servo patch complete!");
    println!("All servos are now in safe position.");
}
