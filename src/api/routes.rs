use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use movement::gaits::{GaitTemplate, LegCycleOffsets};
use std::sync::Arc;
use glam::Vec3;

use super::state::AppState;

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
    
    let gait = state.gait_controller.lock().await;
    let phase = gait.get_gait_phase();
    let template = gait.get_template();
    
    Ok(Json(HexapodStatusResponse {
        battery: BatteryStatusResponse {
            voltage: battery_status.voltage,
            current: battery_status.current,
            power_state: format!("{:?}", power_state),
            has_data: battery_status.last_update.is_some(),
        },
        gait_phase: phase,
        gait_name: template.name.to_string(),
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
    
    Ok(Json(BatteryStatusResponse {
        voltage: battery_status.voltage,
        current: battery_status.current,
        power_state: format!("{:?}", power_state),
        has_data: battery_status.last_update.is_some(),
    }))
}

// ============= Movement Control Endpoints =============

#[derive(Deserialize)]
pub struct MoveRequest {
    pub forward: f32,   // -100.0 to 100.0 (mm/s)
    pub strafe: f32,    // -100.0 to 100.0 (mm/s)
    pub rotation: f32,  // -1.0 to 1.0 (rad/s)
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

/// POST /api/stop
pub async fn stop_hexapod(
    State(state): State<Arc<AppState>>,
    Json(_payload): Json<StopRequest>,
) -> Result<Json<MoveResponse>, StatusCode> {
    let mut control = state.control.lock().await;
    control.velocity = Vec3::ZERO;
    control.rotation = 0.0;
    
    Ok(Json(MoveResponse {
        success: true,
        message: "Movement stopped".to_string(),
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

/// POST /api/gait
pub async fn set_gait(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SetGaitRequest>,
) -> Result<Json<GaitResponse>, StatusCode> {
    use movement::gaits::GAITS;
    
    let mut gait_controller = state.gait_controller.lock().await;
    
    // Find matching gait template
    let template = GAITS.iter()
        .find(|g| g.name == payload.gait_name)
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    gait_controller.set_gait(template);
    
    Ok(Json(GaitResponse {
        success: true,
        message: format!("Gait changed to {}", template.name),
        current_gait: template.name.to_string(),
    }))
}

/// GET /api/gait
pub async fn get_gait(
    State(state): State<Arc<AppState>>,
) -> Result<Json<GaitResponse>, StatusCode> {
    let gait = state.gait_controller.lock().await;
    let template = gait.get_template();
    
    Ok(Json(GaitResponse {
        success: true,
        message: "Current gait".to_string(),
        current_gait: template.name.to_string(),
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
    pub push_fraction: f32,
    pub speed_multiplier: f32,
    pub step_length_multiplier: f32,
    pub lift_height_multiplier: f32,
    pub max_step_length: f32,
    pub max_speed: f32,
}

/// POST /api/custom_gait
pub async fn set_custom_gait(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SetCustomGaitRequest>,
) -> Result<Json<GaitResponse>, StatusCode> {
    // Convert name to 'static str by leaking the String (acceptable for tuning/dev)
    let name_static: &'static str = Box::leak(payload.name.into_boxed_str());

    let offsets = LegCycleOffsets {
        left_front: payload.leg_cycle_offsets.left_front,
        left_middle: payload.leg_cycle_offsets.left_middle,
        left_back: payload.leg_cycle_offsets.left_back,
        right_front: payload.leg_cycle_offsets.right_front,
        right_middle: payload.leg_cycle_offsets.right_middle,
        right_back: payload.leg_cycle_offsets.right_back,
    };

    // Build a GaitTemplate and leak it to get a 'static reference quickly for runtime switching
    let template_box = Box::new(GaitTemplate {
        name: name_static,
        leg_cycle_offsets: offsets,
        push_fraction: payload.push_fraction,
        speed_multiplier: payload.speed_multiplier,
        step_length_multiplier: payload.step_length_multiplier,
        lift_height_multiplier: payload.lift_height_multiplier,
        max_step_length: payload.max_step_length,
        max_speed: payload.max_speed,
    });

    let static_template: &'static GaitTemplate = Box::leak(template_box);

    // Apply new gait
    let mut gait_controller = state.gait_controller.lock().await;
    gait_controller.set_gait(static_template);

    Ok(Json(GaitResponse {
        success: true,
        message: format!("Custom gait applied: {}", static_template.name),
        current_gait: static_template.name.to_string(),
    }))
}

// ============= Body Pose Endpoints =============

#[derive(Deserialize)]
pub struct BodyPoseRequest {
    pub roll: f32,   // degrees
    pub pitch: f32,  // degrees
    pub yaw: f32,    // degrees
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
    use movement::controller::BodyPose;
    
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
pub async fn speak_text(
    Json(payload): Json<TTSRequest>,
) -> Result<Json<TTSResponse>, StatusCode> {
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
