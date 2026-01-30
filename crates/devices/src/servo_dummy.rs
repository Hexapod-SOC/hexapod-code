use movement::legs::{Leg, LegAngles, LegPart};

/// (Coxa, Femur, Tibia) pin configuration for each leg
#[derive(Debug, Clone, Copy)]
pub struct ServoPins {
    pub left_front: (u8, u8, u8), // (Coxa, Femur, Tibia)
    pub left_middle: (u8, u8, u8),
    pub left_back: (u8, u8, u8),
    pub right_front: (u8, u8, u8),
    pub right_middle: (u8, u8, u8),
    pub right_back: (u8, u8, u8),
}

/// (Coxa, Femur, Tibia) PWA offsets for each leg servo
/// These offsets are measured in PWA units relative to 369 PWA (center position)
/// WARNING: All servos were measured in one configuration. Left/right side servos
/// are physically reversed/mirrored, so these offsets may need inversion for right side.
#[derive(Debug, Clone, Copy)]
pub struct ServoOffsets {
    pub left_front: (f32, f32, f32),  // (Coxa, Femur, Tibia) in PWA units
    pub left_middle: (f32, f32, f32),
    pub left_back: (f32, f32, f32),
    pub right_front: (f32, f32, f32),
    pub right_middle: (f32, f32, f32),
    pub right_back: (f32, f32, f32),
}

pub struct ServoController {
    servo_offsets: ServoOffsets,
}

impl ServoController {
    pub fn new(_servo_pins: ServoPins, servo_offsets: ServoOffsets) -> Self {
        let mut servos_controller = ServoController { servo_offsets };
        servos_controller.init_servos();
        servos_controller.set_all_legs_to_angles(90.0, 50.0, 50.0); // Default position
        servos_controller
    }

    pub fn init_servos(&mut self) {
        println!("(Dummy) Initializing servos...");
    }

    /// Set a single servo to a specific angle (0-180 degrees)
    pub fn set_servo_angle(&mut self, leg: Leg, part: LegPart, angle: f32) {
        println!("(Dummy) Setting {:?} {:?} to angle {:.2}", leg, part, angle);
    }

    /// Set all three servos for a leg
    pub fn set_leg_angles(&mut self, leg: Leg, angles: LegAngles) {
        println!(
            "(Dummy) Setting {:?} to angles: Coxa {:.2}, Femur {:.2}, Tibia {:.2}",
            leg, angles.coxa, angles.femur, angles.tibia
        );
    }

    /// Set all legs to the same angles (coxa, femur, tibia)
    pub fn set_all_legs_to_angles(&mut self, coxa: f32, femur: f32, tibia: f32) {
        println!(
            "(Dummy) Setting all legs to angles: Coxa {:.2}, Femur {:.2}, Tibia {:.2}",
            coxa, femur, tibia
        );
    }

    /// Update servo offsets (applies on next set_* call)
    pub fn set_offsets(&mut self, offsets: ServoOffsets) {
        self.servo_offsets = offsets;
    }

    /// Get current servo offsets
    pub fn get_offsets(&self) -> ServoOffsets {
        self.servo_offsets
    }
}
