use crate::config::{IMU_ENABLE, IMU_I2C_ADR, IMU_I2C_BUS, UBEC_PORT};
use devices::imu::{self, Imu};
use devices::imu_driver;
use devices::picoubec::{BatteryStatus, PicoUbecController, PowerState};
use devices::servo::{ServoController, ServoOffsets, ServoPins};
/// High-level Hexapod robot controller
///
/// This module provides a unified interface for controlling the hexapod robot,
/// combining servo control, inverse kinematics, and gait generation.
use glam::{EulerRot, Mat2, Quat, Vec3};
use hexmath::hexapod::{Hexapod as MathHexapod, LegId};
use hexmath::ik::{ik_solve_leg, Constraints, Geometry, LegAngles};
use hexmath::{compute_leg_joints, step_hexapod, GaitConfig, GaitType, InputState, WalkState};
use std::sync::Arc;
use tokio::sync::Mutex;

// Small motion deadzones to avoid unintended walking when inputs jitter near zero
const VEL_DEADZONE: f32 = 0.5; // mm/s deadband for forward/strafe
const ROT_DEADZONE: f32 = 0.01; // rad/s deadband for yaw rotation
// Input smoothing (first-order low-pass) to make gait transitions gentler
const VEL_SMOOTH_TAU: f32 = 0.25; // seconds, smaller = snappier
const ROT_SMOOTH_TAU: f32 = 0.25;
// Normalize velocity/rotation inputs into [-1, 1] for hexmath
const MAX_SPEED_MM_S: f32 = 100.0;
const MAX_TURN_RATE: f32 = 1.0;
// Servo command range (aligned to 0..180 mapping).
const SERVO_ANGLE_MIN: f32 = 0.0;
const SERVO_ANGLE_MAX: f32 = 180.0;

pub type Leg = LegId;

pub fn control_to_input(velocity: Vec3, rotation: f32) -> InputState {
    InputState {
        move_forward: (velocity.x / MAX_SPEED_MM_S).clamp(-1.0, 1.0),
        move_strafe: (velocity.y / MAX_SPEED_MM_S).clamp(-1.0, 1.0),
        turn: (rotation / MAX_TURN_RATE).clamp(-1.0, 1.0),
        body_yaw: 0.0,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BodyPose {
    pub rotation: Vec3,    // degrees: roll (x), pitch (y), yaw (z)
    pub translation: Vec3, // mm
}

impl BodyPose {
    pub fn with_rotation(roll: f32, pitch: f32, yaw: f32) -> Self {
        Self {
            rotation: Vec3::new(roll, pitch, yaw),
            translation: Vec3::ZERO,
        }
    }

    pub fn transform_position(&self, pos: Vec3) -> Vec3 {
        let rot = Quat::from_euler(
            EulerRot::XYZ,
            self.rotation.x.to_radians(),
            self.rotation.y.to_radians(),
            self.rotation.z.to_radians(),
        );
        rot * pos + self.translation
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LegStances {
    pub left_front: Vec3,
    pub left_middle: Vec3,
    pub left_back: Vec3,
    pub right_front: Vec3,
    pub right_middle: Vec3,
    pub right_back: Vec3,
}

impl LegStances {
    pub fn get(&self, leg: LegId) -> Vec3 {
        match leg {
            LegId::LeftFront => self.left_front,
            LegId::LeftMiddle => self.left_middle,
            LegId::LeftBack => self.left_back,
            LegId::RightFront => self.right_front,
            LegId::RightMiddle => self.right_middle,
            LegId::RightBack => self.right_back,
        }
    }

    pub fn set(&mut self, leg: LegId, value: Vec3) {
        match leg {
            LegId::LeftFront => self.left_front = value,
            LegId::LeftMiddle => self.left_middle = value,
            LegId::LeftBack => self.left_back = value,
            LegId::RightFront => self.right_front = value,
            LegId::RightMiddle => self.right_middle = value,
            LegId::RightBack => self.right_back = value,
        }
    }

    pub fn to_array(&self, leg: LegId) -> [f32; 3] {
        let v = self.get(leg);
        [v.x, v.y, v.z]
    }

    pub fn from_hexapod(hexapod: &MathHexapod, base_height: f32) -> Self {
        let make = |leg: &hexmath::hexapod::Leg| {
            let reach = leg.femur_length * 0.6 + leg.tibia_length * 0.4;
            Vec3::new(leg.coxa_length + reach, 0.0, base_height)
        };

        Self {
            left_front: make(&hexapod.legs.left_front),
            left_middle: make(&hexapod.legs.left_middle),
            left_back: make(&hexapod.legs.left_back),
            right_front: make(&hexapod.legs.right_front),
            right_middle: make(&hexapod.legs.right_middle),
            right_back: make(&hexapod.legs.right_back),
        }
    }
}

impl Default for LegStances {
    fn default() -> Self {
        Self {
            left_front: Vec3::ZERO,
            left_middle: Vec3::ZERO,
            left_back: Vec3::ZERO,
            right_front: Vec3::ZERO,
            right_middle: Vec3::ZERO,
            right_back: Vec3::ZERO,
        }
    }
}
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
            left_front: ServoAngleTriplet {
                coxa: 0.0,
                femur: 0.0,
                tibia: 0.0,
            },
            left_middle: ServoAngleTriplet {
                coxa: 0.0,
                femur: 0.0,
                tibia: 0.0,
            },
            left_back: ServoAngleTriplet {
                coxa: 0.0,
                femur: 0.0,
                tibia: 0.0,
            },
            right_front: ServoAngleTriplet {
                coxa: 0.0,
                femur: 0.0,
                tibia: 0.0,
            },
            right_middle: ServoAngleTriplet {
                coxa: 0.0,
                femur: 0.0,
                tibia: 0.0,
            },
            right_back: ServoAngleTriplet {
                coxa: 0.0,
                femur: 0.0,
                tibia: 0.0,
            },
        }
    }
}

pub struct GaitController {
    gait_config: GaitConfig,
    walk_state: WalkState,
    math_hexapod: MathHexapod,
    constraints: Constraints,
    body_pose: BodyPose,
    default_stance: LegStances,
    custom_gait_name: Option<String>,
}

impl GaitController {
    pub fn new(initial_gait: GaitType, constraints: Constraints, default_stance: Option<LegStances>) -> Self {
        let mut math_hexapod = MathHexapod::new();
        apply_constraints_to_hexapod(&mut math_hexapod, &constraints);

        let mut gait_config = GaitConfig::default();
        gait_config.gait_type = initial_gait;

        let default_stance = default_stance.unwrap_or_else(|| {
            LegStances::from_hexapod(&math_hexapod, gait_config.base_height)
        });

        Self {
            gait_config,
            walk_state: WalkState::default(),
            math_hexapod,
            constraints,
            body_pose: BodyPose::default(),
            default_stance,
            custom_gait_name: None,
        }
    }

    pub fn update(&mut self, input: &InputState, dt: f32) {
        let _ = step_hexapod(
            &mut self.math_hexapod,
            &mut self.walk_state,
            input,
            &self.gait_config,
            dt,
            1.0,
            0.0,
            0.0,
        );
    }

    pub fn get_gait_phase(&self) -> f32 {
        self.walk_state.phase
    }

    pub fn get_gait_name(&self) -> String {
        self.custom_gait_name
            .clone()
            .unwrap_or_else(|| self.gait_config.gait_type.name().to_string())
    }

    pub fn get_gait_type(&self) -> GaitType {
        self.gait_config.gait_type
    }

    pub fn set_gait(&mut self, gait_type: GaitType) {
        self.gait_config.gait_type = gait_type;
        self.gait_config.phase_offsets_override = None;
        self.custom_gait_name = None;
    }

    pub fn set_custom_gait(
        &mut self,
        name: String,
        offsets: [f32; 6],
        push_fraction: f32,
        speed_multiplier: f32,
        step_length_multiplier: f32,
        lift_height_multiplier: f32,
        max_step_length: f32,
        max_speed: f32,
    ) {
        let base = GaitConfig::default();
        self.custom_gait_name = Some(name);
        self.gait_config.phase_offsets_override = Some(offsets);
        self.gait_config.duty_factor = push_fraction.clamp(0.05, 0.95);
        self.gait_config.step_length =
            (base.step_length * step_length_multiplier).clamp(0.0, max_step_length.max(0.0));
        self.gait_config.step_height = (base.step_height * lift_height_multiplier).max(0.0);
        self.gait_config.speed = (base.speed * speed_multiplier).clamp(0.1, max_speed.max(0.1));
    }

    pub fn set_custom_gait_absolute(
        &mut self,
        name: String,
        offsets: [f32; 6],
        duty_factor: f32,
        speed: f32,
        step_length: f32,
        step_height: f32,
        base_height: f32,
        body_push_gain: f32,
        max_step_length: f32,
        max_speed: f32,
    ) {
        let base = GaitConfig::default();
        self.custom_gait_name = Some(name);
        self.gait_config.phase_offsets_override = Some(offsets);
        self.gait_config.duty_factor = duty_factor.clamp(0.05, 0.95);
        self.gait_config.step_length = step_length.clamp(0.0, max_step_length.max(0.0));
        self.gait_config.step_height = step_height.max(0.0);
        self.gait_config.speed = speed.clamp(0.1, max_speed.max(0.1));
        self.gait_config.base_height = base_height.clamp(-300.0, 0.0);
        self.gait_config.body_push_gain = body_push_gain.clamp(0.0, 10.0);
    }

    pub fn get_body_pose(&self) -> BodyPose {
        self.body_pose
    }

    pub fn set_body_pose(&mut self, pose: BodyPose) {
        self.body_pose = pose;
    }

    pub fn get_default_stance(&self) -> LegStances {
        self.default_stance
    }

    pub fn set_default_stance(&mut self, stance: LegStances) {
        self.default_stance = stance;
    }

    pub fn calculate_pose_angles(&self) -> Vec<(LegId, LegAngles)> {
        let posed = self.pose_hexapod();
        self.current_leg_angles_from_hexapod(&posed)
    }

    pub fn current_leg_angles(&self) -> Vec<(LegId, LegAngles)> {
        all_legs()
            .into_iter()
            .map(|leg| (leg, target_angles_with_offsets(&self.math_hexapod, leg, &self.constraints)))
            .collect()
    }

    pub fn current_leg_angles_from_hexapod(&self, hexapod: &MathHexapod) -> Vec<(LegId, LegAngles)> {
        all_legs()
            .into_iter()
            .map(|leg| (leg, target_angles_with_offsets(hexapod, leg, &self.constraints)))
            .collect()
    }

    pub fn simulate_hexapod(&self, input: &InputState, dt: f32) -> (MathHexapod, WalkState) {
        let mut hexapod = self.math_hexapod.clone();
        let mut walk = self.walk_state.clone();
        let _ = step_hexapod(
            &mut hexapod,
            &mut walk,
            input,
            &self.gait_config,
            dt,
            1.0,
            0.0,
            0.0,
        );
        (hexapod, walk)
    }

    pub fn pose_hexapod(&self) -> MathHexapod {
        let mut hexapod = self.math_hexapod.clone();
        for leg_id in all_legs() {
            let pos = self.body_pose.transform_position(self.default_stance.get(leg_id));
            let leg = match leg_id {
                LegId::LeftFront => &hexapod.legs.left_front,
                LegId::LeftMiddle => &hexapod.legs.left_middle,
                LegId::LeftBack => &hexapod.legs.left_back,
                LegId::RightFront => &hexapod.legs.right_front,
                LegId::RightMiddle => &hexapod.legs.right_middle,
                LegId::RightBack => &hexapod.legs.right_back,
            };

            let mount_angle = match leg_id {
                LegId::LeftFront | LegId::LeftMiddle | LegId::LeftBack => -leg.mount_angle,
                _ => leg.mount_angle,
            };
            let rot = Mat2::from_angle(mount_angle.to_radians());
            let xy = rot.transpose() * glam::vec2(pos.x, pos.y);
            let foot = Vec3::new(xy.x, xy.y, pos.z);

            let geom = Geometry {
                coxa_len: leg.coxa_length,
                femur_len: leg.femur_length,
                tibia_len: leg.tibia_length,
                coxa_attach_angle: 0.0,
                femur_attach_angle: 0.0,
                tibia_attach_angle: 0.0,
            };

            let angles = ik_solve_leg(geom, foot);
            set_leg_targets(
                &mut hexapod,
                leg_id,
                LegAngles {
                    coxa: angles.coxa,
                    femur: angles.femur,
                    tibia: angles.tibia,
                },
            );
        }

        hexapod
    }

    pub fn leg_positions_from_hexapod(&self, hexapod: &MathHexapod) -> LegStances {
        let left_front = compute_leg_joints(&hexapod.legs.left_front, 1.0)[3];
        let left_middle = compute_leg_joints(&hexapod.legs.left_middle, 1.0)[3];
        let left_back = compute_leg_joints(&hexapod.legs.left_back, 1.0)[3];
        let right_front = compute_leg_joints(&hexapod.legs.right_front, 1.0)[3];
        let right_middle = compute_leg_joints(&hexapod.legs.right_middle, 1.0)[3];
        let right_back = compute_leg_joints(&hexapod.legs.right_back, 1.0)[3];

        LegStances {
            left_front,
            left_middle,
            left_back,
            right_front,
            right_middle,
            right_back,
        }
    }
}

fn apply_constraints_to_hexapod(hexapod: &mut MathHexapod, constraints: &Constraints) {
    for leg in [
        &mut hexapod.legs.left_front,
        &mut hexapod.legs.left_middle,
        &mut hexapod.legs.left_back,
        &mut hexapod.legs.right_front,
        &mut hexapod.legs.right_middle,
        &mut hexapod.legs.right_back,
    ] {
        leg.coxa_length = constraints.coxa_length;
        leg.femur_length = constraints.femur_length;
        leg.tibia_length = constraints.tibia_length;
    }
}

fn all_legs() -> [LegId; 6] {
    [
        LegId::LeftFront,
        LegId::LeftMiddle,
        LegId::LeftBack,
        LegId::RightFront,
        LegId::RightMiddle,
        LegId::RightBack,
    ]
}

fn set_leg_targets(hexapod: &mut MathHexapod, leg_id: LegId, angles: LegAngles) {
    match leg_id {
        LegId::LeftFront => hexapod.legs.left_front.set_target_angles(angles.coxa, angles.femur, angles.tibia),
        LegId::LeftMiddle => hexapod.legs.left_middle.set_target_angles(angles.coxa, angles.femur, angles.tibia),
        LegId::LeftBack => hexapod.legs.left_back.set_target_angles(angles.coxa, angles.femur, angles.tibia),
        LegId::RightFront => hexapod.legs.right_front.set_target_angles(angles.coxa, angles.femur, angles.tibia),
        LegId::RightMiddle => hexapod.legs.right_middle.set_target_angles(angles.coxa, angles.femur, angles.tibia),
        LegId::RightBack => hexapod.legs.right_back.set_target_angles(angles.coxa, angles.femur, angles.tibia),
    }
}

fn target_angles_with_offsets(hexapod: &MathHexapod, leg_id: LegId, constraints: &Constraints) -> LegAngles {
    let leg = match leg_id {
        LegId::LeftFront => &hexapod.legs.left_front,
        LegId::LeftMiddle => &hexapod.legs.left_middle,
        LegId::LeftBack => &hexapod.legs.left_back,
        LegId::RightFront => &hexapod.legs.right_front,
        LegId::RightMiddle => &hexapod.legs.right_middle,
        LegId::RightBack => &hexapod.legs.right_back,
    };

    LegAngles {
        coxa: leg.target_coxa_angle + 90.0 + constraints.coxa_soffset,
        femur: 180.0 - (leg.target_femur_angle + constraints.femur_soffset),
        tibia: 180.0 - (leg.target_tibia_angle + constraints.tibia_soffset),
    }
}

/// Control inputs for the hexapod - can be set by any control interface
#[derive(Debug, Clone, Copy)]
pub struct HexapodControl {
    pub velocity: Vec3,      // X=forward, Y=strafe (Z unused)
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
    imu: Option<Arc<Mutex<Box<dyn Imu>>>>,
    smoothed_velocity: Vec3,
    smoothed_rotation: f32,
}

impl Hexapod {
    /// Create a new hexapod controller
    pub fn new(
        servo_pins: ServoPins,
        servo_offsets: ServoOffsets,
        ik_constraints: Constraints,
        initial_gait: GaitType,
        default_stance: Option<LegStances>,
    ) -> Self {
        let servo_controller = ServoController::new(servo_pins, servo_offsets);
        let gait_controller = GaitController::new(initial_gait, ik_constraints, default_stance);

        // Initialize battery monitor (will gracefully handle if not available)
        let ubec_port = std::env::var("UBEC_PORT").unwrap_or_else(|_| UBEC_PORT.to_string());
        let mut ubec_controller = PicoUbecController::new(&ubec_port);

        // Enable servos on startup
        ubec_controller.enable_servos();

        // Initialize IMU
        let imu: Option<Arc<Mutex<Box<dyn Imu>>>> = if IMU_ENABLE {
            match imu_driver::new(IMU_I2C_BUS, IMU_I2C_ADR) {
                Ok(driver) => {
                    println!("IMU initialized successfully");
                    Some(Arc::new(Mutex::new(Box::new(driver))))
                }
                Err(e) => {
                    eprintln!("Failed to initialize IMU: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            servo_controller: Arc::new(Mutex::new(servo_controller)),
            gait_controller: Arc::new(Mutex::new(gait_controller)),
            ubec_controller: Arc::new(Mutex::new(ubec_controller)),
            control: Arc::new(Mutex::new(HexapodControl::default())),
            servo_angle_tweaks: Arc::new(Mutex::new(ServoAngleTweaks::default())),
            imu,
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

    /// Get shared reference to IMU (if available)
    pub fn get_imu(&self) -> Option<Arc<Mutex<Box<dyn Imu>>>> {
        self.imu.clone()
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

        // Snap to zero when we've decayed below deadzones to prevent "walking in place".
        if self.smoothed_velocity.length_squared() < VEL_DEADZONE * VEL_DEADZONE
            && self.smoothed_rotation.abs() < ROT_DEADZONE
        {
            self.smoothed_velocity = Vec3::ZERO;
            self.smoothed_rotation = 0.0;
        }

        let is_moving =
            self.smoothed_velocity.length_squared() > 0.0 || self.smoothed_rotation != 0.0;

        let mut gait = self.gait_controller.lock().await;
        gait.set_body_pose(control.body_pose);

        let input_state = control_to_input(self.smoothed_velocity, self.smoothed_rotation);

        let angles = if is_moving {
            gait.update(&input_state, dt);
            gait.current_leg_angles()
        } else {
            gait.calculate_pose_angles()
        };
        drop(gait);

        // Apply per-servo angle tweaks
        let tweaks = { self.servo_angle_tweaks.lock().await.clone() };
        let mut adjusted = angles;
        for (leg, leg_angles) in adjusted.iter_mut() {
            let t = match leg {
                LegId::LeftFront => tweaks.left_front,
                LegId::LeftMiddle => tweaks.left_middle,
                LegId::LeftBack => tweaks.left_back,
                LegId::RightFront => tweaks.right_front,
                LegId::RightMiddle => tweaks.right_middle,
                LegId::RightBack => tweaks.right_back,
            };
            leg_angles.coxa = (leg_angles.coxa + t.coxa).clamp(SERVO_ANGLE_MIN, SERVO_ANGLE_MAX);
            leg_angles.femur = (leg_angles.femur + t.femur).clamp(SERVO_ANGLE_MIN, SERVO_ANGLE_MAX);
            leg_angles.tibia = (leg_angles.tibia + t.tibia).clamp(SERVO_ANGLE_MIN, SERVO_ANGLE_MAX);
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
    pub async fn set_gait(&self, gait_type: GaitType) {
        self.gait_controller.lock().await.set_gait(gait_type);
    }

    /// Get current gait phase (0.0 to 1.0)
    pub async fn get_gait_phase(&self) -> f32 {
        self.gait_controller.lock().await.get_gait_phase()
    }

    /// Get current gait template name
    pub async fn get_gait_template_name(&self) -> String {
        self.gait_controller.lock().await.get_gait_name()
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
    initial_gait: GaitType,
    default_stance: Option<LegStances>,
}

impl HexapodBuilder {
    pub fn new(
        servo_pins: ServoPins,
        servo_offsets: ServoOffsets,
        ik_constraints: Constraints,
        initial_gait: GaitType,
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
