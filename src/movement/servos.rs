use crate::movement::{Leg, LegPart, LegAngles, ServoPins};
use linux_embedded_hal::I2cdev;
use pwm_pca9685::{Address, Channel, Pca9685};

// Servo pulse width constants for 60Hz (prescale 100)
// MG996R servos: 1000µs (1ms) = 0°, 1500µs (1.5ms) = 90°, 2000µs (2ms) = 180°
const SERVO_MIN: u16 = 246;     // 0 degrees (1000µs)
const SERVO_CENTER: u16 = 369;  // 90 degrees (1500µs) 
const SERVO_MAX: u16 = 492;     // 180 degrees (2000µs)

pub struct ServoController {
    pca_left: Pca9685<I2cdev>,
    pca_right: Pca9685<I2cdev>,
    servo_pins: ServoPins,
}

impl ServoController {
    pub fn new(servo_pins: ServoPins) -> Self {
        let mut servos_controller = ServoController {
            pca_left: Pca9685::new(I2cdev::new("/dev/i2c-1").unwrap(), Address::from(0x40)).unwrap(),
            pca_right: Pca9685::new(I2cdev::new("/dev/i2c-1").unwrap(), Address::from(0x41)).unwrap(),
            servo_pins,
        };
        servos_controller.init_servos();
        servos_controller.set_all_legs_to_angles(90.0, 50.0, 50.0); // Default position
        servos_controller
    }

    pub fn init_servos(&mut self) {
        self.pca_left.set_prescale(100).unwrap();
        self.pca_right.set_prescale(100).unwrap();

        // It is necessary to enable the device.
        self.pca_left.enable().unwrap();
        self.pca_right.enable().unwrap();
    }

    /// Set a single servo to a specific angle (0-180 degrees)
    pub fn set_servo_angle(&mut self, leg: Leg, part: LegPart, angle: f32) {
        // Convert angle (0-180 degrees) to PWM value
        let pwm_value = self.angle_to_pwm(angle);
        
        // Get the pin number for this leg and part
        let pin = self.get_pin(leg, part);
        let channel = self.pin_to_channel(pin);
        
        // Determine which PCA to use and if we need to invert
        match leg {
            Leg::LeftFront | Leg::LeftMiddle | Leg::LeftBack => {
                self.pca_left.set_channel_on(channel, 0).unwrap();
                self.pca_left.set_channel_off(channel, pwm_value).unwrap();
            },
            Leg::RightFront | Leg::RightMiddle | Leg::RightBack => {
                // Invert angle for right side (mirrored servos)
                let final_pwm = SERVO_MIN + SERVO_MAX - pwm_value;
                self.pca_right.set_channel_on(channel, 0).unwrap();
                self.pca_right.set_channel_off(channel, final_pwm).unwrap();
            },
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
    
    /// Convert angle (0-180 degrees) to PWM value (246-492)
    fn angle_to_pwm(&self, angle: f32) -> u16 {
        // Clamp angle to 0-180 range
        let angle = angle.clamp(0.0, 180.0);
        
        // Linear interpolation between SERVO_MIN and SERVO_MAX
        let range = (SERVO_MAX - SERVO_MIN) as f32;
        let pwm = SERVO_MIN as f32 + (angle / 180.0) * range;
        
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
}
