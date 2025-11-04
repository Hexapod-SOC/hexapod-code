use std::sync::Arc;
use tokio::sync::Mutex;
use devices::servo::ServoController;
use devices::picoubec::PicoUbecController;
use movement::controller::GaitController;

/// Shared application state for the web API
pub struct AppState {
    pub servo_controller: Arc<Mutex<ServoController>>,
    pub gait_controller: Arc<Mutex<GaitController>>,
    pub ubec_controller: Arc<Mutex<PicoUbecController>>,
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
        }
    }
}
