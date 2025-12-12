use linux_embedded_hal::I2cdev;
use pwm_pca9685::{Address, Channel, Pca9685};
use std::io::{self, Write};

// Servo pulse width constants for 60Hz (prescale 100)
const SERVO_MIN: u16 = 246; // 0 degrees (1000µs)
const SERVO_MAX: u16 = 492; // 180 degrees (2000µs)

// Extended range for testing (±30° beyond normal limits)
// Allows going to extreme positions for testing mechanical limits
const SERVO_EXTENDED_MIN: u16 = 205; // ~-30 degrees (allows testing beyond 0°)
const SERVO_EXTENDED_MAX: u16 = 533; // ~210 degrees (allows testing beyond 180°)

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

fn print_help() {
    println!("\n=== Servo Centering Tool ===");
    println!("Commands:");
    println!("  ---      : Move left by 1 PWM unit (micro adjustment)");
    println!("  --       : Move left by 5 PWM units (fine adjustment)");
    println!("  -        : Move left by 20 PWM units (large adjustment)");
    println!("  +++      : Move right by 1 PWM unit (micro adjustment)");
    println!("  ++       : Move right by 5 PWM units (fine adjustment)");
    println!("  +        : Move right by 20 PWM units (large adjustment)");
    println!("  c        : Center servo at PWM 369 (~90°)");
    println!("  s [pwm]  : Set specific PWM value (205-533 extended range)");
    println!("  p [pin]  : Switch to different pin (0-15)");
    println!("  b [addr] : Switch PCA board (0x40 or 0x41)");
    println!("  h        : Show this help");
    println!("  q        : Quit");
    println!("\n📏 PWM Ranges:");
    println!("   Standard: 246 (0°) to 492 (180°)");
    println!("   Extended: 205 (~-30°) to 533 (~210°) - Use with caution!");
    println!("   Center: 369 (~90°)");
    println!("\n⚠️  WARNING: Extended range may damage servos if mechanical limits are hit!");
    println!("============================\n");
}

fn main() {
    println!("=== PCA9685 Servo Centering Tool ===");
    println!("This tool helps you find the center position for each servo.\n");

    // Ask for PCA board address
    print!("Enter PCA9685 I2C address (0x40 or 0x41) [default: 0x40]: ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let addr_str = input.trim();
    let mut board_addr = if addr_str.is_empty() {
        0x40
    } else {
        u8::from_str_radix(addr_str.trim_start_matches("0x"), 16).unwrap_or(0x40)
    };

    // Ask for pin number
    print!("Enter servo pin number (0-15): ");
    io::stdout().flush().unwrap();
    input.clear();
    io::stdin().read_line(&mut input).unwrap();
    let mut pin: u8 = input.trim().parse().unwrap_or(0);

    // Initialize PCA9685
    let mut pca = Pca9685::new(
        I2cdev::new("/dev/i2c-1").expect("Failed to open I2C device"),
        Address::from(board_addr),
    )
    .expect("Failed to initialize PCA9685");
    pca.set_prescale(100).expect("Failed to set prescale");
    pca.enable().expect("Failed to enable PCA9685");

    // Start at center position (PWM value, not angle)
    let mut current_pwm: u16 = 369; // Center position (~90°)
    let channel = pin_to_channel(pin);
    pca.set_channel_on(channel, 0)
        .expect("Failed to set channel on");
    pca.set_channel_off(channel, current_pwm)
        .expect("Failed to set channel off");

    println!(
        "\nInitialized PCA9685 at address 0x{:02X}, pin {}",
        board_addr, pin
    );
    println!("Starting at PWM 369 (center position, ~90°)");
    print_help();

    loop {
        // Calculate approximate angle for display only
        let approx_angle =
            ((current_pwm - SERVO_MIN) as f32 / (SERVO_MAX - SERVO_MIN) as f32) * 180.0;

        // Check if in extended range
        let range_indicator = if current_pwm < SERVO_MIN || current_pwm > SERVO_MAX {
            "⚠️ EXTENDED"
        } else {
            "✓"
        };

        println!("\n📍 Current Position:");
        println!(
            "   Board: 0x{:02X} | Pin: {} | PWM: {} | ~{:.1}° [{}]",
            board_addr, pin, current_pwm, approx_angle, range_indicator
        );
        print!("\nCommand: ");
        io::stdout().flush().unwrap();

        input.clear();
        io::stdin().read_line(&mut input).unwrap();
        let cmd = input.trim();

        let old_pwm = current_pwm;

        match cmd {
            "---" => current_pwm = current_pwm.saturating_sub(1),
            "--" => current_pwm = current_pwm.saturating_sub(5),
            "-" => current_pwm = current_pwm.saturating_sub(20),
            "+++" => current_pwm = current_pwm.saturating_add(1),
            "++" => current_pwm = current_pwm.saturating_add(5),
            "+" => current_pwm = current_pwm.saturating_add(20),
            "c" => current_pwm = 369,
            "h" => {
                print_help();
                continue;
            }
            "q" => {
                let final_angle =
                    ((current_pwm - SERVO_MIN) as f32 / (SERVO_MAX - SERVO_MIN) as f32) * 180.0;
                println!("\n📋 Final Position Summary:");
                println!("   Board: 0x{:02X}", board_addr);
                println!("   Pin: {}", pin);
                println!("   PWM: {} (record this value!)", current_pwm);
                println!("   Approx Angle: {:.2}°", final_angle);
                println!("\n💡 Use PWM value {} for calibration", current_pwm);
                println!("Goodbye!");
                break;
            }
            _ if cmd.starts_with("s ") => {
                let pwm_str = cmd.trim_start_matches("s ").trim();
                match pwm_str.parse::<u16>() {
                    Ok(pwm) if pwm >= SERVO_EXTENDED_MIN && pwm <= SERVO_EXTENDED_MAX => {
                        if pwm < SERVO_MIN || pwm > SERVO_MAX {
                            println!(
                                "⚠️  WARNING: Setting to extended range! Watch for mechanical limits!"
                            );
                        }
                        current_pwm = pwm;
                    }
                    _ => {
                        println!(
                            "❌ Invalid PWM. Use: s [{}-{}]",
                            SERVO_EXTENDED_MIN, SERVO_EXTENDED_MAX
                        );
                        println!("   Standard range: {}-{}", SERVO_MIN, SERVO_MAX);
                        continue;
                    }
                }
            }
            _ if cmd.starts_with("p ") => {
                let pin_str = cmd.trim_start_matches("p ").trim();
                match pin_str.parse::<u8>() {
                    Ok(new_pin) if new_pin < 16 => {
                        pin = new_pin;
                        println!("✓ Switched to pin {}", pin);
                        continue;
                    }
                    _ => {
                        println!("❌ Invalid pin. Use: p [0-15]");
                        continue;
                    }
                }
            }
            _ if cmd.starts_with("b ") => {
                let addr_str = cmd.trim_start_matches("b ").trim();
                match u8::from_str_radix(addr_str.trim_start_matches("0x"), 16) {
                    Ok(new_addr) if new_addr == 0x40 || new_addr == 0x41 => {
                        board_addr = new_addr;
                        // Reinitialize with new address
                        pca = Pca9685::new(
                            I2cdev::new("/dev/i2c-1").expect("Failed to open I2C device"),
                            Address::from(board_addr),
                        )
                        .expect("Failed to initialize PCA9685");
                        pca.set_prescale(100).expect("Failed to set prescale");
                        pca.enable().expect("Failed to enable PCA9685");
                        println!("✓ Switched to board 0x{:02X}", board_addr);
                        continue;
                    }
                    _ => {
                        println!("❌ Invalid address. Use: b [0x40 or 0x41]");
                        continue;
                    }
                }
            }
            "" => continue,
            _ => {
                println!("❌ Unknown command. Type 'h' for help.");
                continue;
            }
        }

        // Clamp PWM to extended valid range
        current_pwm = current_pwm.clamp(SERVO_EXTENDED_MIN, SERVO_EXTENDED_MAX);

        // Warn if entering extended range
        if (old_pwm >= SERVO_MIN && old_pwm <= SERVO_MAX)
            && (current_pwm < SERVO_MIN || current_pwm > SERVO_MAX)
        {
            println!("⚠️  WARNING: Entering extended range! Monitor servo for mechanical limits!");
        }

        // Update servo position
        let channel = pin_to_channel(pin);
        pca.set_channel_on(channel, 0)
            .expect("Failed to set channel on");
        pca.set_channel_off(channel, current_pwm)
            .expect("Failed to set channel off");

        // Show movement
        let diff = current_pwm as i32 - old_pwm as i32;
        if diff > 0 {
            println!("➡️  Moved right by {} PWM units", diff);
        } else if diff < 0 {
            println!("⬅️  Moved left by {} PWM units", diff.abs());
        }
    }
}
