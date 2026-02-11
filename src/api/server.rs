use axum::{
    Router,
    routing::{get, post},
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber;

use super::routes;
use super::state::AppState;

pub async fn run_server(state: AppState, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let app_state = Arc::new(state);

    // Build our application with routes
    let app = Router::new()
        // Health check
        .route("/api/health", get(routes::health_check))
        // Status endpoints
        .route("/api/status", get(routes::get_status))
        .route("/api/battery", get(routes::get_battery))
        .route("/api/legs", get(routes::get_leg_kinematics))
        // Movement control
        .route("/api/move", post(routes::move_hexapod))
        .route("/api/stop", post(routes::stop_hexapod))
        // Gait control
        .route("/api/gait", get(routes::get_gait))
        .route("/api/gait", post(routes::set_gait))
        .route("/api/gait_config", get(routes::get_gait_config))
        .route("/api/gait_config", post(routes::set_gait_config))
        // Custom gait tuning
        .route("/api/custom_gait", post(routes::set_custom_gait))
        // Leg calibration
        .route("/api/leg_stance", get(routes::get_leg_stance))
        .route("/api/leg_stance", post(routes::set_leg_stance))
        .route("/api/leg_stance/save", post(routes::save_leg_stance))
        .route("/api/leg_stance/saved", get(routes::get_saved_leg_stance))
        // Servo angle tweaks (per-servo calibration)
        .route("/api/servo_tweaks", get(routes::get_servo_tweaks))
        .route("/api/servo_tweaks", post(routes::set_servo_tweaks))
        .route("/api/servo_tweaks/save", post(routes::save_servo_tweaks))
        .route(
            "/api/servo_tweaks/saved",
            get(routes::get_saved_servo_tweaks),
        )
        // Body pose
        .route("/api/pose", post(routes::set_body_pose))
        // Text-to-speech
        .route("/api/tts", post(routes::speak_text))
    // LiDAR SLAM data
    .route("/api/lidar/frame", get(routes::get_lidar_frame))
    .route("/api/lidar/map", get(routes::get_lidar_map))
        // Add state and middleware
        .with_state(app_state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Hexapod API server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
