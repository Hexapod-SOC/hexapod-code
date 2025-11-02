use std::process::{Command, Stdio};
use std::path::Path;
use std::io::{self, Write};
use std::thread;

/// Plays a WAV file using `aplay`.
pub fn play_wav(file_path: &str) -> io::Result<()> {
    let path = Path::new(file_path);
    if !path.exists() {
        eprintln!("WAV file does not exist: {}", file_path);
        return Ok(());
    }

    println!("Executing command: sudo aplay {}", file_path);
    let status = Command::new("sudo")
        .arg("aplay")
        .arg(file_path)
        .status()?;

    if status.success() {
        println!("WAV playback finished successfully.");
    } else {
        eprintln!("WAV playback failed with status: {:?}", status);
    }

    Ok(())
}

/// Plays a WAV file from memory using `aplay`.
pub fn play_wav_bytes(wav_data: &[u8]) -> io::Result<()> {
    // Start aplay with piped stdin
    let mut child = Command::new("sudo")
        .arg("aplay")
        .arg("-f")
        .arg("cd") // default CD quality; change if your WAV format is different
        .stdin(Stdio::piped())
        .spawn()?;

    // Write the WAV bytes to aplay's stdin
    child.stdin.as_mut().unwrap().write_all(wav_data)?;

    // Wait for playback to finish
    let status = child.wait()?;
    if status.success() {
        println!("WAV playback finished successfully.");
    } else {
        eprintln!("WAV playback failed with status: {:?}", status);
    }

    Ok(())
}

/// Starts playing a WAV file using `aplay` and returns immediately (non-blocking).
pub fn play_wav_detached(file_path: &str) -> io::Result<()> {
    let path = Path::new(file_path);
    if !path.exists() {
        eprintln!("WAV file does not exist: {}", file_path);
        return Ok(());
    }

    println!("(detached) Executing command: sudo aplay {}", file_path);
    let _child = Command::new("sudo")
        .arg("aplay")
        .arg(file_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    // Do not wait; child continues playing in background
    Ok(())
}

/// Starts playing WAV bytes using `aplay` and returns immediately. Data is piped on a background thread.
pub fn play_wav_bytes_detached(wav_data: &[u8]) -> io::Result<()> {
    // Start aplay with piped stdin
    let mut child = Command::new("sudo")
        .arg("aplay")
        .arg("-f")
        .arg("cd")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    // Take stdin
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Failed to open stdin for aplay"))?;

    // Copy data so the caller can return immediately
    let data = wav_data.to_vec();

    // Move the child handle so we can wait after writing without blocking the caller
    thread::spawn(move || {
        if let Err(e) = stdin.write_all(&data) {
            eprintln!("Failed to write WAV bytes to aplay stdin: {}", e);
        }
        // Explicitly drop stdin to signal EOF
        drop(stdin);
        if let Err(e) = child.wait() {
            eprintln!("aplay process wait() failed: {}", e);
        }
    });

    Ok(())
}
