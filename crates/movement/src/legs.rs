#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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