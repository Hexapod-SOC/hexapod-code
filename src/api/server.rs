use axum::{
    routing::{get, post},
    Router,
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
        
        // Movement control
        .route("/api/move", post(routes::move_hexapod))
        .route("/api/stop", post(routes::stop_hexapod))
        
        // Gait control
        .route("/api/gait", get(routes::get_gait))
        .route("/api/gait", post(routes::set_gait))
        // Custom gait tuning
        .route("/api/custom_gait", post(routes::set_custom_gait))
        
        // Leg calibration
        .route("/api/leg_stance", get(routes::get_leg_stance))
        .route("/api/leg_stance", post(routes::set_leg_stance))
        
        // Body pose
        .route("/api/pose", post(routes::set_body_pose))
        
        // Text-to-speech
        .route("/api/tts", post(routes::speak_text))
        
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
