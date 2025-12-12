//FIXME eventually convert to config files not hardcoded constants
use devices::lidar::LidarSlamConfig;
use devices::servo::{ServoOffsets, ServoPins};
use lidar_slam::SlamParams;
use movement::ik;
use std::time::Duration;

pub const WEB_API_ENABLE: bool = true;
pub const API_PORT: u16 = 3000;
pub const WEB_PANEL_ENABLE: bool = true;
pub const WEB_PANEL_PORT: u16 = 8080;
pub const LIDAR_SLAM_ENABLE: bool = true;
pub const LIDAR_SERIAL_PORT: &str = "/dev/ttyUSB0";
pub const LIDAR_IDLE_SLEEP_MS: u64 = 5;

pub const TMP_DIR: &str = "/tmp/hexapod/";
pub const TTS_URL: &str = "http://127.0.0.1:5000";
//pub const VOICE_ID: &str = "en_us_001";

/// Default serial port for the UBEC/battery monitor.
/// You can override at runtime with the UBEC_PORT env var.
pub const UBEC_PORT: &str = "/dev/serial0";

pub const SERVO_PINS: ServoPins = ServoPins {
    left_front: (12, 13, 14),
    left_middle: (4, 5, 6),
    left_back: (0, 1, 2),
    right_front: (12, 13, 14),
    right_middle: (4, 5, 6),
    right_back: (0, 1, 2),
};

pub const CONSTRAINTS: ik::Constraints = ik::Constraints {
    coxa_length: 43.0,   // Length of the coxa segment in mm
    femur_length: 60.0,  // Length of the femur segment in mm
    tibia_length: 104.0, // Length of the tibia segment in mm

    coxa_soffset: 90.0,  // Offset to align coxa angle to 0 degrees forward
    femur_soffset: 83.0, // Offset to align femur angle to horizontal
    tibia_soffset: 35.0, // Offset to align tibia angle to straight down
};

//FIXME mirrored maybe reverse polarity?
pub const SERVO_OFFSETS: ServoOffsets = ServoOffsets {
    left_front: (-2.5, -4.5, 0.0),
    left_middle: (-5.0, -4.0, -1.5),
    left_back: (7.5, -10.0, -6.0),
    right_front: (2.5, 5.0, -3.0),
    right_middle: (2.5, -4.0, -5.0),
    right_back: (5.0, -1.5, -2.5), // Fixed: was 5.0, should be -2.5 based on B1 data
};

// Persistent calibration storage (JSON). Relative to the working directory.
// Example path: ./calibration/leg_stance.json
pub const CALIBRATION_LEG_STANCE_FILE: &str = "calibration/leg_stance.json";
/// Persistent per-servo angle tweaks (degrees), applied on top of IK outputs.
pub const CALIBRATION_SERVO_TWEAKS_FILE: &str = "calibration/servo_angle_tweaks.json";

/// Helper to build the configuration passed into the LiDAR SLAM thread.
pub fn lidar_slam_config() -> LidarSlamConfig {
    let mut params = SlamParams::default();
    params.map_size_pixels = 1024;
    params.map_resolution = 0.04;
    params.max_range_m = 12.0;
    params.sample_step = 2;

    LidarSlamConfig {
        port: LIDAR_SERIAL_PORT.into(),
        read_buffer_len: 8192,
        idle_sleep: Duration::from_millis(LIDAR_IDLE_SLEEP_MS),
        slam_params: params,
    }
}
