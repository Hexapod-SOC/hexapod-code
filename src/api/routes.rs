use crate::config::{
    calibration_gait_configs_path, calibration_leg_stance_path, calibration_servo_tweaks_path,
};
use axum::{Json, extract::{Query, State}, http::StatusCode};
use devices::lidar::SlamSnapshot;
use glam::Vec3;
use hexmath::hexapod::LegId;
use hexmath::{get_leg_phase_offsets, GaitConfig, GaitType, WalkState};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use super::state::AppState;
use crate::hexapod::{BodyPose, LegStances, ServoAngleTriplet, ServoAngleTweaks};

#[derive(Serialize, Clone, Copy)]
pub struct BodyPoseData {
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Serialize, Clone, Copy)]
pub struct LegKinematicsData {
    pub position: [f32; 3],
    pub angles_deg: [f32; 3],
    pub angles_tweaked_deg: [f32; 3],
    pub angles_rad: [f32; 3],
}

#[derive(Serialize, Clone)]
pub struct LegKinematicsResponse {
    pub gait_phase: f32,
    pub gait_name: String,
    pub velocity: [f32; 3],
    pub rotation: f32,
    pub body_pose: BodyPoseData,
    pub legs: LegsKinematicsBlock,
}

#[derive(Serialize, Clone, Copy)]
pub struct LegsKinematicsBlock {
    pub left_front: LegKinematicsData,
    pub left_middle: LegKinematicsData,
    pub left_back: LegKinematicsData,
    pub right_front: LegKinematicsData,
    pub right_middle: LegKinematicsData,
    pub right_back: LegKinematicsData,
}

fn vec3_to_array(v: Vec3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

fn tweak_for_leg(tweaks: &ServoAngleTweaks, leg: LegId) -> ServoAngleTriplet {
    match leg {
        LegId::LeftFront => tweaks.left_front,
        LegId::LeftMiddle => tweaks.left_middle,
        LegId::LeftBack => tweaks.left_back,
        LegId::RightFront => tweaks.right_front,
        LegId::RightMiddle => tweaks.right_middle,
        LegId::RightBack => tweaks.right_back,
    }
}

fn to_visualizer_angles(leg: LegId, angles_deg: ServoAngleTriplet) -> [f32; 3] {
    let mut coxa = angles_deg.coxa;
    let mut femur = angles_deg.femur;
    let mut tibia = 180.0 + (angles_deg.tibia);


    if matches!(leg, LegId::RightFront | LegId::RightMiddle | LegId::RightBack) {
        coxa = -coxa;
        femur = femur; //45.0 + 
        tibia = tibia;
    }

    if matches!(leg, LegId::LeftFront | LegId::LeftMiddle | LegId::LeftBack) {
        coxa = 135.0 - coxa;
        femur = 90.0 + femur; //-45.0 - 
        tibia =  270.0 +  tibia; 
    }

    let coxa = coxa.to_radians();
    let femur = femur.to_radians();
    let tibia = tibia.to_radians();

    [coxa, femur, tibia]
}

// ============= Status Endpoints =============

#[derive(Serialize)]
pub struct BatteryStatusResponse {
    pub voltage: f32,
    pub current: f32,
    pub power_state: String,
    pub has_data: bool,
}

#[derive(Serialize)]
pub struct HexapodStatusResponse {
    pub battery: BatteryStatusResponse,
    pub gait_phase: f32,
    pub gait_name: String,
}

/// GET /api/status
pub async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<HexapodStatusResponse>, StatusCode> {
    let mut ubec = state.ubec_controller.lock().await;
    ubec.update();

    let battery_status = ubec.get_battery_status();
    let power_state = ubec.get_power_state();
    let has_data = battery_status.last_update.is_some() || battery_status.voltage > 0.1;

    let gait = state.gait_controller.lock().await;
    let phase = gait.get_gait_phase();
    let gait_name = gait.get_gait_name();

    Ok(Json(HexapodStatusResponse {
        battery: BatteryStatusResponse {
            voltage: battery_status.voltage,
            current: battery_status.current,
            power_state: format!("{:?}", power_state),
            has_data,
        },
        gait_phase: phase,
        gait_name,
    }))
}

/// GET /api/legs
pub async fn get_leg_kinematics(
    State(state): State<Arc<AppState>>,
) -> Result<Json<LegKinematicsResponse>, StatusCode> {
    let control = state.control.lock().await;
    let velocity = control.velocity;
    let rotation = control.rotation;

    let gait = state.gait_controller.lock().await;
    let body_pose = gait.get_body_pose();
    let phase = gait.get_gait_phase();
    let gait_name = gait.get_gait_name();

    let is_moving = velocity.length_squared() > 0.0 || rotation != 0.0;
    let input_state = crate::hexapod::control_to_input(velocity, rotation);

    let (sim_hexapod, sim_walk) = if is_moving {
        gait.simulate_hexapod(&input_state, 0.0)
    } else {
        (gait.pose_hexapod(), WalkState::default())
    };

    let mut positions = gait.leg_positions_from_hexapod(&sim_hexapod);

    let legs = [
        LegId::LeftFront,
        LegId::LeftMiddle,
        LegId::LeftBack,
        LegId::RightFront,
        LegId::RightMiddle,
        LegId::RightBack,
    ];

    for leg in legs {
        let pos = positions.get(leg);
        let transformed = body_pose.transform_position(pos) + Vec3::new(sim_walk.body_pos.x, 0.0, sim_walk.body_pos.z);
        positions.set(leg, transformed);
    }

    let angles = if is_moving {
        gait.current_leg_angles_from_hexapod(&sim_hexapod)
    } else {
        gait.calculate_pose_angles()
    };

    let tweaks = state.servo_angle_tweaks.lock().await.clone();

    let angle_for = |leg: LegId| -> ServoAngleTriplet {
        let (_, found) = angles
            .iter()
            .find(|(l, _)| *l == leg)
            .expect("Missing leg angles");
        ServoAngleTriplet {
            coxa: found.coxa,
            femur: found.femur,
            tibia: found.tibia,
        }
    };

    let build_leg = |leg: LegId, pos: Vec3| -> LegKinematicsData {
        let raw = angle_for(leg);
        let tweak = tweak_for_leg(&tweaks, leg);
        let tweaked = ServoAngleTriplet {
            coxa: raw.coxa + tweak.coxa,
            femur: raw.femur + tweak.femur,
            tibia: raw.tibia + tweak.tibia,
        };

        LegKinematicsData {
            position: vec3_to_array(pos),
            angles_deg: [raw.coxa, raw.femur, raw.tibia],
            angles_tweaked_deg: [tweaked.coxa, tweaked.femur, tweaked.tibia],
            angles_rad: to_visualizer_angles(leg, tweaked),
        }
    };

    let legs_block = LegsKinematicsBlock {
        left_front: build_leg(LegId::LeftFront, positions.left_front),
        left_middle: build_leg(LegId::LeftMiddle, positions.left_middle),
        left_back: build_leg(LegId::LeftBack, positions.left_back),
        right_front: build_leg(LegId::RightFront, positions.right_front),
        right_middle: build_leg(LegId::RightMiddle, positions.right_middle),
        right_back: build_leg(LegId::RightBack, positions.right_back),
    };

    Ok(Json(LegKinematicsResponse {
        gait_phase: phase,
        gait_name,
        velocity: [velocity.x, velocity.y, velocity.z],
        rotation,
        body_pose: BodyPoseData {
            roll: body_pose.rotation.x,
            pitch: body_pose.rotation.y,
            yaw: body_pose.rotation.z,
            x: body_pose.translation.x,
            y: body_pose.translation.y,
            z: body_pose.translation.z,
        },
        legs: legs_block,
    }))
}

/// GET /api/battery
pub async fn get_battery(
    State(state): State<Arc<AppState>>,
) -> Result<Json<BatteryStatusResponse>, StatusCode> {
    let mut ubec = state.ubec_controller.lock().await;
    ubec.update();

    let battery_status = ubec.get_battery_status();
    let power_state = ubec.get_power_state();
    let has_data = battery_status.last_update.is_some() || battery_status.voltage > 0.1;

    Ok(Json(BatteryStatusResponse {
        voltage: battery_status.voltage,
        current: battery_status.current,
        power_state: format!("{:?}", power_state),
        has_data,
    }))
}

// ============= Movement Control Endpoints =============

#[derive(Deserialize)]
pub struct MoveRequest {
    pub forward: f32,  // -100.0 to 100.0 (mm/s)
    pub strafe: f32,   // -100.0 to 100.0 (mm/s)
    pub rotation: f32, // -1.0 to 1.0 (rad/s)
}

#[derive(Serialize)]
pub struct MoveResponse {
    pub success: bool,
    pub message: String,
}

/// POST /api/move
/// Just updates the control velocity - hexapod.update() does the actual movement
pub async fn move_hexapod(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MoveRequest>,
) -> Result<Json<MoveResponse>, StatusCode> {
    let mut control = state.control.lock().await;
    // X=forward/back, Y=left/right (strafe), Z=up/down
    control.velocity = Vec3::new(payload.forward, payload.strafe, 0.0);
    control.rotation = payload.rotation;
    drop(control);

    // Ensure servos are enabled so the robot actually moves!
    let mut ubec = state.ubec_controller.lock().await;
    ubec.enable_servos();

    Ok(Json(MoveResponse {
        success: true,
        message: format!(
            "Control set: forward={:.1}, strafe={:.1}, rotation={:.2}",
            payload.forward, payload.strafe, payload.rotation
        ),
    }))
}

#[derive(Deserialize)]
pub struct StopRequest {}

#[derive(Deserialize)]
pub struct EStopRequest {}

/// POST /api/stop
pub async fn stop_hexapod(
    State(state): State<Arc<AppState>>,
    Json(_payload): Json<StopRequest>,
) -> Result<Json<MoveResponse>, StatusCode> {
    let mut control = state.control.lock().await;
    control.velocity = Vec3::ZERO;
    control.rotation = 0.0;
    drop(control);

    // Disable servos (cuts relay power) via UBEC
    let mut ubec = state.ubec_controller.lock().await;
    ubec.disable_servos();
    ubec.send_shutdown(30);

    if let Err(error) = std::process::Command::new("sudo").arg("poweroff").spawn() {
        eprintln!("Failed to execute poweroff: {}", error);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(Json(MoveResponse {
        success: true,
        message: "Stop: shutdown scheduled (30s) and poweroff initiated".to_string(),
    }))
}

/// POST /api/estop
pub async fn estop_hexapod(
    State(state): State<Arc<AppState>>,
    Json(_payload): Json<EStopRequest>,
) -> Result<Json<MoveResponse>, StatusCode> {
    let mut control = state.control.lock().await;
    control.velocity = Vec3::ZERO;
    control.rotation = 0.0;
    drop(control);

    let mut ubec = state.ubec_controller.lock().await;
    if !ubec.is_connected() {
        eprintln!("E-Stop requested but UBEC is not connected");
    }
    ubec.disable_servos();
    ubec.send_shutdown(1);

    Ok(Json(MoveResponse {
        success: true,
        message: "E-Stop: shutdown command sent (1s)".to_string(),
    }))
}

// ============= Gait Control Endpoints =============

#[derive(Deserialize)]
pub struct SetGaitRequest {
    pub gait_name: String, // "tri", "wave", "ripple", "bi", "quad", "hop"
}

#[derive(Serialize)]
pub struct GaitResponse {
    pub success: bool,
    pub message: String,
    pub current_gait: String,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct GaitConfigData {
    pub duty_factor: f32,
    pub speed: f32,
    pub step_length_mm: f32,
    pub step_height_mm: f32,
    pub base_height_mm: f32,
    pub body_push_gain: f32,
    pub phase_offsets: [f32; 6],
    pub max_step_length: f32,
    pub max_speed: f32,
}

#[derive(Serialize)]
pub struct GaitConfigResponse {
    pub success: bool,
    pub message: String,
    pub gait_name: String,
    pub config: GaitConfigData,
}

#[derive(Deserialize)]
pub struct GaitConfigQuery {
    pub gait_name: String,
}

#[derive(Deserialize)]
pub struct SetGaitConfigRequest {
    pub gait_name: String,
    pub config: GaitConfigData,
}

fn parse_gait_name(name: &str) -> Option<GaitType> {
    match name.to_lowercase().as_str() {
        "tripod" | "tri" | "t"    => Some(GaitType::Tripod),
        "tetrapod" | "quad" | "bi" => Some(GaitType::Tetrapod),
        "wave" | "w"               => Some(GaitType::Wave),
        "ripple" | "r"             => Some(GaitType::Ripple),
        "crawl" | "c" | "slope"    => Some(GaitType::Crawl),
        _ => None,
    }
}

/// POST /api/gait
pub async fn set_gait(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SetGaitRequest>,
) -> Result<Json<GaitResponse>, StatusCode> {
    let gait_type = parse_gait_name(&payload.gait_name).ok_or(StatusCode::BAD_REQUEST)?;
    let mut gait_controller = state.gait_controller.lock().await;
    gait_controller.set_gait(gait_type);

    Ok(Json(GaitResponse {
        success: true,
        message: format!("Gait changed to {}", gait_type.name()),
        current_gait: gait_type.name().to_string(),
    }))
}

/// GET /api/gait
pub async fn get_gait(
    State(state): State<Arc<AppState>>,
) -> Result<Json<GaitResponse>, StatusCode> {
    let gait = state.gait_controller.lock().await;
    let gait_name = gait.get_gait_name();

    Ok(Json(GaitResponse {
        success: true,
        message: "Current gait".to_string(),
        current_gait: gait_name,
    }))
}

/// GET /api/gait_config?gait_name=Tripod
pub async fn get_gait_config(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GaitConfigQuery>,
) -> Result<Json<GaitConfigResponse>, StatusCode> {
    let gait_type = parse_gait_name(&query.gait_name).ok_or(StatusCode::BAD_REQUEST)?;
    let gait_key = match gait_type {
        GaitType::Tripod   => "tripod",
        GaitType::Tetrapod => "tetrapod",
        GaitType::Wave     => "wave",
        GaitType::Ripple   => "ripple",
        GaitType::Crawl    => "crawl",
    };

    if let Ok(content) = fs::read_to_string(calibration_gait_configs_path()).await {
        if let Ok(saved) = serde_json::from_str::<std::collections::HashMap<String, GaitConfigData>>(&content) {
            if let Some(config) = saved.get(gait_key) {
                return Ok(Json(GaitConfigResponse {
                    success: true,
                    message: "Gait config".to_string(),
                    gait_name: gait_type.name().to_string(),
                    config: *config,
                }));
            }
        }
    }

    let gait_controller = state.gait_controller.lock().await;
    let config = gait_controller.get_gait_config_for(gait_type);

    let phase_offsets = if let Some(override_offsets) = config.phase_offsets_override {
        override_offsets
    } else {
        let offsets = get_leg_phase_offsets(gait_type, &config.disabled_legs, None);
        [
            offsets[0].1,
            offsets[1].1,
            offsets[2].1,
            offsets[3].1,
            offsets[4].1,
            offsets[5].1,
        ]
    };

    Ok(Json(GaitConfigResponse {
        success: true,
        message: "Gait config".to_string(),
        gait_name: gait_type.name().to_string(),
        config: GaitConfigData {
            duty_factor: config.duty_factor,
            speed: config.speed,
            step_length_mm: config.step_length,
            step_height_mm: config.step_height,
            base_height_mm: config.base_height,
            body_push_gain: config.body_push_gain,
            phase_offsets,
            max_step_length: 0.0,
            max_speed: 0.0,
        },
    }))
}

/// POST /api/gait_config
pub async fn set_gait_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SetGaitConfigRequest>,
) -> Result<Json<GaitConfigResponse>, StatusCode> {
    let gait_type = parse_gait_name(&payload.gait_name).ok_or(StatusCode::BAD_REQUEST)?;
    let mut gait_controller = state.gait_controller.lock().await;
    let mut config = gait_controller.get_gait_config_for(gait_type);

    let mut offsets = payload.config.phase_offsets;
    for value in offsets.iter_mut() {
        *value = value.clamp(0.0, 1.0);
    }

    config.gait_type = gait_type;
    config.phase_offsets_override = Some(offsets);
    config.duty_factor = payload.config.duty_factor.clamp(0.05, 0.95);
    let max_step_length = payload
        .config
        .max_step_length
        .max(payload.config.step_length_mm)
        .max(0.0);
    let max_speed = payload
        .config
        .max_speed
        .max(payload.config.speed)
        .max(0.1);

    config.step_length = payload.config.step_length_mm.clamp(0.0, max_step_length);
    config.step_height = payload.config.step_height_mm.max(0.0);
    config.speed = payload.config.speed.clamp(0.1, max_speed);
    config.base_height = payload.config.base_height_mm.clamp(-300.0, 0.0);
    config.body_push_gain = payload.config.body_push_gain.clamp(0.0, 10.0);

    gait_controller.set_gait_config_for(gait_type, config.clone());

    let gait_key = match gait_type {
        GaitType::Tripod   => "tripod",
        GaitType::Tetrapod => "tetrapod",
        GaitType::Wave     => "wave",
        GaitType::Ripple   => "ripple",
        GaitType::Crawl    => "crawl",
    };

    let mut all_configs: std::collections::HashMap<String, GaitConfigData> =
        if let Ok(content) = fs::read_to_string(calibration_gait_configs_path()).await {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            std::collections::HashMap::new()
        };

    all_configs.insert(
        gait_key.to_string(),
        GaitConfigData {
            duty_factor: config.duty_factor,
            speed: config.speed,
            step_length_mm: config.step_length,
            step_height_mm: config.step_height,
            base_height_mm: config.base_height,
            body_push_gain: config.body_push_gain,
            phase_offsets: offsets,
            max_step_length,
            max_speed,
        },
    );

    let path = calibration_gait_configs_path();
    if let Some(dir) = path.parent() {
        if let Err(e) = fs::create_dir_all(dir).await {
            eprintln!("Failed to create calibration dir: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    let json = serde_json::to_string_pretty(&all_configs)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut file = fs::File::create(&path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    file.write_all(json.as_bytes())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(GaitConfigResponse {
        success: true,
        message: "Gait config updated".to_string(),
        gait_name: gait_type.name().to_string(),
        config: GaitConfigData {
            duty_factor: config.duty_factor,
            speed: config.speed,
            step_length_mm: config.step_length,
            step_height_mm: config.step_height,
            base_height_mm: config.base_height,
            body_push_gain: config.body_push_gain,
            phase_offsets: offsets,
            max_step_length,
            max_speed,
        },
    }))
}

// ============= Custom Gait Endpoint =============

#[derive(Deserialize)]
pub struct CustomLegOffsetsPayload {
    pub left_front: f32,
    pub left_middle: f32,
    pub left_back: f32,
    pub right_front: f32,
    pub right_middle: f32,
    pub right_back: f32,
}

#[derive(Deserialize)]
pub struct SetCustomGaitRequest {
    pub name: String,
    pub leg_cycle_offsets: CustomLegOffsetsPayload,
    #[serde(default)]
    pub push_fraction: f32,
    #[serde(default)]
    pub speed_multiplier: f32,
    #[serde(default)]
    pub step_length_multiplier: f32,
    #[serde(default)]
    pub lift_height_multiplier: f32,
    #[serde(default)]
    pub max_step_length: f32,
    #[serde(default)]
    pub max_speed: f32,
    #[serde(default)]
    pub duty_factor: Option<f32>,
    #[serde(default)]
    pub speed: Option<f32>,
    #[serde(default)]
    pub step_length_mm: Option<f32>,
    #[serde(default)]
    pub step_height_mm: Option<f32>,
    #[serde(default)]
    pub base_height_mm: Option<f32>,
    #[serde(default)]
    pub body_push_gain: Option<f32>,
}

/// POST /api/custom_gait
pub async fn set_custom_gait(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SetCustomGaitRequest>,
) -> Result<Json<GaitResponse>, StatusCode> {
    let base = GaitConfig::default();
    let offsets = [
        payload.leg_cycle_offsets.left_front,
        payload.leg_cycle_offsets.left_middle,
        payload.leg_cycle_offsets.left_back,
        payload.leg_cycle_offsets.right_front,
        payload.leg_cycle_offsets.right_middle,
        payload.leg_cycle_offsets.right_back,
    ];

    let mut gait_controller = state.gait_controller.lock().await;
    let use_absolute = payload.duty_factor.is_some()
        || payload.speed.is_some()
        || payload.step_length_mm.is_some()
        || payload.step_height_mm.is_some()
        || payload.base_height_mm.is_some()
        || payload.body_push_gain.is_some();

    let max_step_length = if payload.max_step_length > 0.0 {
        payload.max_step_length
    } else {
        base.step_length * 3.0
    };
    let max_speed = if payload.max_speed > 0.0 {
        payload.max_speed
    } else {
        base.speed * 5.0
    };

    if use_absolute {
        let duty = payload.duty_factor.unwrap_or(payload.push_fraction.max(0.0));
        let speed = payload.speed.unwrap_or(base.speed * payload.speed_multiplier.max(0.0));
        let step_length =
            payload.step_length_mm.unwrap_or(base.step_length * payload.step_length_multiplier.max(0.0));
        let step_height =
            payload.step_height_mm.unwrap_or(base.step_height * payload.lift_height_multiplier.max(0.0));
        let base_height = payload.base_height_mm.unwrap_or(base.base_height);
        let body_push_gain = payload.body_push_gain.unwrap_or(base.body_push_gain);

        gait_controller.set_custom_gait_absolute(
            payload.name.clone(),
            offsets,
            duty,
            speed,
            step_length,
            step_height,
            base_height,
            body_push_gain,
            max_step_length,
            max_speed,
        );
    } else {
        gait_controller.set_custom_gait(
            payload.name.clone(),
            offsets,
            payload.push_fraction,
            payload.speed_multiplier,
            payload.step_length_multiplier,
            payload.lift_height_multiplier,
            max_step_length,
            max_speed,
        );
    }

    Ok(Json(GaitResponse {
        success: true,
        message: format!("Custom gait applied: {}", payload.name),
        current_gait: payload.name,
    }))
}

// ============= Body Pose Endpoints =============

#[derive(Deserialize)]
pub struct BodyPoseRequest {
    pub roll: f32,  // degrees
    pub pitch: f32, // degrees
    pub yaw: f32,   // degrees
}

#[derive(Serialize)]
pub struct BodyPoseResponse {
    pub success: bool,
    pub message: String,
}

/// POST /api/pose
pub async fn set_body_pose(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BodyPoseRequest>,
) -> Result<Json<BodyPoseResponse>, StatusCode> {
    let pose = BodyPose::with_rotation(payload.roll, payload.pitch, payload.yaw);

    let mut control = state.control.lock().await;
    control.body_pose = pose;

    Ok(Json(BodyPoseResponse {
        success: true,
        message: format!(
            "Body pose set: roll={:.1}°, pitch={:.1}°, yaw={:.1}°",
            payload.roll, payload.pitch, payload.yaw
        ),
    }))
}

// ============= Text-to-Speech Endpoints =============

#[derive(Deserialize)]
pub struct TTSRequest {
    pub text: String,
    #[serde(default)]
    pub voice: Option<String>, // Optional: "en_US-ryan-medium", "sk_SK-lili-medium", etc.
}

#[derive(Serialize)]
pub struct TTSResponse {
    pub success: bool,
    pub message: String,
}

/// POST /api/tts
pub async fn speak_text(Json(payload): Json<TTSRequest>) -> Result<Json<TTSResponse>, StatusCode> {
    use audio::tts;

    // Spawn TTS in a background task since it might take time
    let text = payload.text.clone();
    let voice = payload.voice.clone();

    tokio::task::spawn_blocking(move || {
        let result = if let Some(v) = voice.as_deref() {
            tts::say(&text, Some(v))
        } else {
            tts::sayen(&text)
        };

        if let Err(e) = result {
            eprintln!("TTS error: {}", e);
        }
    });

    Ok(Json(TTSResponse {
        success: true,
        message: format!("Speaking: '{}'", payload.text),
    }))
}

// ============= Leg Calibration Endpoints =============

#[derive(Deserialize)]
pub struct SetLegStanceRequest {
    pub left_front: [f32; 3], // [x, y, z]
    pub left_middle: [f32; 3],
    pub left_back: [f32; 3],
    pub right_front: [f32; 3],
    pub right_middle: [f32; 3],
    pub right_back: [f32; 3],
    #[serde(default)]
    pub print_to_console: Option<bool>,
}

#[derive(Serialize)]
pub struct LegStanceResponse {
    pub success: bool,
    pub message: String,
    pub current_stance: LegStancesData,
}

#[derive(Serialize)]
pub struct LegStancesData {
    pub left_front: [f32; 3],
    pub left_middle: [f32; 3],
    pub left_back: [f32; 3],
    pub right_front: [f32; 3],
    pub right_middle: [f32; 3],
    pub right_back: [f32; 3],
}

// ============= LiDAR SLAM Endpoints =============

#[derive(Serialize, Default, Clone, Copy)]
pub struct PoseResponse {
    pub x: f32,
    pub y: f32,
    pub theta: f32,
}

#[derive(Serialize, Clone, Copy)]
pub struct LidarPointResponse {
    pub angle_deg: f32,
    pub distance_mm: u32,
    pub intensity: u16,
}

#[derive(Serialize)]
pub struct LidarFrameResponse {
    pub frame: u64,
    pub timestamp_ns: u64,
    pub pose: PoseResponse,
    pub rpm: f32,
    pub points: Vec<LidarPointResponse>,
}

#[derive(Serialize)]
pub struct LidarMapResponse {
    pub frame: u64,
    pub pose: PoseResponse,
    pub width: usize,
    pub height: usize,
    pub resolution: f32,
    pub origin: PoseResponse,
    pub cells: Vec<i8>,
}

/// GET /api/leg_stance
pub async fn get_leg_stance(
    State(state): State<Arc<AppState>>,
) -> Result<Json<LegStanceResponse>, StatusCode> {
    let gait = state.gait_controller.lock().await;
    let stance = gait.get_default_stance();

    Ok(Json(LegStanceResponse {
        success: true,
        message: "Current leg stance".to_string(),
        current_stance: LegStancesData {
            left_front: stance.to_array(LegId::LeftFront),
            left_middle: stance.to_array(LegId::LeftMiddle),
            left_back: stance.to_array(LegId::LeftBack),
            right_front: stance.to_array(LegId::RightFront),
            right_middle: stance.to_array(LegId::RightMiddle),
            right_back: stance.to_array(LegId::RightBack),
        },
    }))
}

/// POST /api/leg_stance
pub async fn set_leg_stance(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SetLegStanceRequest>,
) -> Result<Json<LegStanceResponse>, StatusCode> {
    let new_stance = LegStances {
        left_front: Vec3::from_array(payload.left_front),
        left_middle: Vec3::from_array(payload.left_middle),
        left_back: Vec3::from_array(payload.left_back),
        right_front: Vec3::from_array(payload.right_front),
        right_middle: Vec3::from_array(payload.right_middle),
        right_back: Vec3::from_array(payload.right_back),
    };

    let mut gait = state.gait_controller.lock().await;
    gait.set_default_stance(new_stance);

    // Optional: Print Rust code format for easy copy-paste into config
    if payload.print_to_console.unwrap_or(false) {
        println!("\n=== Calibrated Leg Stance (copy to constants) ===");
        println!("LegStances {{");
        println!(
            "    left_front: Vec3::new({:.1}, {:.1}, {:.1}),",
            payload.left_front[0], payload.left_front[1], payload.left_front[2]
        );
        println!(
            "    left_middle: Vec3::new({:.1}, {:.1}, {:.1}),",
            payload.left_middle[0], payload.left_middle[1], payload.left_middle[2]
        );
        println!(
            "    left_back: Vec3::new({:.1}, {:.1}, {:.1}),",
            payload.left_back[0], payload.left_back[1], payload.left_back[2]
        );
        println!(
            "    right_front: Vec3::new({:.1}, {:.1}, {:.1}),",
            payload.right_front[0], payload.right_front[1], payload.right_front[2]
        );
        println!(
            "    right_middle: Vec3::new({:.1}, {:.1}, {:.1}),",
            payload.right_middle[0], payload.right_middle[1], payload.right_middle[2]
        );
        println!(
            "    right_back: Vec3::new({:.1}, {:.1}, {:.1}),",
            payload.right_back[0], payload.right_back[1], payload.right_back[2]
        );
        println!("}}");
        println!("==================================================\n");
    }

    // Also persist immediately so no manual copy/paste is needed
    let stance_path = calibration_leg_stance_path();
    if let Some(dir) = stance_path.parent() {
        if let Err(e) = fs::create_dir_all(dir).await {
            eprintln!("Failed to create calibration dir: {}", e);
        }
    }
    let data = serde_json::json!({
        "left_front": payload.left_front,
        "left_middle": payload.left_middle,
        "left_back": payload.left_back,
        "right_front": payload.right_front,
        "right_middle": payload.right_middle,
        "right_back": payload.right_back,
    });
    match fs::File::create(&stance_path).await {
        Ok(mut file) => {
            if let Err(e) = file
                .write_all(serde_json::to_string_pretty(&data).unwrap().as_bytes())
                .await
            {
                eprintln!("Failed to write leg stance file: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to open leg stance file: {}", e);
        }
    }

    Ok(Json(LegStanceResponse {
        success: true,
        message: "Leg stance applied and saved".to_string(),
        current_stance: LegStancesData {
            left_front: payload.left_front,
            left_middle: payload.left_middle,
            left_back: payload.left_back,
            right_front: payload.right_front,
            right_middle: payload.right_middle,
            right_back: payload.right_back,
        },
    }))
}

// ============= Leg Calibration Persistence =============

#[derive(Serialize, Deserialize)]
struct LegStanceFile {
    left_front: [f32; 3],
    left_middle: [f32; 3],
    left_back: [f32; 3],
    right_front: [f32; 3],
    right_middle: [f32; 3],
    right_back: [f32; 3],
}

#[derive(Serialize)]
pub struct SaveResponse {
    pub success: bool,
    pub message: String,
}

/// POST /api/leg_stance/save
pub async fn save_leg_stance(
    Json(payload): Json<SetLegStanceRequest>,
) -> Result<Json<SaveResponse>, StatusCode> {
    // Ensure directory exists
    let stance_path = calibration_leg_stance_path();
    if let Some(dir) = stance_path.parent() {
        if let Err(e) = fs::create_dir_all(dir).await {
            eprintln!("Failed to create calibration dir: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    let data = LegStanceFile {
        left_front: payload.left_front,
        left_middle: payload.left_middle,
        left_back: payload.left_back,
        right_front: payload.right_front,
        right_middle: payload.right_middle,
        right_back: payload.right_back,
    };

    let json = match serde_json::to_string_pretty(&data) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to serialize leg stance: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    match fs::File::create(&stance_path).await {
        Ok(mut file) => {
            if let Err(e) = file.write_all(json.as_bytes()).await {
                eprintln!("Failed to write leg stance file: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
        Err(e) => {
            eprintln!("Failed to open leg stance file: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    Ok(Json(SaveResponse {
        success: true,
        message: "Leg stance saved".to_string(),
    }))
}

/// GET /api/leg_stance/saved
pub async fn get_saved_leg_stance() -> Result<Json<LegStanceResponse>, StatusCode> {
    let path = calibration_leg_stance_path();
    if !path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let content = match fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read leg stance file: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let parsed: LegStanceFile = match serde_json::from_str(&content) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to parse leg stance file: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    Ok(Json(LegStanceResponse {
        success: true,
        message: "Saved leg stance".to_string(),
        current_stance: LegStancesData {
            left_front: parsed.left_front,
            left_middle: parsed.left_middle,
            left_back: parsed.left_back,
            right_front: parsed.right_front,
            right_middle: parsed.right_middle,
            right_back: parsed.right_back,
        },
    }))
}

// ============= IMU Endpoints =============

#[derive(Serialize)]
pub struct ImuResponse {
    pub success: bool,
    pub message: String,
    pub euler: [f32; 3], // Roll, Pitch, Yaw
    pub quat: [f32; 4], // X, Y, Z, W
    pub calibration: u8,
}

/// GET /api/imu
pub async fn get_imu_data(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ImuResponse>, StatusCode> {
    if let Some(imu_arc) = &state.imu {
        let mut imu = imu_arc.lock().await;
        match imu.read_data() {
            Ok(data) => Ok(Json(ImuResponse {
                success: true,
                message: "IMU data".to_string(),
                euler: [data.euler.x, data.euler.y, data.euler.z],
                quat: [data.quat.x, data.quat.y, data.quat.z, data.quat.w],
                calibration: data.calibration,
            })),
            Err(e) => Ok(Json(ImuResponse {
                success: false,
                message: format!("Failed to read IMU: {}", e),
                euler: [0.0; 3],
                quat: [0.0; 4],
                calibration: 0,
            })),
        }
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

// ============= Servo Angle Tweaks (Per-Servo) =============

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct ServoTweaksData {
    pub left_front: [f32; 3], // [coxa, femur, tibia] in degrees
    pub left_middle: [f32; 3],
    pub left_back: [f32; 3],
    pub right_front: [f32; 3],
    pub right_middle: [f32; 3],
    pub right_back: [f32; 3],
}

#[derive(Serialize)]
pub struct ServoTweaksResponse {
    pub success: bool,
    pub message: String,
    pub tweaks: ServoTweaksData,
}

/// GET /api/servo_tweaks
pub async fn get_servo_tweaks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ServoTweaksResponse>, StatusCode> {
    let t = state.servo_angle_tweaks.lock().await.clone();
    let data = ServoTweaksData {
        left_front: [t.left_front.coxa, t.left_front.femur, t.left_front.tibia],
        left_middle: [t.left_middle.coxa, t.left_middle.femur, t.left_middle.tibia],
        left_back: [t.left_back.coxa, t.left_back.femur, t.left_back.tibia],
        right_front: [t.right_front.coxa, t.right_front.femur, t.right_front.tibia],
        right_middle: [
            t.right_middle.coxa,
            t.right_middle.femur,
            t.right_middle.tibia,
        ],
        right_back: [t.right_back.coxa, t.right_back.femur, t.right_back.tibia],
    };
    Ok(Json(ServoTweaksResponse {
        success: true,
        message: "Current servo angle tweaks".to_string(),
        tweaks: data,
    }))
}

/// POST /api/servo_tweaks
pub async fn set_servo_tweaks(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ServoTweaksData>,
) -> Result<Json<ServoTweaksResponse>, StatusCode> {
    let mut t = state.servo_angle_tweaks.lock().await;
    *t = ServoAngleTweaks {
        left_front: ServoAngleTriplet {
            coxa: payload.left_front[0],
            femur: payload.left_front[1],
            tibia: payload.left_front[2],
        },
        left_middle: ServoAngleTriplet {
            coxa: payload.left_middle[0],
            femur: payload.left_middle[1],
            tibia: payload.left_middle[2],
        },
        left_back: ServoAngleTriplet {
            coxa: payload.left_back[0],
            femur: payload.left_back[1],
            tibia: payload.left_back[2],
        },
        right_front: ServoAngleTriplet {
            coxa: payload.right_front[0],
            femur: payload.right_front[1],
            tibia: payload.right_front[2],
        },
        right_middle: ServoAngleTriplet {
            coxa: payload.right_middle[0],
            femur: payload.right_middle[1],
            tibia: payload.right_middle[2],
        },
        right_back: ServoAngleTriplet {
            coxa: payload.right_back[0],
            femur: payload.right_back[1],
            tibia: payload.right_back[2],
        },
    };
    Ok(Json(ServoTweaksResponse {
        success: true,
        message: "Servo angle tweaks applied".to_string(),
        tweaks: payload,
    }))
}

/// POST /api/servo_tweaks/save
pub async fn save_servo_tweaks(
    Json(payload): Json<ServoTweaksData>,
) -> Result<Json<SaveResponse>, StatusCode> {
    // Ensure directory exists
    let tweaks_path = calibration_servo_tweaks_path();
    if let Some(dir) = tweaks_path.parent() {
        if let Err(e) = fs::create_dir_all(dir).await {
            eprintln!("Failed to create calibration dir: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    let json = match serde_json::to_string_pretty(&payload) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to serialize servo tweaks: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    match fs::File::create(&tweaks_path).await {
        Ok(mut file) => {
            if let Err(e) = file.write_all(json.as_bytes()).await {
                eprintln!("Failed to write servo tweaks file: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
        Err(e) => {
            eprintln!("Failed to open servo tweaks file: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    Ok(Json(SaveResponse {
        success: true,
        message: "Servo tweaks saved".to_string(),
    }))
}

/// GET /api/servo_tweaks/saved
pub async fn get_saved_servo_tweaks() -> Result<Json<ServoTweaksResponse>, StatusCode> {
    let path = calibration_servo_tweaks_path();
    if !path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let content = match fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read servo tweaks file: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let parsed: ServoTweaksData = match serde_json::from_str(&content) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to parse servo tweaks file: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    Ok(Json(ServoTweaksResponse {
        success: true,
        message: "Saved servo tweaks".to_string(),
        tweaks: parsed,
    }))
}

fn snapshot_pose(snapshot: &SlamSnapshot) -> PoseResponse {
    PoseResponse {
        x: snapshot.pose.x,
        y: snapshot.pose.y,
        theta: snapshot.pose.theta,
    }
}

/// GET /api/lidar/frame
pub async fn get_lidar_frame(
    State(state): State<Arc<AppState>>,
) -> Result<Json<LidarFrameResponse>, StatusCode> {
    let handle = state
        .lidar
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let snapshot = handle.latest();
    if snapshot.frame == 0 {
        return Err(StatusCode::NO_CONTENT);
    }

    let scan = snapshot
        .last_scan
        .as_ref()
        .ok_or(StatusCode::NO_CONTENT)?;

    let points = scan
        .points
        .iter()
        .map(|p| LidarPointResponse {
            angle_deg: p.angle_deg,
            distance_mm: (p.distance_m.max(0.0) * 1000.0) as u32,
            intensity: p.intensity,
        })
        .collect();

    Ok(Json(LidarFrameResponse {
        frame: snapshot.frame,
        timestamp_ns: snapshot.timestamp_ns,
        pose: snapshot_pose(&snapshot),
        rpm: scan.rpm,
        points,
    }))
}

/// GET /api/lidar/map
pub async fn get_lidar_map(
    State(state): State<Arc<AppState>>,
) -> Result<Json<LidarMapResponse>, StatusCode> {
    let handle = state
        .lidar
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let snapshot = handle.latest();
    let map = snapshot.map.as_ref().ok_or(StatusCode::NO_CONTENT)?;

    Ok(Json(LidarMapResponse {
        frame: snapshot.frame,
        pose: snapshot_pose(&snapshot),
        width: map.width(),
        height: map.height(),
        resolution: map.resolution(),
        origin: PoseResponse {
            x: map.origin().x,
            y: map.origin().y,
            theta: map.origin().theta,
        },
        cells: map.cells().to_vec(),
    }))
}
// ============= Health Check =============

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// GET /api/health
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
