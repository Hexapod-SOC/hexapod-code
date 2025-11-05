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
    
    // Spawn background task for continuous movement updates
    let movement_state = app_state.clone();
    tokio::spawn(async move {
        movement_update_loop(movement_state).await;
    });

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
        
        // Body pose
        .route("/api/pose", post(routes::set_body_pose))
        
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

/// Background task that continuously updates gait and applies movement
async fn movement_update_loop(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(50)); // 20 Hz update rate
    
    loop {
        interval.tick().await;
        
        // Get current movement velocity
        let movement = {
            let mov = state.movement_velocity.lock().await;
            *mov
        };
        
        // Update gait
        {
            let mut gait = state.gait_controller.lock().await;
            gait.update(0.05); // 50ms timestep
        }
        
        // Calculate and apply movement if any velocity is set
        let is_moving = movement.velocity.length() > 0.01 || movement.rotation.abs() > 0.001;
        
        if is_moving {
            // Calculate walking angles
            let angles = {
                let gait = state.gait_controller.lock().await;
                gait.calculate_walking_angles(movement.velocity, movement.rotation)
            };
            
            // Apply to servos
            let mut servo = state.servo_controller.lock().await;
            for (leg, leg_angles) in angles.iter() {
                servo.set_leg_angles(*leg, *leg_angles);
            }
        }
    }
}
