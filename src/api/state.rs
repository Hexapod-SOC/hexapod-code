use std::sync::Arc;
use tokio::sync::Mutex;
use devices::servo::ServoController;
use devices::picoubec::PicoUbecController;
use movement::controller::GaitController;
use glam::Vec3;

/// Current movement velocity
#[derive(Clone, Copy, Debug)]
pub struct MovementVelocity {
    pub velocity: Vec3,  // X=forward, Y=unused, Z=strafe
    pub rotation: f32,   // Yaw rotation rate
}

impl Default for MovementVelocity {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            rotation: 0.0,
        }
    }
}

/// Shared application state for the API
pub struct AppState {
    pub servo_controller: Arc<Mutex<ServoController>>,
    pub gait_controller: Arc<Mutex<GaitController>>,
    pub ubec_controller: Arc<Mutex<PicoUbecController>>,
    pub movement_velocity: Arc<Mutex<MovementVelocity>>,
}

impl AppState {
    /// Create AppState from existing shared controllers
    /// This avoids creating duplicate I2C device connections
    pub fn from_shared(
        servo_controller: Arc<Mutex<ServoController>>,
        gait_controller: Arc<Mutex<GaitController>>,
        ubec_controller: Arc<Mutex<PicoUbecController>>,
    ) -> Self {
        Self {
            servo_controller,
            gait_controller,
            ubec_controller,
            movement_velocity: Arc::new(Mutex::new(MovementVelocity::default())),
        }
    }
}
