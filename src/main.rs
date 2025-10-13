#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod config;
pub mod macros;

// External crates
use glam::Vec3;

// Workspace imports
use config::{TTS_URL, TTS_TMP_DIR, CONSTRAINTS, SERVO_PINS}; //FIXME eventually convert to config files not hardcoded constants
use movement::{ik, legs::{LegAngles, Leg}};
use devices::servo::{ServoPins, ServoController};
use audio::tts;

#[tokio::main]
async fn main() {
    println!("Hello world from Hexapod EY!");

    tts::init(TTS_URL, TTS_TMP_DIR);
    tts::sayen("Hello, I am a hexapod robot!").unwrap();

    let ik = ik::SimpleIK::new(CONSTRAINTS);
    let servos_controller = ServoController::new(SERVO_PINS);
    let movement_controller = MoveTmpStruct::new(servos_controller, ik);

    println!("Demo finished. Press Ctrl+C to exit.");
    tokio::time::sleep(std::time::Duration::from_secs(9999)).await; // Keep the program running
}

//FIXME temp struct to hold movement logic until we add it to movement crate
pub struct MoveTmpStruct {
    servo_controller: ServoController,
    ik: ik::SimpleIK,
}

impl MoveTmpStruct {
    pub fn new(servo_controller: ServoController, ik: ik::SimpleIK) -> Self {
        MoveTmpStruct { servo_controller, ik }
    }
    pub fn move_leg_to_position(&mut self, leg: Leg, position: glam::Vec3) {
        let angles = self.ik.calculate_leg_angles(leg, position);
        self.servo_controller.set_leg_angles(leg, angles);
    }
}