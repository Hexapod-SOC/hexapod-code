use linux_embedded_hal::I2cdev;
use movement::legs::{Leg, LegAngles, LegPart};
use pwm_pca9685::{Address, Channel, Pca9685};

// Servo pulse width constants for 60Hz (prescale 100)
// MG996R servos: 1000µs (1ms) = 0°, 1500µs (1.5ms) = 90°, 2000µs (2ms) = 180°
// const SERVO_MIN: u16 = 246; // 0 degrees (1000µs)
// #[allow(dead_code)]
// const SERVO_CENTER: u16 = 369; // 90 degrees (1500µs) 
// const SERVO_MAX: u16 = 492; // 180 degrees (2000µs)

// for prescale 121 (50Hz)
// Safer range: 1.0ms..2.0ms to reduce over-travel risk.
const SERVO_MIN: u16 = 204; // 0 degrees in safe range (1000µs)
#[allow(dead_code)]
const SERVO_CENTER: u16 = 307; // 90 degrees (1500µs) 
const SERVO_MAX: u16 = 410; // 180 degrees in safe range (2000µs)
const SERVO_ANGLE_MIN: f32 = -30.0;
const SERVO_ANGLE_MAX: f32 = 210.0;


/// (Coxa, Femur, Tibia) pin configuration for each leg
pub struct ServoPins {
    pub left_front: (u8, u8, u8), // (Coxa, Femur, Tibia)
    pub left_middle: (u8, u8, u8),
    pub left_back: (u8, u8, u8),
    pub right_front: (u8, u8, u8),
    pub right_middle: (u8, u8, u8),
    pub right_back: (u8, u8, u8),
}

/// (Coxa, Femur, Tibia) PWA offsets for each leg servo
/// These offsets are measured in PWA units relative to 307 PWA (center position)
/// WARNING: All servos were measured in one configuration. Left/right side servos
/// are physically reversed/mirrored, so these offsets may need inversion for right side.
pub struct ServoOffsets {
    pub left_front: (f32, f32, f32), // (Coxa, Femur, Tibia) in PWA units
    pub left_middle: (f32, f32, f32),
    pub left_back: (f32, f32, f32),
    pub right_front: (f32, f32, f32),
    pub right_middle: (f32, f32, f32),
    pub right_back: (f32, f32, f32),
}

pub struct ServoController {
    pca_left: Pca9685<I2cdev>,
    pca_right: Pca9685<I2cdev>,
    servo_pins: ServoPins,
    servo_offsets: ServoOffsets,
    // Cache last PWM we sent per channel to avoid re-sending identical pulses,
    // which can create audible/visible twitch on some servos.
    last_pwm_left: [Option<u16>; 16],
    last_pwm_right: [Option<u16>; 16],
}

impl ServoController {
    pub fn new(servo_pins: ServoPins, servo_offsets: ServoOffsets) -> Self {
        let mut servos_controller = ServoController {
            pca_left: Pca9685::new(I2cdev::new("/dev/i2c-1").unwrap(), Address::from(0x41))
                .unwrap(),
            pca_right: Pca9685::new(I2cdev::new("/dev/i2c-1").unwrap(), Address::from(0x40))
                .unwrap(),
            servo_pins,
            servo_offsets,
            last_pwm_left: [None; 16],
            last_pwm_right: [None; 16],
        };
        servos_controller.init_servos();
        servos_controller.set_all_legs_to_angles(90.0, 50.0, 50.0); // Default position
        servos_controller
    }

    pub fn init_servos(&mut self) {
        self.pca_left.set_prescale(121).unwrap();
        self.pca_right.set_prescale(121).unwrap();

        // It is necessary to enable the device.
        self.pca_left.enable().unwrap();
        self.pca_right.enable().unwrap();
    }

    /// Set a single servo to a specific angle (expanded range)
    pub fn set_servo_angle(&mut self, leg: Leg, part: LegPart, angle: f32) {
        // Convert angle (0-180 degrees) to PWM value
        let pwm_value = self.angle_to_pwm(angle);
        self.set_servo_pwm(leg, part, pwm_value);
    }

    /// Set a single servo using raw PWM value (246-492)
    /// This bypasses angle conversion for precise control from calibration data
    pub fn set_servo_pwm(&mut self, leg: Leg, part: LegPart, pwm_value: u16) {
        // Get the pin number for this leg and part
        let pin = self.get_pin(leg, part);
        let channel = self.pin_to_channel(pin);

        // Apply servo offset to shift the operating range
        let offset = self.get_offset(leg, part);
        let adjusted_pwm =
            (pwm_value as f32 + offset).clamp(SERVO_MIN as f32, SERVO_MAX as f32) as u16;

        // Determine which PCA to use and if we need to invert
        // Avoid sending duplicate PWM to reduce jitter
        match leg {
            Leg::LeftFront | Leg::LeftMiddle | Leg::LeftBack => {
                if self.should_skip_pwm(true, pin, adjusted_pwm) {
                    return;
                }
                self.pca_left.set_channel_on(channel, 0).unwrap();
                self.pca_left
                    .set_channel_off(channel, adjusted_pwm)
                    .unwrap();
            }
            Leg::RightFront | Leg::RightMiddle | Leg::RightBack => {
                // Invert PWM for right side (mirrored servos)
                let final_pwm = SERVO_MIN + SERVO_MAX - adjusted_pwm;
                if self.should_skip_pwm(false, pin, final_pwm) {
                    return;
                }
                self.pca_right.set_channel_on(channel, 0).unwrap();
                self.pca_right.set_channel_off(channel, final_pwm).unwrap();
            }
        };
    }

    /// Set all three servos for a leg
    pub fn set_leg_angles(&mut self, leg: Leg, angles: LegAngles) {
        self.set_servo_angle(leg, LegPart::Coxa, angles.coxa);
        self.set_servo_angle(leg, LegPart::Femur, angles.femur);
        self.set_servo_angle(leg, LegPart::Tibia, angles.tibia);
    }

    /// Set all legs to the same angles (coxa, femur, tibia)
    pub fn set_all_legs_to_angles(&mut self, coxa: f32, femur: f32, tibia: f32) {
        let angles = LegAngles { coxa, femur, tibia };

        self.set_leg_angles(Leg::LeftFront, angles);
        self.set_leg_angles(Leg::LeftMiddle, angles);
        self.set_leg_angles(Leg::LeftBack, angles);
        self.set_leg_angles(Leg::RightFront, angles);
        self.set_leg_angles(Leg::RightMiddle, angles);
        self.set_leg_angles(Leg::RightBack, angles);
    }

    /// Convert angle (expanded range) to PWM value (SERVO_MIN..SERVO_MAX)
    fn angle_to_pwm(&self, angle: f32) -> u16 {
        // Clamp angle to expanded range
        let angle = angle.clamp(SERVO_ANGLE_MIN, SERVO_ANGLE_MAX);

        // Linear interpolation between SERVO_MIN and SERVO_MAX
        let range = (SERVO_MAX - SERVO_MIN) as f32;
        let normalized = (angle - SERVO_ANGLE_MIN) / (SERVO_ANGLE_MAX - SERVO_ANGLE_MIN);
        let pwm = SERVO_MIN as f32 + normalized * range;

        pwm as u16
    }

    /// Get the pin number for a specific leg part
    fn get_pin(&self, leg: Leg, part: LegPart) -> u8 {
        let pins = match leg {
            Leg::LeftFront => self.servo_pins.left_front,
            Leg::LeftMiddle => self.servo_pins.left_middle,
            Leg::LeftBack => self.servo_pins.left_back,
            Leg::RightFront => self.servo_pins.right_front,
            Leg::RightMiddle => self.servo_pins.right_middle,
            Leg::RightBack => self.servo_pins.right_back,
        };

        match part {
            LegPart::Coxa => pins.0,
            LegPart::Femur => pins.1,
            LegPart::Tibia => pins.2,
        }
    }

    /// Get the PWA offset for a specific leg part
    fn get_offset(&self, leg: Leg, part: LegPart) -> f32 {
        let offsets = match leg {
            Leg::LeftFront => self.servo_offsets.left_front,
            Leg::LeftMiddle => self.servo_offsets.left_middle,
            Leg::LeftBack => self.servo_offsets.left_back,
            Leg::RightFront => self.servo_offsets.right_front,
            Leg::RightMiddle => self.servo_offsets.right_middle,
            Leg::RightBack => self.servo_offsets.right_back,
        };

        match part {
            LegPart::Coxa => offsets.0,
            LegPart::Femur => offsets.1,
            LegPart::Tibia => offsets.2,
        }
    }

    /// Convert pin number (0-15) to PCA9685 Channel
    fn pin_to_channel(&self, pin: u8) -> Channel {
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

    /// Track whether the PWM for a given channel already matches the target.
    /// This is a best-effort jitter reduction: we skip writes when unchanged.
    fn should_skip_pwm(&mut self, is_left: bool, pin: u8, target: u16) -> bool {
        let slot = if is_left {
            &mut self.last_pwm_left[pin as usize]
        } else {
            &mut self.last_pwm_right[pin as usize]
        };

        if let Some(prev) = slot {
            if *prev == target {
                return true;
            }
        }
        *slot = Some(target);
        false
    }
}
