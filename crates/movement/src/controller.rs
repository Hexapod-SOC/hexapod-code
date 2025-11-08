use crate::gait::{Gait, LegStances};
use crate::gaits::GaitTemplate;
use crate::ik::SimpleIK;
use crate::legs::{Leg, LegAngles};
use glam::{Vec3, Mat3, Quat};

/// Body orientation and position offset
#[derive(Debug, Clone, Copy)]
pub struct BodyPose {
    pub translation: Vec3,  // X, Y, Z offset
    pub rotation: Vec3,     // Roll, Pitch, Yaw in degrees
}

impl Default for BodyPose {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Vec3::ZERO,
        }
    }
}

impl BodyPose {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create pose with rotation (roll, pitch, yaw in degrees)
    pub fn with_rotation(roll: f32, pitch: f32, yaw: f32) -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Vec3::new(roll, pitch, yaw),
        }
    }

    /// Create pose with translation
    pub fn with_translation(x: f32, y: f32, z: f32) -> Self {
        Self {
            translation: Vec3::new(x, y, z),
            rotation: Vec3::ZERO,
        }
    }

    /// Get rotation matrix for body orientation
    pub fn get_rotation_matrix(&self) -> Mat3 {
        let roll = self.rotation.x.to_radians();
        let pitch = self.rotation.y.to_radians();
        let yaw = self.rotation.z.to_radians();
        
        Mat3::from_quat(Quat::from_euler(glam::EulerRot::XYZ, roll, pitch, yaw))
    }

    /// Transform a leg position based on body pose
    pub fn transform_position(&self, pos: Vec3) -> Vec3 {
        let rot_matrix = self.get_rotation_matrix();
        rot_matrix * pos + self.translation
    }
}

/// High-level controller for hexapod movement
pub struct GaitController {
    pub gait: Gait,
    pub ik: SimpleIK,  // Made public for hexapod.rs access
    body_pose: BodyPose,
}

impl GaitController {
    pub fn new(gait_template: &'static GaitTemplate, ik: SimpleIK) -> Self {
        Self {
            gait: Gait::new(gait_template),
            ik,
            body_pose: BodyPose::default(),
        }
    }

    /// Update gait based on time delta (in seconds)
    pub fn update(&mut self, dt: f32) {
        self.gait.update(dt);
    }

    /// Set body pose for orientation control
    pub fn set_body_pose(&mut self, pose: BodyPose) {
        self.body_pose = pose;
    }

    /// Get current body pose
    pub fn get_body_pose(&self) -> BodyPose {
        self.body_pose
    }

    /// Calculate leg angles for walking with given velocity and rotation
    /// velocity: Vec3(X=forward/back, Y=left/right, Z=up/down) - movement direction and speed
    /// rotation: body rotation speed (yaw rate, positive = counter-clockwise from above)
    pub fn calculate_walking_angles(
        &self,
        velocity: Vec3,
        rotation: f32,
    ) -> [(Leg, LegAngles); 6] {
        let leg_positions = self.gait.calculate_all_leg_positions(velocity, rotation);
        self.leg_positions_to_angles(leg_positions)
    }

    /// Calculate leg angles for static body pose (no walking)
    pub fn calculate_pose_angles(&self) -> [(Leg, LegAngles); 6] {
        let mut positions = LegStances::default();
        
        // Apply body pose transformation to each default stance
        let legs = [
            Leg::LeftFront, Leg::LeftMiddle, Leg::LeftBack,
            Leg::RightFront, Leg::RightMiddle, Leg::RightBack,
        ];
        
        for leg in legs.iter() {
            let default_pos = positions.get(*leg);
            let transformed = self.body_pose.transform_position(default_pos);
            positions.set(*leg, transformed);
        }
        
        self.leg_positions_to_angles(positions)
    }

    /// Calculate angles for walking + body pose combined
    pub fn calculate_walking_with_pose_angles(
        &self,
        velocity: Vec3,
        rotation: f32,
    ) -> [(Leg, LegAngles); 6] {
        let mut leg_positions = self.gait.calculate_all_leg_positions(velocity, rotation);
        
        // Apply body pose transformation
        let legs = [
            Leg::LeftFront, Leg::LeftMiddle, Leg::LeftBack,
            Leg::RightFront, Leg::RightMiddle, Leg::RightBack,
        ];
        
        for leg in legs.iter() {
            let pos = leg_positions.get(*leg);
            let transformed = self.body_pose.transform_position(pos);
            leg_positions.set(*leg, transformed);
        }
        
        self.leg_positions_to_angles(leg_positions)
    }

    /// Convert leg positions to angles using IK
    fn leg_positions_to_angles(&self, positions: LegStances) -> [(Leg, LegAngles); 6] {
        [
            (Leg::LeftFront, self.ik.calc_pos_leg_angles(Leg::LeftFront, positions.left_front)),
            (Leg::LeftMiddle, self.ik.calc_pos_leg_angles(Leg::LeftMiddle, positions.left_middle)),
            (Leg::LeftBack, self.ik.calc_pos_leg_angles(Leg::LeftBack, positions.left_back)),
            (Leg::RightFront, self.ik.calc_pos_leg_angles(Leg::RightFront, positions.right_front)),
            (Leg::RightMiddle, self.ik.calc_pos_leg_angles(Leg::RightMiddle, positions.right_middle)),
            (Leg::RightBack, self.ik.calc_pos_leg_angles(Leg::RightBack, positions.right_back)),
        ]
    }

    /// Change gait template on the fly
    pub fn set_gait(&mut self, gait_template: &'static GaitTemplate) {
        self.gait = Gait::new(gait_template);
    }

    /// Get current gait phase (0.0 to 1.0)
    pub fn get_gait_phase(&self) -> f32 {
        self.gait.get_phase()
    }

    /// Get current gait template
    pub fn get_template(&self) -> &GaitTemplate {
        self.gait.get_template()
    }
}
