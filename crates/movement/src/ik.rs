use crate::legs::{Leg, LegAngles};
use glam;

pub struct Constraints {
    pub coxa_length: f32,
    pub femur_length: f32,
    pub tibia_length: f32,
    pub coxa_soffset: f32,
    pub femur_soffset: f32,
    pub tibia_soffset: f32,
}

pub struct SimpleIK {
    c: Constraints,
}

impl SimpleIK {
    pub fn new(constraints: Constraints) -> Self {
        SimpleIK { c: constraints }
    }

    pub fn calculate_leg_angles(&self, leg: Leg, pos: glam::Vec3) -> LegAngles {
        let mut coxa_angle = (pos.x / pos.z).atan().to_degrees();
        let horizontal_dist = (pos.x.powi(2) + pos.z.powi(2)).sqrt() + self.c.coxa_length; // - self.constraints.coxa_length;
        let vertical_diag = (horizontal_dist.powi(2) + pos.y.powi(2)).sqrt();
        
        let femur_angle = (vertical_diag.powi(2) + self.c.femur_length.powi(2) - self.c.tibia_length.powi(2))
            / (2.0 * vertical_diag * self.c.femur_length);
        let femur_angle = femur_angle.acos().to_degrees() - pos.y.atan2(horizontal_dist).to_degrees().abs();

        let tibia_angle = (self.c.femur_length.powi(2) + self.c.tibia_length.powi(2) - vertical_diag.powi(2))
            / (2.0 * self.c.femur_length * self.c.tibia_length);
        let tibia_angle = tibia_angle.acos().to_degrees();

        match leg {
            Leg::LeftBack | Leg::RightBack => coxa_angle = coxa_angle - 45.0,
            Leg::LeftFront | Leg::RightFront => coxa_angle = coxa_angle + 45.0,
            _ => {}
        }

        //println!("IK Debug - Pos: {:?}, Coxa Angle: {:.2}, Femur Angle: {:.2}, Tibia Angle: {:.2}", pos, coxa_angle, femur_angle, tibia_angle);
        LegAngles {
            coxa:  -coxa_angle - self.c.coxa_soffset,
            femur: femur_angle - self.c.femur_soffset,
            tibia: tibia_angle - self.c.tibia_soffset,
        }
    }
}