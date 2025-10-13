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
    let servos_controller = ServoController::new(SERVO_PINS);

    let mut movement_controller = movementA::movement::Movement::new(servos_controller, ik);

    println!("Starting tripod gait walking demo...");
    
    // Walking parameters (X: front/back, Y: left/right, Z: up/down)
    let step_forward = 30.0;      // How far forward each step (X axis)
    let step_back = -30.0;        // How far back each step (X axis)
    let lateral_offset = 50.0;     // Side offset (Y axis)
    let step_height = -40.0;      // How high to lift legs (Z axis)
    let ground_height = -80.0;    // Ground level (Z axis)
    let step_duration_ms = 300;   // Duration of each phase in milliseconds
    
    // Number of walking cycles
    let num_cycles = 15;
    
    for cycle in 0..num_cycles {
        println!("Walking cycle {}/{}", cycle + 1, num_cycles);
        
        // Phase 1: Lift tripod 1 (RightFront, LeftMiddle, RightBack) UP
        //          Tripod 2 is on ground at step_back
        movement_controller.move_leg_to_position(Leg::RightFront, Vec3::new(step_back, lateral_offset, step_height));
        movement_controller.move_leg_to_position(Leg::LeftMiddle, Vec3::new(step_back, lateral_offset, step_height));
        movement_controller.move_leg_to_position(Leg::RightBack, Vec3::new(step_back, lateral_offset, step_height));
        tokio::time::sleep(std::time::Duration::from_millis(step_duration_ms)).await;

        // Phase 2: Move tripod 1 FORWARD while in air
        //          Tripod 2 still on ground at step_back
        movement_controller.move_leg_to_position(Leg::RightFront, Vec3::new(step_forward, lateral_offset, step_height));
        movement_controller.move_leg_to_position(Leg::LeftMiddle, Vec3::new(step_forward, lateral_offset, step_height));
        movement_controller.move_leg_to_position(Leg::RightBack, Vec3::new(step_forward, lateral_offset, step_height));
        tokio::time::sleep(std::time::Duration::from_millis(step_duration_ms)).await;
        
        // Phase 3: Lower tripod 1 DOWN to ground at step_forward
        //          Tripod 2 still on ground at step_back
        movement_controller.move_leg_to_position(Leg::RightFront, Vec3::new(step_forward, lateral_offset, ground_height));
        movement_controller.move_leg_to_position(Leg::LeftMiddle, Vec3::new(step_forward, lateral_offset, ground_height));
        movement_controller.move_leg_to_position(Leg::RightBack, Vec3::new(step_forward, lateral_offset, ground_height));
        tokio::time::sleep(std::time::Duration::from_millis(step_duration_ms)).await;
        
        // Phase 4: Both tripods on ground - push body forward
        //          Tripod 1 moves from step_forward to step_back (pushes body)
        //          Tripod 2 moves from step_back to step_forward (pushed by body)
        movement_controller.move_leg_to_position(Leg::RightFront, Vec3::new(step_back, lateral_offset, ground_height));
        movement_controller.move_leg_to_position(Leg::LeftMiddle, Vec3::new(step_back, lateral_offset, ground_height));
        movement_controller.move_leg_to_position(Leg::RightBack, Vec3::new(step_back, lateral_offset, ground_height));
        movement_controller.move_leg_to_position(Leg::LeftFront, Vec3::new(step_forward, lateral_offset, ground_height));
        movement_controller.move_leg_to_position(Leg::RightMiddle, Vec3::new(step_forward, lateral_offset, ground_height));
        movement_controller.move_leg_to_position(Leg::LeftBack, Vec3::new(step_forward, lateral_offset, ground_height));
        tokio::time::sleep(std::time::Duration::from_millis(step_duration_ms)).await;
        
        // Phase 5: Lift tripod 2 (LeftFront, RightMiddle, LeftBack) UP
        //          Tripod 1 is on ground at step_back
        movement_controller.move_leg_to_position(Leg::LeftFront, Vec3::new(step_forward, lateral_offset, step_height));
        movement_controller.move_leg_to_position(Leg::RightMiddle, Vec3::new(step_forward, lateral_offset, step_height));
        movement_controller.move_leg_to_position(Leg::LeftBack, Vec3::new(step_forward, lateral_offset, step_height));
        tokio::time::sleep(std::time::Duration::from_millis(step_duration_ms)).await;
        
        // Phase 6: Move tripod 2 BACK while in air (retracting for next step)
        //          Tripod 1 still on ground at step_back
        movement_controller.move_leg_to_position(Leg::LeftFront, Vec3::new(step_back, lateral_offset, step_height));
        movement_controller.move_leg_to_position(Leg::RightMiddle, Vec3::new(step_back, lateral_offset, step_height));
        movement_controller.move_leg_to_position(Leg::LeftBack, Vec3::new(step_back, lateral_offset, step_height));
        tokio::time::sleep(std::time::Duration::from_millis(step_duration_ms)).await;
        
        // Phase 7: Lower tripod 2 DOWN to ground at step_back
        //          Tripod 1 still on ground at step_back
        movement_controller.move_leg_to_position(Leg::LeftFront, Vec3::new(step_back, lateral_offset, ground_height));
        movement_controller.move_leg_to_position(Leg::RightMiddle, Vec3::new(step_back, lateral_offset, ground_height));
        movement_controller.move_leg_to_position(Leg::LeftBack, Vec3::new(step_back, lateral_offset, ground_height));
        tokio::time::sleep(std::time::Duration::from_millis(step_duration_ms)).await;
        
        // Phase 8: Ready for next cycle - both tripods at step_back
        // (No movement needed, both already at step_back)
    }
    
    println!("Walking demo complete! Returning to neutral stance...");
    
    // Return all legs to neutral position
    let neutral_pos = Vec3::new(0.0, lateral_offset, ground_height);
    movement_controller.move_leg_to_position(Leg::LeftFront, neutral_pos);
    movement_controller.move_leg_to_position(Leg::LeftMiddle, neutral_pos);
    movement_controller.move_leg_to_position(Leg::LeftBack, neutral_pos);
    movement_controller.move_leg_to_position(Leg::RightFront, neutral_pos);
    movement_controller.move_leg_to_position(Leg::RightMiddle, neutral_pos);
    movement_controller.move_leg_to_position(Leg::RightBack, neutral_pos);
    
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    
    println!("Demo finished. Press Ctrl+C to exit.");
    tokio::time::sleep(std::time::Duration::from_secs(9999)).await;
}