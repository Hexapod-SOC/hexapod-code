#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod config;
pub mod hexapod;
pub mod demos;

// Workspace imports
use config::{TTS_URL, TMP_DIR, CONSTRAINTS, SERVO_PINS};
use movement::gaits::GAITS;
use audio::tts;

#[tokio::main]
async fn main() {
    println!("╔════════════════════════════════════════╗");
    println!("║   HEXAPOD ROBOT - EY                  ║");
    println!("╚════════════════════════════════════════╝\n");

    // Initialize text-to-speech
    println!("Initializing TTS...");
    tts::init(TTS_URL, TMP_DIR);
    tts::cleanup_cache(7).unwrap();
    tts::sayen("Hexapod initializing...").unwrap();

    // Create hexapod controller with tripod gait
    println!("Creating hexapod controller...");
    let mut hexapod = hexapod::Hexapod::new(
        SERVO_PINS,
        CONSTRAINTS,
        &GAITS[0], // Tripod gait
        None, // Use default stance
    );

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    println!("Hexapod ready!\n");

    // Run demonstration sequence
    tts::sayen("Starting demonstration sequence").unwrap();
    
    // Choose which demos to run:
    // Option 1: Run all demos
    //demos::run_all_demos(&mut hexapod).await;
    
    // Option 2: Run specific demos
    //demos::demo_body_tilt(&mut hexapod).await;
    demos::demo_tripod_walk(&mut hexapod, 10.0).await;
    //demos::demo_rotation(&mut hexapod, 4.0).await;
    //demos::demo_strafe(&mut hexapod, 4.0).await;
    //demos::demo_walk_with_tilt(&mut hexapod, 5.0).await;
    //demos::demo_combined_movement(&mut hexapod, 5.0).await;

    // Return to default stance
    hexapod.reset_to_default_stance();

    println!("\n╔════════════════════════════════════════╗");
    println!("║   DEMO COMPLETE                       ║");
    println!("╚════════════════════════════════════════╝");
    
    tts::sayen("Demo complete. Standing by.").unwrap();
    
    println!("\nPress Ctrl+C to exit.");
    tokio::time::sleep(std::time::Duration::from_secs(9999)).await;
}