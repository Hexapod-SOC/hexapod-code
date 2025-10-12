use crate::movement::{ik, servos, Leg};

pub struct Movement {
    servo_controller: servos::ServoController,
    ik: ik::SimpleIK,
}

impl Movement {
    pub fn new(servo_controller: servos::ServoController, ik: ik::SimpleIK) -> Self {
        Movement { servo_controller, ik }
    }
    pub fn move_leg_to_position(&mut self, leg: Leg, position: glam::Vec3) {
        let angles = self.ik.calculate_leg_angles(leg, position);
        self.servo_controller.set_leg_angles(leg, angles);
    }
}