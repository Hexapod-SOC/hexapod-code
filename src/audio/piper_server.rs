// audio/server.rs
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use std::path::Path;

/// Spawns the Piper TTS HTTP server in a background thread
/// 
/// # Arguments
/// * `voice_model` - Optional voice model to use (e.g., "en_US-amy-medium.onnx")
///                   If None, uses TTS_VOICE_MODEL env var or default
/// 
/// # Returns
/// Returns a JoinHandle that can be used to wait for the server thread
pub fn spawn_voice_server(voice_model: Option<&str>) -> thread::JoinHandle<()> {
    let model = voice_model
        .map(|s| s.to_string())
        .or_else(|| std::env::var("TTS_VOICE_MODEL").ok())
        .unwrap_or_else(|| "en_US-amy-medium.onnx".to_string());

    thread::spawn(move || {
        println!("Starting Piper TTS server with voice model: {}", model);
        
        // Get paths from environment or use defaults
        let venv_path = std::env::var("PIPER_VENV_PATH")
            .unwrap_or_else(|_| "/hexapod/libs/pip/tts".to_string());
        let voices_path = std::env::var("PIPER_VOICES_PATH")
            .unwrap_or_else(|_| "/hexapod/libs/voices".to_string());
        
        let python_path = format!("{}/bin/python3", venv_path);
        
        // Check if paths exist
        if !Path::new(&python_path).exists() {
            eprintln!("Python not found at: {}", python_path);
            return;
        }
        
        if !Path::new(&voices_path).exists() {
            eprintln!("Voices directory not found at: {}", voices_path);
            return;
        }
        
        println!("Using Python: {}", python_path);
        println!("Using voices from: {}", voices_path);
        
        // Run the server directly
        let status = Command::new(&python_path)
            .arg("-m")
            .arg("piper.http_server")
            .arg("-m")
            .arg(&model)
            .current_dir(&voices_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();

        match status {
            Ok(exit_status) => {
                if exit_status.success() {
                    println!("Voice server exited successfully");
                } else {
                    eprintln!("Voice server exited with status: {}", exit_status);
                }
            }
            Err(e) => {
                eprintln!("Failed to start voice server: {}", e);
            }
        }
    })
}

/// Spawns the Piper TTS HTTP server as a detached background process
/// 
/// This version doesn't wait for the process to complete and returns immediately.
/// The server will continue running even after the parent process exits.
/// 
/// # Arguments
/// * `voice_model` - Optional voice model to use
/// 
/// # Returns
/// Returns Ok(()) if the process was spawned successfully, Err otherwise
pub fn spawn_voice_server_detached(voice_model: Option<&str>) -> Result<(), std::io::Error> {
    let model = voice_model
        .map(|s| s.to_string())
        .or_else(|| std::env::var("TTS_VOICE_MODEL").ok())
        .unwrap_or_else(|| "en_US-amy-medium.onnx".to_string());

    println!("Starting Piper TTS server (detached) with voice model: {}", model);
    
    // Get paths from environment or use defaults
    let venv_path = std::env::var("PIPER_VENV_PATH")
        .unwrap_or_else(|_| "/hexapod/libs/pip/tts".to_string());
    let voices_path = std::env::var("PIPER_VOICES_PATH")
        .unwrap_or_else(|_| "/hexapod/libs/voices".to_string());
    
    let python_path = format!("{}/bin/python3", venv_path);
    
    // Check if python exists
    if !Path::new(&python_path).exists() {
        eprintln!("Python not found at: {}", python_path);
        eprintln!("Set PIPER_VENV_PATH environment variable to the correct path");
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Python not found at {}", python_path)
        ));
    }
    
    // Check if voices directory exists
    if !Path::new(&voices_path).exists() {
        eprintln!("Voices directory not found at: {}", voices_path);
        eprintln!("Set PIPER_VOICES_PATH environment variable to the correct path");
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Voices directory not found at {}", voices_path)
        ));
    }
    
    println!("Using Python: {}", python_path);
    println!("Using voices from: {}", voices_path);
    
    // Spawn the server directly without shell
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/piper_tts.log")?;
    
    let log_file_err = log_file.try_clone()?;
    
    let _child = Command::new(&python_path)
        .arg("-m")
        .arg("piper.http_server")
        .arg("-m")
        .arg(&model)
        .current_dir(&voices_path)
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_file_err))
        .stdin(Stdio::null())
        .spawn()?;
    
    println!("Server process spawned. Waiting for startup...");
    
    // Give the server a moment to start
    thread::sleep(Duration::from_millis(1500));
    
    println!("Server spawn command executed. Check /tmp/piper_tts.log for details.");
    
    Ok(())
}

/// Checks if the TTS server is running by attempting to connect
pub fn is_server_running(server_url: &str) -> bool {
    use reqwest::blocking::Client;
    
    let client = Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    
    client.get(server_url).send().is_ok()
}

/// Starts the voice server if it's not already running
/// 
/// # Arguments
/// * `server_url` - URL to check if server is running (e.g., "http://127.0.0.1:5000")
/// * `voice_model` - Optional voice model to use
/// 
/// # Returns
/// Returns Ok(true) if server was started, Ok(false) if already running, Err on failure
pub fn ensure_voice_server_running(
    server_url: &str,
    voice_model: Option<&str>,
) -> Result<bool, std::io::Error> {
    if is_server_running(server_url) {
        println!("Voice server is already running at {}", server_url);
        Ok(false)
    } else {
        println!("Voice server not detected, starting...");
        spawn_voice_server_detached(voice_model)?;
        
        // Wait up to 5 seconds for server to start
        for i in 1..=20 {
            thread::sleep(Duration::from_millis(500));
            if is_server_running(server_url) {
            println!("Voice server started successfully!");
            return Ok(true);
            }
            if i % 2 == 0 {
            println!("Waiting for server to start... ({}/10s)", i / 2);
            }
        }
        
        eprintln!("Warning: Server may not have started properly");
        Ok(true)
    }
}
