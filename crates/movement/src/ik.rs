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

    pub fn calc_pos_leg_angles(&self, leg: Leg, pos: glam::Vec3) -> LegAngles {
let mut pos = pos;

// Servo mount correction: rotate world target into local servo space
let theta = 45_f32.to_radians(); // 45° mount angle

match leg {
    // Legs whose servos are rotated +45° relative to body frame
    Leg::LeftFront | Leg::RightBack => {
        let rot = glam::Mat2::from_angle(theta);
        let xy = rot.transpose() * glam::vec2(pos.x, pos.y);
        pos.x = xy.x;
        pos.y = xy.y;
    }
    // Legs whose servos are rotated -45° relative to body frame
    Leg::LeftBack | Leg::RightFront => {
        let rot = glam::Mat2::from_angle(-theta);
        let xy = rot.transpose() * glam::vec2(pos.x, pos.y);
        pos.x = xy.x;
        pos.y = xy.y;
    }
    _ => {}
}

        // Coxa angle: rotation in horizontal plane (XY plane)
        // atan2(Y, X) gives angle from X-axis toward Y-axis
        let mut coxa_angle = pos.y.atan2(pos.x).to_degrees();
        
        // Horizontal distance from body center (after coxa joint)
        let horizontal_dist = (pos.x.powi(2) + pos.y.powi(2)).sqrt() - self.c.coxa_length;
        
        // Diagonal distance in the vertical plane (combining horizontal reach and vertical height)
        let vertical_diag = (horizontal_dist.powi(2) + pos.z.powi(2)).sqrt();
        
        // Femur angle: using law of cosines in the vertical plane
        let femur_angle = (vertical_diag.powi(2) + self.c.femur_length.powi(2) - self.c.tibia_length.powi(2))
            / (2.0 * vertical_diag * self.c.femur_length);
        // Adjust for the angle of the diagonal relative to horizontal
        // Z is down (negative), so we use -pos.z for the calculation
        let femur_angle = femur_angle.acos().to_degrees() - (-pos.z).atan2(horizontal_dist).to_degrees();

        // Tibia angle: using law of cosines
        let tibia_angle = (self.c.femur_length.powi(2) + self.c.tibia_length.powi(2) - vertical_diag.powi(2))
            / (2.0 * self.c.femur_length * self.c.tibia_length);
        let tibia_angle = tibia_angle.acos().to_degrees();
        
        //println!("IK Debug - Leg: {:?}, Pos: {:?}, Coxa Angle: {:.2}, Femur Angle: {:.2}, Tibia Angle: {:.2}", leg, pos, coxa_angle, femur_angle, tibia_angle);

/*        match leg {
            Leg::LeftFront | Leg::RightBack => coxa_angle = coxa_angle - 45.0,
            Leg::LeftBack | Leg::RightFront => coxa_angle = coxa_angle + 45.0,
            _ => {}
        } */
        match leg {
            Leg::RightBack | Leg::RightFront | Leg::RightMiddle => {}
            Leg::LeftBack | Leg::LeftFront | Leg::LeftMiddle => coxa_angle = -coxa_angle,
        }

        LegAngles {
            coxa:  (-coxa_angle) + 90.0 + self.c.coxa_soffset,
            femur: femur_angle + self.c.femur_soffset,
            tibia: tibia_angle + self.c.tibia_soffset,
        }
    }

    pub fn calc_pos_leg_angles_forbevysim(&self, leg: Leg, pos: glam::Vec3) -> LegAngles {
        let mut angles = self.calc_pos_leg_angles(leg, pos);
        match leg {
            Leg::LeftBack | Leg::RightBack => angles.coxa = angles.coxa + 45.0,
            Leg::LeftFront | Leg::RightFront => angles.coxa = angles.coxa - 45.0,
            _ => {}
        }

        LegAngles {
            coxa: -angles.coxa,
            femur: angles.femur,
            tibia: angles.tibia,
        }
    }
}