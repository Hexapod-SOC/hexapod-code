use nalgebra::Vector2;

#[derive(Clone, Copy, Debug, Default)]
pub struct Pose2D {
    pub x: f32,
    pub y: f32,
    pub theta: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PoseDelta {
    pub forward: f32,
    pub sideways: f32,
    pub dtheta: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct LidarPoint {
    pub angle_deg: f32,
    pub distance_m: f32,
    pub intensity: u16,
}

#[derive(Clone, Debug)]
pub struct LaserScan {
    pub timestamp_ns: u64,
    pub rpm: f32,
    pub start_angle_deg: f32,
    pub end_angle_deg: f32,
    pub points: Vec<LidarPoint>,
}

impl LaserScan {
    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Beam {
    pub direction: Vector2<f32>,
    pub range_m: f32,
}
