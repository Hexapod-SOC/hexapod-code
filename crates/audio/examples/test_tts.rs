use audio::tts;

fn main() {
    // Initialize TTS with your server URL and temp directory
    // Adjust these paths as needed for your setup
    let tts_url = "http://localhost:5000";  // Update with your TTS server URL
    let tmp_dir = "/tmp/hexapod";  // Update with your desired temp directory
    
    tts::init(tts_url, tmp_dir);
    
    println!("Testing TTS with 'Hello World'...");
    
    // Test with English voice
    match tts::sayen("Hello World") {
        Ok(_) => println!("Successfully played 'Hello World'"),
        Err(e) => eprintln!("Error: {}", e),
    }
    
    println!("\nTesting again to demonstrate caching...");
    
    // Say it again - should use cache on second run
    match tts::sayen("Hello World") {
        Ok(_) => println!("Successfully played 'Hello World' (2nd time)"),
        Err(e) => eprintln!("Error: {}", e),
    }
    
    println!("\nTesting with Slovak voice...");
    
    // Test with Slovak voice
    match tts::saysk("Ahoj svet") {
        Ok(_) => println!("Successfully played 'Ahoj svet'"),
        Err(e) => eprintln!("Error: {}", e),
    }
    
    println!("\nTest complete!");
}
