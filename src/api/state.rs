use std::sync::Arc;
use tokio::sync::Mutex;
use devices::picoubec::PicoUbecController;
use devices::servo::ServoController;
use movement::controller::GaitController;
use crate::hexapod::HexapodControl;

/// Shared application state for the API
/// 
/// This only contains references to the control state and read-only controllers.
/// All movement calculations happen in hexapod.update(), not in the API handlers.
pub struct AppState {
    pub control: Arc<Mutex<HexapodControl>>,
    pub gait_controller: Arc<Mutex<GaitController>>,
    pub ubec_controller: Arc<Mutex<PicoUbecController>>,
    pub servo_controller: Arc<Mutex<ServoController>>,
}

impl AppState {
    pub fn from_hexapod(
        control: Arc<Mutex<HexapodControl>>,
        gait_controller: Arc<Mutex<GaitController>>,
        ubec_controller: Arc<Mutex<PicoUbecController>>,
        servo_controller: Arc<Mutex<ServoController>>,
    ) -> Self {
        Self {
            control,
            gait_controller,
            ubec_controller,
            servo_controller,
        }
    }
}
