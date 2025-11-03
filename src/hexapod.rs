/// High-level Hexapod robot controller
/// 
/// This module provides a unified interface for controlling the hexapod robot,
/// combining servo control, inverse kinematics, and gait generation.

use glam::Vec3;
use movement::{
    controller::{BodyPose, GaitController},
    gait::LegStances,
    gaits::GaitTemplate,
    ik::{Constraints, SimpleIK},
    legs::{Leg, LegAngles},
};
use devices::servo::{ServoController, ServoPins};

/// Main hexapod robot controller
/// 
/// Combines servo control, inverse kinematics, and gait generation
/// into a single high-level interface.
pub struct Hexapod {
    servo_controller: ServoController,
    gait_controller: GaitController,
}

impl Hexapod {
    /// Create a new hexapod controller
    pub fn new(
        servo_pins: ServoPins,
        ik_constraints: Constraints,
        initial_gait: &'static GaitTemplate,
        default_stance: Option<LegStances>,
    ) -> Self {
        let servo_controller = ServoController::new(servo_pins);
        let ik = SimpleIK::new(ik_constraints);
        
        let mut gait_controller = GaitController::new(initial_gait, ik);
        
        // Set custom stance if provided
        if let Some(stance) = default_stance {
            gait_controller.gait.set_default_stance(stance);
        }
        
        Self {
            servo_controller,
            gait_controller,
        }
    }

    /// Update the gait cycle by a time delta (in seconds)
    pub fn update(&mut self, dt: f32) {
        self.gait_controller.update(dt);
    }

    /// Set the body pose (orientation and translation)
    pub fn set_body_pose(&mut self, pose: BodyPose) {
        self.gait_controller.set_body_pose(pose);
    }

    /// Get the current body pose
    pub fn get_body_pose(&self) -> BodyPose {
        self.gait_controller.get_body_pose()
    }

    /// Change the gait pattern
    pub fn set_gait(&mut self, gait_template: &'static GaitTemplate) {
        self.gait_controller.set_gait(gait_template);
    }

    /// Get current gait phase (0.0 to 1.0)
    pub fn get_gait_phase(&self) -> f32 {
        self.gait_controller.get_gait_phase()
    }

    /// Get current gait template
    pub fn get_gait_template(&self) -> &GaitTemplate {
        self.gait_controller.get_template()
    }

    /// Move a single leg to a specific position
    /// 
    /// # Arguments
    /// * `leg` - Which leg to move
    /// * `position` - Target position in mm (X, Y, Z)
    pub fn move_leg_to_position(&mut self, leg: Leg, position: Vec3) {
        let angles = self.gait_controller.ik.calc_pos_leg_angles(leg, position);
        self.servo_controller.set_leg_angles(leg, angles);
    }

    /// Apply a static body pose without walking
    /// 
    /// Useful for body orientation control, tilting, etc.
    pub fn apply_static_pose(&mut self) {
        let angles = self.gait_controller.calculate_pose_angles();
        self.apply_leg_angles(angles);
    }

    /// Walk with the current gait
    /// 
    /// # Arguments
    /// * `velocity` - Movement velocity (X=forward, Y=strafe, Z=vertical)
    /// * `rotation` - Rotation speed (yaw rate)
    pub fn walk(&mut self, velocity: Vec3, rotation: f32) {
        let angles = self.gait_controller.calculate_walking_angles(velocity, rotation);
        self.apply_leg_angles(angles);
    }

    /// Walk while maintaining a body pose
    /// 
    /// Combines walking motion with body orientation control
    /// 
    /// # Arguments
    /// * `velocity` - Movement velocity (X=forward, Y=strafe, Z=vertical)
    /// * `rotation` - Rotation speed (yaw rate)
    pub fn walk_with_pose(&mut self, velocity: Vec3, rotation: f32) {
        let angles = self.gait_controller.calculate_walking_with_pose_angles(velocity, rotation);
        self.apply_leg_angles(angles);
    }

    /// Set all legs to the same angle
    /// 
    /// Useful for calibration or testing
    pub fn set_all_legs(&mut self, coxa: f32, femur: f32, tibia: f32) {
        self.servo_controller.set_all_legs_to_angles(coxa, femur, tibia);
    }

    /// Set a single leg's angles directly
    pub fn set_leg_angles(&mut self, leg: Leg, angles: LegAngles) {
        self.servo_controller.set_leg_angles(leg, angles);
    }

    /// Apply calculated leg angles to servos
    fn apply_leg_angles(&mut self, angles: [(Leg, LegAngles); 6]) {
        for (leg, leg_angles) in angles.iter() {
            self.servo_controller.set_leg_angles(*leg, *leg_angles);
        }
    }

    /// Reset to default standing position
    pub fn reset_to_default_stance(&mut self) {
        self.gait_controller.set_body_pose(BodyPose::default());
        self.apply_static_pose();
    }
}

/// Builder pattern for creating a Hexapod with custom configuration
pub struct HexapodBuilder {
    servo_pins: ServoPins,
    ik_constraints: Constraints,
    initial_gait: &'static GaitTemplate,
    default_stance: Option<LegStances>,
}

impl HexapodBuilder {
    pub fn new(
        servo_pins: ServoPins,
        ik_constraints: Constraints,
        initial_gait: &'static GaitTemplate,
    ) -> Self {
        Self {
            servo_pins,
            ik_constraints,
            initial_gait,
            default_stance: None,
        }
    }

    pub fn with_stance(mut self, stance: LegStances) -> Self {
        self.default_stance = Some(stance);
        self
    }

    pub fn build(self) -> Hexapod {
        Hexapod::new(
            self.servo_pins,
            self.ik_constraints,
            self.initial_gait,
            self.default_stance,
        )
    }
}
