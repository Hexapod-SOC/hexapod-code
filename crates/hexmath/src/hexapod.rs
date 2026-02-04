use glam;

#[derive(Clone, Debug)]
pub struct Leg {
    pub location: glam::Vec3,
    /// The angle the coxa servo is mounted at, relative to straight out from body (in degrees)
    /// 0 = pointing straight out, 45 = angled 45° forward, -45 = angled 45° backward
    pub mount_angle: f32,
    pub coxa_length: f32,
    pub femur_length: f32,
    pub tibia_length: f32,
    pub coxa_angle: f32,
    pub femur_angle: f32,
    pub tibia_angle: f32,
    pub target_coxa_angle: f32,
    pub target_femur_angle: f32,
    pub target_tibia_angle: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum Joint {
    Coxa,
    Femur,
    Tibia,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LegId {
    LeftFront,
    LeftMiddle,
    LeftBack,
    RightFront,
    RightMiddle,
    RightBack,
}

#[derive(Clone, Debug)]
pub struct Legs {
    pub left_front: Leg,
    pub left_middle: Leg,
    pub left_back: Leg,
    pub right_front: Leg,
    pub right_middle: Leg,
    pub right_back: Leg,
}

#[derive(Clone, Debug)]
pub struct Dimensions {
    pub body_length: f32,
    pub body_width: f32,
    pub body_height: f32,

}
/// Represents a Hexapod robot with six legs and body dimensions
#[derive(Clone, Debug)]
pub struct Hexapod {
    /// Legs of the hexapod
    pub legs: Legs,
    /// Dimensions of the hexapod body in mm
    pub dimensions: Dimensions,
}

impl Hexapod {
    pub fn new() -> Self {
        // Body dimensions
        let body_half_width = 50.0;  // Half of body width (X axis)
        let body_half_length = 70.0; // Half of body length (Y axis)
        
        // Front/back legs attach at corners (45° from center)
        // Using same distance from center as middle legs for consistency
        let corner_offset_x = 50.0;  // X offset for corner legs
        let corner_offset_y = 70.0;  // Y offset for corner legs
        
        Hexapod {
            dimensions: Dimensions {
                body_length: 140.0,
                body_width: 100.0,
                body_height: 16.0,
            },
            legs: Legs {
                left_front: Leg {
                    location: glam::Vec3::new(-corner_offset_x, corner_offset_y, 10.0),
                    mount_angle: -45.0,  // Angled 45° forward-outward
                    coxa_length: 45.0,
                    femur_length: 60.0,
                    tibia_length: 105.0,
                    coxa_angle: 0.0,
                    femur_angle: 0.0,
                    tibia_angle: 0.0,
                    target_coxa_angle: 0.0,
                    target_femur_angle: 0.0,
                    target_tibia_angle: 0.0,
                },
                right_front: Leg {
                    location: glam::Vec3::new(corner_offset_x, corner_offset_y, 10.0),
                    mount_angle: 45.0,  // Angled 45° forward-outward (mirrored for right side)
                    coxa_length: 45.0,
                    femur_length: 60.0,
                    tibia_length: 105.0,
                    coxa_angle: 0.0,
                    femur_angle: 0.0,
                    tibia_angle: 0.0,
                    target_coxa_angle: 0.0,
                    target_femur_angle: 0.0,
                    target_tibia_angle: 0.0,
                },
                left_middle: Leg {
                    location: glam::Vec3::new(-80.0, 0.0, 10.0),
                    mount_angle: 0.0,  // Straight out
                    coxa_length: 45.0,
                    femur_length: 60.0,
                    tibia_length: 105.0,
                    coxa_angle: 0.0,
                    femur_angle: 0.0,
                    tibia_angle: 0.0,
                    target_coxa_angle: 0.0,
                    target_femur_angle: 0.0,
                    target_tibia_angle: 0.0,
                },
                right_middle: Leg {
                    location: glam::Vec3::new(80.0, 0.0, 10.0),
                    mount_angle: 0.0,  // Straight out
                    coxa_length: 45.0,
                    femur_length: 60.0,
                    tibia_length: 105.0,
                    coxa_angle: 0.0,
                    femur_angle: 0.0,
                    tibia_angle: 0.0,
                    target_coxa_angle: 0.0,
                    target_femur_angle: 0.0,
                    target_tibia_angle: 0.0,
                },
                left_back: Leg {
                    location: glam::Vec3::new(-corner_offset_x, -corner_offset_y, 10.0),
                    mount_angle: 45.0,  // Angled 45° backward-outward
                    coxa_length: 45.0,
                    femur_length: 60.0,
                    tibia_length: 105.0,
                    coxa_angle: 0.0,
                    femur_angle: 0.0,
                    tibia_angle: 0.0,
                    target_coxa_angle: 0.0,
                    target_femur_angle: 0.0,
                    target_tibia_angle: 0.0,
                },
                right_back: Leg {
                    location: glam::Vec3::new(corner_offset_x, -corner_offset_y, 10.0),
                    mount_angle: -45.0,  // Angled 45° backward-outward (mirrored for right side)
                    coxa_length: 45.0,
                    femur_length: 60.0,
                    tibia_length: 105.0,
                    coxa_angle: 0.0,
                    femur_angle: 0.0,
                    tibia_angle: 0.0,
                    target_coxa_angle: 0.0,
                    target_femur_angle: 0.0,
                    target_tibia_angle: 0.0,
                },
            },
        }
    }
}

impl Leg {
    pub fn set_target_angles(&mut self, coxa: f32, femur: f32, tibia: f32) {
        self.target_coxa_angle = coxa;
        self.target_femur_angle = femur;
        self.target_tibia_angle = tibia;
    }

    pub fn set_target_angle(&mut self, joint: Joint, angle: f32) {
        match joint {
            Joint::Coxa => self.target_coxa_angle = angle,
            Joint::Femur => self.target_femur_angle = angle,
            Joint::Tibia => self.target_tibia_angle = angle,
        }
    }
}


impl Hexapod {
    pub fn get_current_legs_state(&self) -> &Legs {
        &self.legs
    }
    pub fn set_legs_state(&mut self, new_legs: Legs) {
        self.legs = new_legs;
    }

    pub fn set_leg_target_angles(&mut self, leg: LegId, coxa: f32, femur: f32, tibia: f32) {
        let target_leg = match leg {
            LegId::LeftFront => &mut self.legs.left_front,
            LegId::LeftMiddle => &mut self.legs.left_middle,
            LegId::LeftBack => &mut self.legs.left_back,
            LegId::RightFront => &mut self.legs.right_front,
            LegId::RightMiddle => &mut self.legs.right_middle,
            LegId::RightBack => &mut self.legs.right_back,
        };

        target_leg.set_target_angles(coxa, femur, tibia);
    }
}