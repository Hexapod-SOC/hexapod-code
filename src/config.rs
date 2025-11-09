//FIXME eventually convert to config files not hardcoded constants
use devices::servo::{ServoPins, ServoOffsets};
use movement::ik;

pub const WEB_API_ENABLE: bool = true;
pub const API_PORT: u16 = 3000;
pub const WEB_PANEL_ENABLE: bool = true;
pub const WEB_PANEL_PORT: u16 = 8080;

pub const TMP_DIR: &str = "/tmp/hexapod/";
pub const TTS_URL: &str = "http://127.0.0.1:5000";
//pub const VOICE_ID: &str = "en_us_001";

pub const SERVO_PINS: ServoPins = ServoPins {
    left_front: (12, 13, 14),
    left_middle: (4, 5, 6),
    left_back: (0, 1, 2),
    right_front: (12, 13, 14),
    right_middle: (4, 5, 6),
    right_back: (0, 1, 2),
};

pub const CONSTRAINTS: ik::Constraints = ik::Constraints {
    coxa_length:  43.0,  // Length of the coxa segment in mm
    femur_length: 60.0,  // Length of the femur segment in mm
    tibia_length: 104.0, // Length of the tibia segment in mm

    coxa_soffset:  90.0, // Offset to align coxa angle to 0 degrees forward
    femur_soffset: 83.0, // Offset to align femur angle to horizontal
    tibia_soffset: 35.0, // Offset to align tibia angle to straight down
};

pub const SERVO_OFFSETS: ServoOffsets = ServoOffsets {
    left_front: (-2.5, -4.5, 0.0),
    left_middle: (-5.0, -40.0, -1.5),
    left_back: (7.5, -10.0, -6.0),
    right_front: (2.5, 5.0, -3.0),
    right_middle: (2.5, -40.0, 5.0),
    right_back: (5.0, -1.5, -2.5),  // Fixed: was 5.0, should be -2.5 based on B1 data
};
