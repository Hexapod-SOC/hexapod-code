use crate::movementA::ik;
use movement::legs::{Leg};
use devices::servo::ServoController;

pub struct Movement {
    servo_controller: ServoController,
    ik: ik::SimpleIK,
}

impl Movement {
    pub fn new(servo_controller: ServoController, ik: ik::SimpleIK) -> Self {
        Movement { servo_controller, ik }
    }
    pub fn move_leg_to_position(&mut self, leg: Leg, position: glam::Vec3) {
        let angles = self.ik.calculate_leg_angles(leg, position);
        self.servo_controller.set_leg_angles(leg, angles);
    }
}