#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod config;
pub mod macros;

// External crates
use glam::Vec3;

// Workspace imports
use config::{TTS_URL, TMP_DIR, CONSTRAINTS, SERVO_PINS}; //FIXME eventually convert to config files not hardcoded constants
use movement::{ik, legs::{LegAngles, Leg}, controller::{GaitController, BodyPose}, gaits::GAITS};
use devices::servo::{ServoPins, ServoController};
use audio::tts;

#[tokio::main]
async fn main() {
    println!("Hello world from Hexapod EY!");

    tts::init(TTS_URL, TMP_DIR);
    tts::cleanup_cache(7).unwrap();
    
    tts::sayen("Starting...").unwrap();

    let ik = ik::SimpleIK::new(CONSTRAINTS);
    let mut servos_controller = ServoController::new(SERVO_PINS);
    //let mut move_tmp = MoveTmpStruct::new(servos_controller, ik);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;



/*     move_tmp.move_leg_to_pos(Leg::RightMiddle, Vec3 { x: 0.0, y: 90.0, z: -90.0 });
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    move_tmp.move_leg_to_pos(Leg::RightMiddle, Vec3 { x: 0.0, y: 90.0, z: -60.0 });
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    move_tmp.move_leg_to_pos(Leg::RightMiddle, Vec3 { x: 0.0, y: 90.0, z: -30.0 });
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    move_tmp.move_leg_to_pos(Leg::RightMiddle, Vec3 { x: 0.0, y: 90.0, z: -1.0 });
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    move_tmp.move_leg_to_pos(Leg::RightMiddle, Vec3 { x: 0.0, y: 90.0, z: 30.0 });
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    move_tmp.move_leg_to_pos(Leg::RightMiddle, Vec3 { x: 0.0, y: 90.0, z: 60.0 });
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
 */
    // Create gait controller with tripod gait
    let mut gait_controller = GaitController::new(&GAITS[0], ik); // GAIT_TRI

    println!("\n=== Hexapod Movement Demo ===\n");

    // Demo 1: Static body tilt
    //demo_body_tilt(&mut gait_controller, &mut servos_controller).await;
    //tts::sayen("This is a really long sentence to test if its blocking the main thread").unwrap();

    // Demo 2: Tripod walking forward
    demo_tripod_walk(&mut gait_controller, &mut servos_controller).await;
    
    // Demo 3: Walking with body tilt
    //demo_walk_with_tilt(&mut gait_controller, &mut servos_controller).await;

    // Demo 4: Rotation in place
    //demo_rotation(&mut gait_controller, &mut servos_controller).await;

    println!("\nDemo finished. Press Ctrl+C to exit.");
    tokio::time::sleep(std::time::Duration::from_secs(9999)).await; // Keep the program running
}

/// Demo: Body tilt/rotation without walking
async fn demo_body_tilt(
    gait_controller: &mut GaitController, 
    servos: &mut ServoController
) {
    println!("Demo 1: Body Tilt (Roll/Pitch/Yaw)");
    //tts::sayen("Demonstrating body orientation control").unwrap();
    
    let poses = vec![
        BodyPose::with_rotation(30.0, 0.0, 0.0),   // Roll right
        BodyPose::with_rotation(-30.0, 0.0, 0.0),  // Roll left
        BodyPose::with_rotation(0.0, 30.0, 0.0),   // Pitch forward
        BodyPose::with_rotation(0.0, -30.0, 0.0),  // Pitch back
        BodyPose::with_rotation(0.0, 0.0, 40.0),   // Yaw right
        BodyPose::with_rotation(0.0, 0.0, -40.0),  // Yaw left
        BodyPose::default(),                        // Return to neutral
    ];

    for pose in poses {
        gait_controller.set_body_pose(pose);
        let angles = gait_controller.calculate_pose_angles();
        
        for (leg, leg_angles) in angles.iter() {
            servos.set_leg_angles(*leg, *leg_angles);
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }
    
    println!("Body tilt demo complete\n");
}

/// Demo: Tripod gait walking forward
async fn demo_tripod_walk(
    gait_controller: &mut GaitController,
    servos: &mut ServoController
) {
    println!("Demo 2: Tripod Walking Forward");
    //tts::sayen("Walking forward with tripod gait").unwrap();
    
    let velocity = Vec3::new(70.0, 0.0, 0.0); // X=forward, Y=left/right, Z=up/down
    let duration = 30.0; // seconds
    let dt = 0.025;
    let steps = (duration / dt) as i32;

    for _ in 0..steps {
        gait_controller.update(dt);
        let angles = gait_controller.calculate_walking_angles(velocity, 0.0);
        
        for (leg, leg_angles) in angles.iter() {
            servos.set_leg_angles(*leg, *leg_angles);
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis((dt * 1000.0) as u64)).await;
    }
    
    println!("Walking demo complete\n");
}

/// Demo: Walking while tilting body
async fn demo_walk_with_tilt(
    gait_controller: &mut GaitController,
    servos: &mut ServoController
) {
    println!("Demo 3: Walking with Body Tilt");
    tts::sayen("Walking with body tilt").unwrap();
    
    let velocity = Vec3::new(30.0, 0.0, 0.0);
    let duration = 5.0;
    let dt = 0.05;
    let steps = (duration / dt) as i32;

    for i in 0..steps {
        let t = (i as f32) / (steps as f32);
        
        // Oscillating roll during walk
        let roll = 10.0 * (t * 6.28).sin(); // 1 cycle
        gait_controller.set_body_pose(BodyPose::with_rotation(roll, 0.0, 0.0));
        
        gait_controller.update(dt);
        let angles = gait_controller.calculate_walking_with_pose_angles(velocity, 0.0);
        
        for (leg, leg_angles) in angles.iter() {
            servos.set_leg_angles(*leg, *leg_angles);
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis((dt * 1000.0) as u64)).await;
    }
    
    // Return to neutral
    gait_controller.set_body_pose(BodyPose::default());
    
    println!("Walk with tilt demo complete\n");
}

/// Demo: Rotate in place
async fn demo_rotation(
    gait_controller: &mut GaitController,
    servos: &mut ServoController
) {
    println!("Demo 4: Rotation in Place");
    tts::sayen("Rotating in place").unwrap();
    
    let rotation_speed = 0.5; // Rotation rate
    let duration = 4.0;
    let dt = 0.05;
    let steps = (duration / dt) as i32;

    for _ in 0..steps {
        gait_controller.update(dt);
        let angles = gait_controller.calculate_walking_angles(Vec3::ZERO, rotation_speed);
        
        for (leg, leg_angles) in angles.iter() {
            servos.set_leg_angles(*leg, *leg_angles);
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis((dt * 1000.0) as u64)).await;
    }
    
    println!("Rotation demo complete\n");
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
    pub fn move_leg_to_pos(&mut self, leg: Leg, position: glam::Vec3) {
        let angles = self.ik.calc_pos_leg_angles(leg, position);
        self.servo_controller.set_leg_angles(leg, angles);
    }



    pub fn move_legs_to_ang(&mut self, angles: LegAngles) {
        self.servo_controller.set_all_legs_to_angles(angles.coxa, angles.femur, angles.tibia);
    }
}