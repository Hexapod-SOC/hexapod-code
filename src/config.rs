//FIXME eventually convert to config files not hardcoded constants
use devices::servo::ServoPins;
use movement::ik;

pub const TMP_DIR: &str = "/tmp/hexapod/";
pub const TTS_URL: &str = "http://127.0.0.1:5000";

pub const SERVO_PINS: ServoPins = ServoPins {
    left_front: (0, 1, 2),
    left_middle: (4, 5, 6),
    left_back: (8, 9, 10),
    right_front: (0, 1, 2),
    right_middle: (4, 5, 6),
    right_back: (8, 9, 10),
};

pub const CONSTRAINTS: ik::Constraints = ik::Constraints {
    coxa_length:  43.0,  // Length of the coxa segment in mm
    femur_length: 60.0,  // Length of the femur segment in mm
    tibia_length: 104.0, // Length of the tibia segment in mm

    coxa_soffset:  90.0, // Offset to align coxa angle to 0 degrees forward
    femur_soffset: 83.0, // Offset to align femur angle to horizontal
    tibia_soffset: 35.0, // Offset to align tibia angle to straight down
};
