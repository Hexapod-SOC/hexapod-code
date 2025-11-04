/// High-level Hexapod robot controller
/// 
/// This module provides a unified interface for controlling the hexapod robot,
/// combining servo control, inverse kinematics, and gait generation.

use glam::Vec3;
use std::sync::Arc;
use tokio::sync::Mutex;
use movement::{
    controller::{BodyPose, GaitController},
    gait::LegStances,
    gaits::GaitTemplate,
    ik::{Constraints, SimpleIK},
    legs::{Leg, LegAngles},
};
use devices::servo::{ServoController, ServoPins};
use devices::picoubec::{PicoUbecController, BatteryStatus, PowerState};

/// Main hexapod robot controller
/// 
/// Combines servo control, inverse kinematics, and gait generation
/// into a single high-level interface.
pub struct Hexapod {
    servo_controller: Arc<Mutex<ServoController>>,
    gait_controller: Arc<Mutex<GaitController>>,
    ubec_controller: Arc<Mutex<PicoUbecController>>,
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
        
        // Initialize battery monitor (will gracefully handle if not available)
        let ubec_controller = PicoUbecController::new("/dev/serial0");
        
        Self {
            servo_controller: Arc::new(Mutex::new(servo_controller)),
            gait_controller: Arc::new(Mutex::new(gait_controller)),
            ubec_controller: Arc::new(Mutex::new(ubec_controller)),
        }
    }
    
    /// Get shared reference to servo controller for web API
    pub fn get_servo_controller(&self) -> Arc<Mutex<ServoController>> {
        self.servo_controller.clone()
    }
    
    /// Get shared reference to gait controller for web API
    pub fn get_gait_controller(&self) -> Arc<Mutex<GaitController>> {
        self.gait_controller.clone()
    }
    
    /// Get shared reference to UBEC controller for web API
    pub fn get_ubec_controller(&self) -> Arc<Mutex<PicoUbecController>> {
        self.ubec_controller.clone()
    }

    /// Update the gait cycle by a time delta (in seconds)
    /// Also updates battery monitoring
    pub async fn update(&mut self, dt: f32) {
        self.gait_controller.lock().await.update(dt);
        self.ubec_controller.lock().await.update();
    }
    
    /// Get current battery status
    pub async fn get_battery_status(&self) -> BatteryStatus {
        self.ubec_controller.lock().await.get_battery_status()
    }
    
    /// Get current power state
    pub async fn get_power_state(&self) -> PowerState {
        self.ubec_controller.lock().await.get_power_state()
    }
    
    /// Check if battery is in critical state
    pub async fn is_battery_critical(&self) -> bool {
        self.ubec_controller.lock().await.is_critical()
    }
    
    /// Emergency shutdown - executes system shutdown command
    /// 
    /// This should be called when battery is critically low to safely
    /// shut down the Raspberry Pi before power loss.
    pub fn emergency_shutdown(&self) -> std::io::Result<()> {
        use std::process::Command;
        
        println!("⚠️  EMERGENCY SHUTDOWN INITIATED");
        println!("Executing system shutdown...");
        
        // Execute shutdown command (requires proper permissions)
        // The system will shutdown in 1 minute by default, or use 'now' for immediate
        Command::new("sudo")
            .arg("shutdown")
            .arg("-h")
            .arg("now")
            .arg("Critical battery - emergency shutdown")
            .spawn()?;
        
        Ok(())
    }

    /// Set the body pose (orientation and translation)
    pub async fn set_body_pose(&mut self, pose: BodyPose) {
        self.gait_controller.lock().await.set_body_pose(pose);
    }

    /// Get the current body pose
    pub async fn get_body_pose(&self) -> BodyPose {
        self.gait_controller.lock().await.get_body_pose()
    }

    /// Change the gait pattern
    pub async fn set_gait(&mut self, gait_template: &'static GaitTemplate) {
        self.gait_controller.lock().await.set_gait(gait_template);
    }

    /// Get current gait phase (0.0 to 1.0)
    pub async fn get_gait_phase(&self) -> f32 {
        self.gait_controller.lock().await.get_gait_phase()
    }

    /// Get current gait template name
    pub async fn get_gait_template_name(&self) -> String {
        self.gait_controller.lock().await.get_template().name.to_string()
    }

    /// Move a single leg to a specific position
    /// 
    /// # Arguments
    /// * `leg` - Which leg to move
    /// * `position` - Target position in mm (X, Y, Z)
    pub async fn move_leg_to_position(&mut self, leg: Leg, position: Vec3) {
        let gait = self.gait_controller.lock().await;
        let angles = gait.ik.calc_pos_leg_angles(leg, position);
        drop(gait);
        self.servo_controller.lock().await.set_leg_angles(leg, angles);
    }

    /// Apply a static body pose without walking
    /// 
    /// Useful for body orientation control, tilting, etc.
    pub async fn apply_static_pose(&mut self) {
        let angles = self.gait_controller.lock().await.calculate_pose_angles();
        self.apply_leg_angles(angles).await;
    }

    /// Walk with the current gait
    /// 
    /// # Arguments
    /// * `velocity` - Movement velocity (X=forward, Y=strafe, Z=vertical)
    /// * `rotation` - Rotation speed (yaw rate)
    pub async fn walk(&mut self, velocity: Vec3, rotation: f32) {
        let angles = self.gait_controller.lock().await.calculate_walking_angles(velocity, rotation);
        self.apply_leg_angles(angles).await;
    }

    /// Walk while maintaining a body pose
    /// 
    /// Combines walking motion with body orientation control
    /// 
    /// # Arguments
    /// * `velocity` - Movement velocity (X=forward, Y=strafe, Z=vertical)
    /// * `rotation` - Rotation speed (yaw rate)
    pub async fn walk_with_pose(&mut self, velocity: Vec3, rotation: f32) {
        let angles = self.gait_controller.lock().await.calculate_walking_with_pose_angles(velocity, rotation);
        self.apply_leg_angles(angles).await;
    }

    /// Set all legs to the same angle
    /// 
    /// Useful for calibration or testing
    pub async fn set_all_legs(&mut self, coxa: f32, femur: f32, tibia: f32) {
        self.servo_controller.lock().await.set_all_legs_to_angles(coxa, femur, tibia);
    }

    /// Set a single leg's angles directly
    pub async fn set_leg_angles(&mut self, leg: Leg, angles: LegAngles) {
        self.servo_controller.lock().await.set_leg_angles(leg, angles);
    }

    /// Apply calculated leg angles to servos
    async fn apply_leg_angles(&mut self, angles: [(Leg, LegAngles); 6]) {
        let mut servo = self.servo_controller.lock().await;
        for (leg, leg_angles) in angles.iter() {
            servo.set_leg_angles(*leg, *leg_angles);
        }
    }

    /// Reset to default standing position
    pub async fn reset_to_default_stance(&mut self) {
        self.gait_controller.lock().await.set_body_pose(BodyPose::default());
        self.apply_static_pose().await;
    }
    
    /// Put hexapod in safe shutdown position
    /// 
    /// Pulls legs up so the body rests on its belly with servos in a
    /// comfortable holding position. This prevents MG996R servos from
    /// drawing excessive current (up to 8A!) when holding awkward angles.
    /// 
    /// Position: Coxa neutral (90°), Femur up (135°), Tibia folded (135°)
    pub async fn safe_shutdown_position(&mut self) {
        println!("Moving to safe shutdown position...");
        
        // Set all legs to a safe "folded up" position
        // Coxa: 90° (neutral, pointing straight out)
        // Femur: 135° (lifted up)
        // Tibia: 135° (folded back toward body)
        // This lets the body rest on its belly with minimal servo strain
        self.servo_controller.lock().await.set_all_legs_to_angles(90.0, 135.0, 135.0);
        
        println!("Servos in safe holding position - body resting on belly");
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
