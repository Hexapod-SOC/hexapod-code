use glam::{Mat2, Vec3};

use crate::hexapod::LegId;

#[derive(Clone, Copy, Debug, Default)]
pub struct LegAngles {
    pub coxa: f32,
    pub femur: f32,
    pub tibia: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Constraints {
    pub coxa_length: f32,
    pub femur_length: f32,
    pub tibia_length: f32,
    pub coxa_soffset: f32,
    pub femur_soffset: f32,
    pub tibia_soffset: f32,
}

pub struct SimpleIk {
    c: Constraints,
}

impl SimpleIk {
    pub fn new(constraints: Constraints) -> Self {
        Self { c: constraints }
    }

    pub fn calc_pos_leg_angles(&self, leg: LegId, pos: Vec3) -> LegAngles {
        let mut pos = pos;
        let theta = 45_f32.to_radians();

        match leg {
            LegId::LeftFront | LegId::RightBack => {
                let rot = Mat2::from_angle(theta);
                let xy = rot.transpose() * glam::vec2(pos.x, pos.y);
                pos.x = xy.x;
                pos.y = xy.y;
            }
            LegId::LeftBack | LegId::RightFront => {
                let rot = Mat2::from_angle(-theta);
                let xy = rot.transpose() * glam::vec2(pos.x, pos.y);
                pos.x = xy.x;
                pos.y = xy.y;
            }
            _ => {}
        }

        let mut coxa_angle = pos.y.atan2(pos.x).to_degrees();
        let horizontal_dist = (pos.x.powi(2) + pos.y.powi(2)).sqrt() - self.c.coxa_length;
        let vertical_diag = (horizontal_dist.powi(2) + pos.z.powi(2)).sqrt();

        let femur_angle = (vertical_diag.powi(2) + self.c.femur_length.powi(2)
            - self.c.tibia_length.powi(2))
            / (2.0 * vertical_diag * self.c.femur_length);
        let femur_angle = femur_angle.acos().to_degrees()
            - (-pos.z).atan2(horizontal_dist).to_degrees();

        let tibia_angle = (self.c.femur_length.powi(2) + self.c.tibia_length.powi(2)
            - vertical_diag.powi(2))
            / (2.0 * self.c.femur_length * self.c.tibia_length);
        let tibia_angle = tibia_angle.acos().to_degrees();

        match leg {
            LegId::RightBack | LegId::RightFront | LegId::RightMiddle => {}
            LegId::LeftBack | LegId::LeftFront | LegId::LeftMiddle => coxa_angle = -coxa_angle,
        }

        LegAngles {
            coxa: (-coxa_angle) + 90.0 + self.c.coxa_soffset,
            femur: femur_angle + self.c.femur_soffset,
            tibia: tibia_angle + self.c.tibia_soffset,
        }
    }

    pub fn calc_pos_leg_angles_for_bevy(&self, leg: LegId, pos: Vec3) -> LegAngles {
        let mut angles = self.calc_pos_leg_angles(leg, pos);
        match leg {
            LegId::LeftBack | LegId::RightBack => angles.coxa = angles.coxa + 45.0,
            LegId::LeftFront | LegId::RightFront => angles.coxa = angles.coxa - 45.0,
            _ => {}
        }

        LegAngles {
            coxa: -angles.coxa,
            femur: angles.femur,
            tibia: angles.tibia,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct JointAngles {
    pub coxa: f32,
    pub femur: f32,
    pub tibia: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Geometry {
    pub coxa_len: f32,
    pub femur_len: f32,
    pub tibia_len: f32,
    pub coxa_attach_angle: f32,
    pub femur_attach_angle: f32,
    pub tibia_attach_angle: f32,
}

fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

pub fn ik_solve_leg(geom: Geometry, foot: Vec3) -> JointAngles {
    let coxa_angle = foot.y.atan2(foot.x).to_degrees();
    let horizontal = (foot.x * foot.x + foot.y * foot.y).sqrt() - geom.coxa_len;
    let vertical = foot.z;

    let l1 = geom.femur_len;
    let l2 = geom.tibia_len;
    let dist = (horizontal * horizontal + vertical * vertical).sqrt();

    let cos_knee = clamp((l1 * l1 + l2 * l2 - dist * dist) / (2.0 * l1 * l2), -1.0, 1.0);
    let knee = (std::f32::consts::PI - cos_knee.acos()).to_degrees();

    let cos_shoulder = clamp((l1 * l1 + dist * dist - l2 * l2) / (2.0 * l1 * dist.max(1e-6)), -1.0, 1.0);
    let shoulder = (vertical.atan2(horizontal) + cos_shoulder.acos()).to_degrees();

    JointAngles {
        coxa: coxa_angle + geom.coxa_attach_angle,
        femur: shoulder + geom.femur_attach_angle,
        tibia: knee + geom.tibia_attach_angle,
    }
}
