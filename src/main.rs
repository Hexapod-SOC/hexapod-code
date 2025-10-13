#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod macros;
//pub mod audio;
pub mod movementA;

// External crates
use glam::Vec3;

// Workspace imports
use movement::legs::{LegAngles, Leg};
use devices::servo::{ServoPins, ServoController};


const SERVO_PINS: ServoPins = ServoPins {
    left_front: (0, 1, 2),
    left_middle: (4, 5, 6),
    left_back: (8, 9, 10),
    right_front: (0, 1, 2),
    right_middle: (4, 5, 6),
    right_back: (8, 9, 10),
};


#[tokio::main]
async fn main() {
    println!("Hello, world from Hexapod EY!");

    let ik = movementA::ik::SimpleIK::new();
    let mut servos_controller = ServoController::new(SERVO_PINS);
    //servos_controller.set_all_legs_to_angles(90.0, 50.0, 50.0);

    let mut movement_controller = movementA::movement::Movement::new(servos_controller, ik);

    
    println!("Demo finished. Press Ctrl+C to exit.");
    tokio::time::sleep(std::time::Duration::from_secs(9999)).await;
}