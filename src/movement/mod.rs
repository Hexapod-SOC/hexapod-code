pub mod ik;
pub mod servos;
pub mod movement;

#[derive(Debug, Clone, Copy)]
pub enum Leg {
    LeftFront,
    LeftMiddle,
    LeftBack,
    RightFront,
    RightMiddle,
    RightBack,
}

impl Leg {
    pub fn is_left(&self) -> bool {
        matches!(self, Leg::LeftFront | Leg::LeftMiddle | Leg::LeftBack)
    }
    pub fn is_right(&self) -> bool {
        matches!(self, Leg::RightFront | Leg::RightMiddle | Leg::RightBack)
    }
    pub fn is_front(&self) -> bool {
        matches!(self, Leg::LeftFront | Leg::RightFront)
    }
    pub fn is_middle(&self) -> bool {
        matches!(self, Leg::LeftMiddle | Leg::RightMiddle)
    }
    pub fn is_back(&self) -> bool {
        matches!(self, Leg::LeftBack | Leg::RightBack)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LegPart {
    Coxa,
    Femur,
    Tibia,
}

#[derive(Debug, Clone, Copy)]
pub struct LegAngles {
    pub coxa: f32,
    pub femur: f32,
    pub tibia: f32,
}

pub struct ServoPins {
    pub left_front: (u8, u8, u8),   // (Coxa, Femur, Tibia)
    pub left_middle: (u8, u8, u8),
    pub left_back: (u8, u8, u8),
    pub right_front: (u8, u8, u8),
    pub right_middle: (u8, u8, u8),
    pub right_back: (u8, u8, u8),
}