use crate::hexapod::HexapodControl;
use crate::hexapod::ServoAngleTweaks;
use devices::lidar::LidarSlamHandle;
use devices::picoubec::PicoUbecController;
use devices::imu::Imu;
use crate::hexapod::GaitController;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shared application state for the API
///
/// This only contains references to the control state and read-only controllers.
/// All movement calculations happen in hexapod.update(), not in the API handlers.
pub struct AppState {
    pub control: Arc<Mutex<HexapodControl>>,
    pub gait_controller: Arc<Mutex<GaitController>>,
    pub ubec_controller: Arc<Mutex<PicoUbecController>>,
    pub servo_angle_tweaks: Arc<Mutex<ServoAngleTweaks>>,
    pub lidar: Option<Arc<LidarSlamHandle>>,
    pub imu: Option<Arc<Mutex<Box<dyn Imu>>>>,
}

impl AppState {
    pub fn from_hexapod(
        control: Arc<Mutex<HexapodControl>>,
        gait_controller: Arc<Mutex<GaitController>>,
        ubec_controller: Arc<Mutex<PicoUbecController>>,
        servo_angle_tweaks: Arc<Mutex<ServoAngleTweaks>>,
        lidar: Option<Arc<LidarSlamHandle>>,
        imu: Option<Arc<Mutex<Box<dyn Imu>>>>,
    ) -> Self {
        Self {
            control,
            gait_controller,
            ubec_controller,
            servo_angle_tweaks,
            lidar,
            imu,
        }
    }
}
