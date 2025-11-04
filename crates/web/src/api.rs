use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use glam::Vec3;

use crate::state::AppState;

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
pub async fn move_hexapod(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MoveRequest>,
) -> Result<Json<MoveResponse>, StatusCode> {
    let mut gait = state.gait_controller.lock().await;
    let mut servo = state.servo_controller.lock().await;
    
    // Update gait
    gait.update(0.05); // 50ms update
    
    // Calculate angles for movement
    let velocity = Vec3::new(payload.forward, 0.0, payload.strafe);
    let angles = gait.calculate_walking_angles(velocity, payload.rotation);
    
    // Apply to servos
    for (leg, leg_angles) in angles.iter() {
        servo.set_leg_angles(*leg, *leg_angles);
    }
    
    Ok(Json(MoveResponse {
        success: true,
        message: format!(
            "Moving: forward={:.1}, strafe={:.1}, rotation={:.2}",
            payload.forward, payload.strafe, payload.rotation
        ),
    }))
}

#[derive(Deserialize)]
pub struct StopRequest {}

/// POST /api/stop
pub async fn stop_hexapod(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<StopRequest>,
) -> Result<Json<MoveResponse>, StatusCode> {
    // Stop movement by setting zero velocity
    // In a real implementation, you'd reset the hexapod to default stance
    
    Ok(Json(MoveResponse {
        success: true,
        message: "Hexapod stopped".to_string(),
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
    
    let mut gait = state.gait_controller.lock().await;
    let mut servo = state.servo_controller.lock().await;
    
    let pose = BodyPose::with_rotation(payload.roll, payload.pitch, payload.yaw);
    gait.set_body_pose(pose);
    
    // Calculate and apply pose angles
    let angles = gait.calculate_pose_angles();
    for (leg, leg_angles) in angles.iter() {
        servo.set_leg_angles(*leg, *leg_angles);
    }
    
    Ok(Json(BodyPoseResponse {
        success: true,
        message: format!(
            "Body pose set: roll={:.1}°, pitch={:.1}°, yaw={:.1}°",
            payload.roll, payload.pitch, payload.yaw
        ),
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
