/// Demo routines for hexapod movement
/// 
/// This module contains various demonstration routines showcasing
/// different capabilities of the hexapod robot.

use crate::hexapod::Hexapod;
use glam::Vec3;
use movement::controller::BodyPose;

/// Demo: Body tilt/rotation without walking
/// 
/// Demonstrates body orientation control by tilting and rotating
/// the body through various poses while keeping legs stationary.
pub async fn demo_body_tilt(hexapod: &mut Hexapod) {
    println!("=== Demo: Body Tilt (Roll/Pitch/Yaw) ===");
    
    let poses = vec![
        ("Roll right", BodyPose::with_rotation(30.0, 0.0, 0.0)),
        ("Roll left", BodyPose::with_rotation(-30.0, 0.0, 0.0)),
        ("Pitch forward", BodyPose::with_rotation(0.0, 30.0, 0.0)),
        ("Pitch back", BodyPose::with_rotation(0.0, -30.0, 0.0)),
        ("Yaw right", BodyPose::with_rotation(0.0, 0.0, 40.0)),
        ("Yaw left", BodyPose::with_rotation(0.0, 0.0, -40.0)),
        ("Return to neutral", BodyPose::default()),
    ];

    for (description, pose) in poses {
        println!("  - {}", description);
        hexapod.set_body_pose(pose).await;
        hexapod.apply_static_pose().await;
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
    }
    
    println!("Body tilt demo complete\n");
}

/// Demo: Tripod gait walking forward
/// 
/// Demonstrates basic walking using the tripod gait pattern.
pub async fn demo_tripod_walk(hexapod: &mut Hexapod, duration_secs: f32) {
    println!("=== Demo: Walking Forward (Tripod Gait) ===");
    
    let velocity = Vec3::new(70.0, 0.0, 0.0); // Forward motion
    let dt = 0.025; // 40Hz update rate
    let steps = (duration_secs / dt) as i32;

    println!("  - Walking for {:.1} seconds", duration_secs);
    
    for _ in 0..steps {
        hexapod.update(dt).await;
        hexapod.walk(velocity, 0.0).await;
        tokio::time::sleep(tokio::time::Duration::from_millis((dt * 1000.0) as u64)).await;
    }
    
    println!("Walking demo complete\n");
}

/// Demo: Walking while tilting body
/// 
/// Demonstrates combining walking motion with body orientation control.
pub async fn demo_walk_with_tilt(hexapod: &mut Hexapod, duration_secs: f32) {
    println!("=== Demo: Walking with Body Tilt ===");
    
    let velocity = Vec3::new(30.0, 0.0, 0.0);
    let dt = 0.05;
    let steps = (duration_secs / dt) as i32;

    println!("  - Walking with oscillating roll for {:.1} seconds", duration_secs);
    
    for i in 0..steps {
        let t = (i as f32) / (steps as f32);
        
        // Oscillating roll during walk
        let roll = 10.0 * (t * 6.28).sin(); // 1 cycle
        hexapod.set_body_pose(BodyPose::with_rotation(roll, 0.0, 0.0)).await;
        
        hexapod.update(dt).await;
        hexapod.walk_with_pose(velocity, 0.0).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis((dt * 1000.0) as u64)).await;
    }
    
    // Return to neutral
    hexapod.set_body_pose(BodyPose::default()).await;
    
    println!("Walk with tilt demo complete\n");
}

/// Demo: Rotate in place
/// 
/// Demonstrates rotating the hexapod in place without forward/backward motion.
pub async fn demo_rotation(hexapod: &mut Hexapod, duration_secs: f32) {
    println!("=== Demo: Rotation in Place ===");
    
    let rotation_speed = 0.5; // Rotation rate
    let dt = 0.05;
    let steps = (duration_secs / dt) as i32;

    println!("  - Rotating for {:.1} seconds", duration_secs);
    
    for _ in 0..steps {
        hexapod.update(dt).await;
        hexapod.walk(Vec3::ZERO, rotation_speed).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis((dt * 1000.0) as u64)).await;
    }
    
    println!("Rotation demo complete\n");
}

/// Demo: Strafe (sideways) movement
/// 
/// Demonstrates lateral movement without changing body orientation.
pub async fn demo_strafe(hexapod: &mut Hexapod, duration_secs: f32) {
    println!("=== Demo: Strafing (Sideways Movement) ===");
    
    let dt = 0.025;
    let steps = (duration_secs / dt) as i32;

    println!("  - Strafing right for {:.1} seconds", duration_secs / 2.0);
    for _ in 0..(steps / 2) {
        hexapod.update(dt).await;
        hexapod.walk(Vec3::new(0.0, 0.0, 40.0), 0.0).await; // Strafe right
        tokio::time::sleep(tokio::time::Duration::from_millis((dt * 1000.0) as u64)).await;
    }

    println!("  - Strafing left for {:.1} seconds", duration_secs / 2.0);
    for _ in 0..(steps / 2) {
        hexapod.update(dt).await;
        hexapod.walk(Vec3::new(0.0, 0.0, -40.0), 0.0).await; // Strafe left
        tokio::time::sleep(tokio::time::Duration::from_millis((dt * 1000.0) as u64)).await;
    }
    
    println!("Strafe demo complete\n");
}

/// Demo: Combined movements
/// 
/// Demonstrates combining forward motion with rotation (curved path).
pub async fn demo_combined_movement(hexapod: &mut Hexapod, duration_secs: f32) {
    println!("=== Demo: Combined Forward + Rotation ===");
    
    let velocity = Vec3::new(50.0, 0.0, 0.0);
    let rotation = 0.3;
    let dt = 0.025;
    let steps = (duration_secs / dt) as i32;

    println!("  - Moving in a curve for {:.1} seconds", duration_secs);
    
    for _ in 0..steps {
        hexapod.update(dt).await;
        hexapod.walk(velocity, rotation).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis((dt * 1000.0) as u64)).await;
    }
    
    println!("Combined movement demo complete\n");
}

/// Run all demos in sequence
pub async fn run_all_demos(hexapod: &mut Hexapod) {
    println!("\n╔════════════════════════════════════════╗");
    println!("║   HEXAPOD MOVEMENT DEMONSTRATION      ║");
    println!("╚════════════════════════════════════════╝\n");

    // Reset to default stance before starting
    hexapod.reset_to_default_stance().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Run demos
    demo_body_tilt(hexapod).await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    demo_tripod_walk(hexapod, 5.0).await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    demo_rotation(hexapod, 4.0).await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    demo_strafe(hexapod, 4.0).await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    demo_walk_with_tilt(hexapod, 5.0).await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    demo_combined_movement(hexapod, 5.0).await;

    // Return to default stance
    hexapod.reset_to_default_stance().await;
    
    println!("\n╔════════════════════════════════════════╗");
    println!("║   ALL DEMOS COMPLETED                 ║");
    println!("╚════════════════════════════════════════╝\n");
}
