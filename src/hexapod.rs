use crate::config::UBEC_PORT;
use devices::picoubec::{BatteryStatus, PicoUbecController, PowerState};
use devices::servo::{ServoController, ServoOffsets, ServoPins};
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
use std::sync::Arc;
use tokio::sync::Mutex;

// Small motion deadzones to avoid unintended walking when inputs jitter near zero
const VEL_DEADZONE: f32 = 0.5; // mm/s deadband for forward/strafe
const ROT_DEADZONE: f32 = 0.01; // rad/s deadband for yaw rotation
// Input smoothing (first-order low-pass) to make gait transitions gentler
const VEL_SMOOTH_TAU: f32 = 0.25; // seconds, smaller = snappier
const ROT_SMOOTH_TAU: f32 = 0.25;
#[derive(Debug, Clone, Copy, Default)]
pub struct ServoAngleTriplet {
    pub coxa: f32,
    pub femur: f32,
    pub tibia: f32,
}

#[derive(Debug, Clone)]
pub struct ServoAngleTweaks {
    pub left_front: ServoAngleTriplet,
    pub left_middle: ServoAngleTriplet,
    pub left_back: ServoAngleTriplet,
    pub right_front: ServoAngleTriplet,
    pub right_middle: ServoAngleTriplet,
    pub right_back: ServoAngleTriplet,
}

impl Default for ServoAngleTweaks {
    fn default() -> Self {
        Self {
            left_front: ServoAngleTriplet::default(),
            left_middle: ServoAngleTriplet::default(),
            left_back: ServoAngleTriplet::default(),
            right_front: ServoAngleTriplet::default(),
            right_middle: ServoAngleTriplet::default(),
            right_back: ServoAngleTriplet::default(),
        }
    }
}

/// Control inputs for the hexapod - can be set by any control interface
#[derive(Debug, Clone, Copy)]
pub struct HexapodControl {
    pub velocity: Vec3,      // X=forward, Z=strafe (Y unused)
    pub rotation: f32,       // Yaw rotation rate
    pub body_pose: BodyPose, // Body orientation
    pub enabled: bool,       // Master enable/disable
}

impl Default for HexapodControl {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            rotation: 0.0,
            body_pose: BodyPose::default(),
            enabled: true,
        }
    }
}

/// Main hexapod robot controller
///
/// Combines servo control, inverse kinematics, and gait generation
/// into a single high-level interface.
pub struct Hexapod {
    servo_controller: Arc<Mutex<ServoController>>,
    gait_controller: Arc<Mutex<GaitController>>,
    ubec_controller: Arc<Mutex<PicoUbecController>>,
    control: Arc<Mutex<HexapodControl>>, // Shared control state
    servo_angle_tweaks: Arc<Mutex<ServoAngleTweaks>>,
    smoothed_velocity: Vec3,
    smoothed_rotation: f32,
}

impl Hexapod {
    /// Create a new hexapod controller
    pub fn new(
        servo_pins: ServoPins,
        servo_offsets: ServoOffsets,
        ik_constraints: Constraints,
        initial_gait: &'static GaitTemplate,
        default_stance: Option<LegStances>,
    ) -> Self {
        let servo_controller = ServoController::new(servo_pins, servo_offsets);
        let ik = SimpleIK::new(ik_constraints);

        let mut gait_controller = GaitController::new(initial_gait, ik);

        // Set custom stance if provided
        if let Some(stance) = default_stance {
            gait_controller.gait.set_default_stance(stance);
        }

        // Initialize battery monitor (will gracefully handle if not available)
        let ubec_port = std::env::var("UBEC_PORT").unwrap_or_else(|_| UBEC_PORT.to_string());
        let mut ubec_controller = PicoUbecController::new(&ubec_port);

        // Enable servos on startup
        ubec_controller.enable_servos();

        Self {
            servo_controller: Arc::new(Mutex::new(servo_controller)),
            gait_controller: Arc::new(Mutex::new(gait_controller)),
            ubec_controller: Arc::new(Mutex::new(ubec_controller)),
            control: Arc::new(Mutex::new(HexapodControl::default())),
            servo_angle_tweaks: Arc::new(Mutex::new(ServoAngleTweaks::default())),
            smoothed_velocity: Vec3::ZERO,
            smoothed_rotation: 0.0,
        }
    }

    /// Get shared reference to control state for external control (API, Bluetooth, etc.)
    pub fn get_control(&self) -> Arc<Mutex<HexapodControl>> {
        self.control.clone()
    }

    /// Get shared reference to per-servo angle tweaks
    pub fn get_servo_angle_tweaks(&self) -> Arc<Mutex<ServoAngleTweaks>> {
        self.servo_angle_tweaks.clone()
    }

    /// Get shared reference to gait controller (for gait changes)
    pub fn get_gait_controller(&self) -> Arc<Mutex<GaitController>> {
        self.gait_controller.clone()
    }

    /// Get shared reference to UBEC controller (for battery status)
    pub fn get_ubec_controller(&self) -> Arc<Mutex<PicoUbecController>> {
        self.ubec_controller.clone()
    }

    /// Main update loop - reads control state and updates servos
    ///
    /// This should be called periodically (e.g., 20-50Hz) in a tokio task.
    /// All control interfaces (web API, gamepad, etc.) just modify the control state,
    /// and this function does all the calculations and applies them.
    pub async fn update(&mut self, dt: f32) {
        // Update battery monitoring
        self.ubec_controller.lock().await.update();

        // Get current control inputs
        let control = {
            let ctrl = self.control.lock().await;
            *ctrl // Copy the control state
        };

        if !control.enabled {
            // If disabled, don't move
            return;
        }

        // Apply deadzones so tiny stick/joystick noise doesn't trigger gait motion
        let mut velocity = control.velocity;
        let mut rotation = control.rotation;
        let apply_deadzone = |v: f32, dz: f32| if v.abs() < dz { 0.0 } else { v };
        velocity.x = apply_deadzone(velocity.x, VEL_DEADZONE);
        velocity.y = apply_deadzone(velocity.y, VEL_DEADZONE);
        velocity.z = apply_deadzone(velocity.z, VEL_DEADZONE);
        rotation = apply_deadzone(rotation, ROT_DEADZONE);

        // Smooth inputs to avoid jerky transitions in gait
        let alpha_vel = (dt / (VEL_SMOOTH_TAU + dt)).clamp(0.0, 1.0);
        let alpha_rot = (dt / (ROT_SMOOTH_TAU + dt)).clamp(0.0, 1.0);
        self.smoothed_velocity =
            self.smoothed_velocity + (velocity - self.smoothed_velocity) * alpha_vel;
        self.smoothed_rotation =
            self.smoothed_rotation + (rotation - self.smoothed_rotation) * alpha_rot;

        // Update gait phase
        {
            let mut gait = self.gait_controller.lock().await;
            gait.update(dt);
            gait.set_body_pose(control.body_pose);
        }

        // Calculate leg angles based on current control state
        let is_moving = self.smoothed_velocity.length_squared() > 0.0 || self.smoothed_rotation != 0.0;

        let angles = if is_moving {
            // Walking with body pose
            let gait = self.gait_controller.lock().await;
            gait.calculate_walking_with_pose_angles(self.smoothed_velocity, self.smoothed_rotation)
        } else {
            // Static pose only
            let gait = self.gait_controller.lock().await;
            gait.calculate_pose_angles()
        };

        // Apply per-servo angle tweaks
        let tweaks = { self.servo_angle_tweaks.lock().await.clone() };
        let mut adjusted = angles;
        for (leg, leg_angles) in adjusted.iter_mut() {
            let t = match leg {
                Leg::LeftFront => tweaks.left_front,
                Leg::LeftMiddle => tweaks.left_middle,
                Leg::LeftBack => tweaks.left_back,
                Leg::RightFront => tweaks.right_front,
                Leg::RightMiddle => tweaks.right_middle,
                Leg::RightBack => tweaks.right_back,
            };
            leg_angles.coxa = (leg_angles.coxa + t.coxa).clamp(0.0, 180.0);
            leg_angles.femur = (leg_angles.femur + t.femur).clamp(0.0, 180.0);
            leg_angles.tibia = (leg_angles.tibia + t.tibia).clamp(0.0, 180.0);
        }

        // Smooth servo commands to avoid abrupt foot impacts
        let mut servo = self.servo_controller.lock().await;
        for (leg, leg_angles) in adjusted.iter() {
            servo.set_leg_angles(*leg, *leg_angles);
        }
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

    /// Set control velocity (for programmatic control or demos)
    pub async fn set_velocity(&self, velocity: Vec3, rotation: f32) {
        let mut control = self.control.lock().await;
        control.velocity = velocity;
        control.rotation = rotation;
    }

    /// Set body pose (for programmatic control or demos)
    pub async fn set_body_pose(&self, pose: BodyPose) {
        let mut control = self.control.lock().await;
        control.body_pose = pose;
    }

    /// Get the current body pose
    pub async fn get_body_pose(&self) -> BodyPose {
        let control = self.control.lock().await;
        control.body_pose
    }

    /// Enable/disable movement
    pub async fn set_enabled(&self, enabled: bool) {
        let mut control = self.control.lock().await;
        control.enabled = enabled;
    }

    /// Emergency stop - immediately zeros all control inputs
    pub async fn emergency_stop(&self) {
        let mut control = self.control.lock().await;
        *control = HexapodControl::default();
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

    /// Change the gait pattern
    pub async fn set_gait(&self, gait_template: &'static GaitTemplate) {
        self.gait_controller.lock().await.set_gait(gait_template);
    }

    /// Get current gait phase (0.0 to 1.0)
    pub async fn get_gait_phase(&self) -> f32 {
        self.gait_controller.lock().await.get_gait_phase()
    }

    /// Get current gait template name
    pub async fn get_gait_template_name(&self) -> String {
        self.gait_controller
            .lock()
            .await
            .get_template()
            .name
            .to_string()
    }

    /// Set all legs to the same angle
    ///
    /// Useful for calibration or testing
    pub async fn set_all_legs(&self, coxa: f32, femur: f32, tibia: f32) {
        self.servo_controller
            .lock()
            .await
            .set_all_legs_to_angles(coxa, femur, tibia);
    }

    /// Reset to default standing position
    pub async fn reset_to_default_stance(&mut self) {
        let mut control = self.control.lock().await;
        control.body_pose = BodyPose::default();
        control.velocity = Vec3::ZERO;
        control.rotation = 0.0;
    }

    /// Put hexapod in safe shutdown position
    ///
    /// Pulls legs up so the body rests on its belly with servos in a
    /// comfortable holding position. This prevents MG996R servos from
    /// drawing excessive current (up to 8A!) when holding awkward angles.
    ///
    /// Position: Coxa neutral (90°), Femur up (135°), Tibia folded (135°)
    pub async fn safe_shutdown_position(&self) {
        println!("Moving to safe shutdown position...");

        // Set all legs to a safe "folded up" position
        // Coxa: 90° (neutral, pointing straight out)
        // Femur: 135° (lifted up)
        // Tibia: 135° (folded back toward body)
        // This lets the body rest on its belly with minimal servo strain
        self.servo_controller
            .lock()
            .await
            .set_all_legs_to_angles(90.0, 135.0, 135.0);

        println!("Servos in safe holding position - body resting on belly");
    }
}

/// Builder pattern for creating a Hexapod with custom configuration
pub struct HexapodBuilder {
    servo_pins: ServoPins,
    servo_offsets: ServoOffsets,
    ik_constraints: Constraints,
    initial_gait: &'static GaitTemplate,
    default_stance: Option<LegStances>,
}

impl HexapodBuilder {
    pub fn new(
        servo_pins: ServoPins,
        servo_offsets: ServoOffsets,
        ik_constraints: Constraints,
        initial_gait: &'static GaitTemplate,
    ) -> Self {
        Self {
            servo_pins,
            servo_offsets,
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
            self.servo_offsets,
            self.ik_constraints,
            self.initial_gait,
            self.default_stance,
        )
    }
}
