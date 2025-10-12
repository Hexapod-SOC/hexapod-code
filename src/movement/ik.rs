use crate::movement::{Leg, LegAngles};
use glam;

const COXA_LENGTH:  f32 = 43.0;  // Length of the coxa segment in mm
const FEMUR_LENGTH: f32 = 60.0; // Length of the femur segment in mm
const TIBIA_LENGTH: f32 = 104.0; // Length of the tibia segment in mm

const COXA_SOFFSET:  f32 = -90.0; // Offset to align coxa angle to 0 degrees forward
const FEMUR_SOFFSET: f32 = -83.0; // Offset to align femur angle to horizontal
const TIBIA_SOFFSET: f32 =  35.0; // Offset to align tibia angle to straight down

pub struct SimpleIK;

impl SimpleIK {
    pub fn new() -> Self {
        SimpleIK
    }

    pub fn calculate_leg_angles(&self, leg: Leg, pos: glam::Vec3) -> LegAngles {
        let mut coxa_angle = (pos.x / pos.z).atan().to_degrees();
        let horizontal_dist = (pos.x.powi(2) + pos.z.powi(2)).sqrt() + COXA_LENGTH; // - COXA_LENGTH;
        let vertical_diag = (horizontal_dist.powi(2) + pos.y.powi(2)).sqrt();
        
        let femur_angle = (vertical_diag.powi(2) + FEMUR_LENGTH.powi(2) - TIBIA_LENGTH.powi(2))
            / (2.0 * vertical_diag * FEMUR_LENGTH);
        let femur_angle = femur_angle.acos().to_degrees() - pos.y.atan2(horizontal_dist).to_degrees().abs();

        let tibia_angle = (FEMUR_LENGTH.powi(2) + TIBIA_LENGTH.powi(2) - vertical_diag.powi(2))
            / (2.0 * FEMUR_LENGTH * TIBIA_LENGTH);
        let tibia_angle = tibia_angle.acos().to_degrees();

        match leg {
            Leg::LeftBack | Leg::RightBack => coxa_angle = coxa_angle - 45.0,
            Leg::LeftFront | Leg::RightFront => coxa_angle = coxa_angle + 45.0,
            _ => {}
        }

        println!("IK Debug - Pos: {:?}, Coxa Angle: {:.2}, Femur Angle: {:.2}, Tibia Angle: {:.2}", pos, coxa_angle, femur_angle, tibia_angle);
        LegAngles {
            coxa:  -coxa_angle  - COXA_SOFFSET,
            femur: femur_angle - FEMUR_SOFFSET,
            tibia: tibia_angle - TIBIA_SOFFSET,
        }
    }
}