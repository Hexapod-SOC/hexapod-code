use std::process::{Command, Stdio};
use std::path::Path;
use std::io;

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

/// Plays an MP3 file by converting it to WAV on the fly and calling `play_wav`.
pub fn play_mp3(file_path: &str) -> io::Result<()> {
    let path = Path::new(file_path);
    if !path.exists() {
        eprintln!("MP3 file does not exist: {}", file_path);
        return Ok(());
    }

    // Convert MP3 to WAV on the fly using ffmpeg
    let ffmpeg = Command::new("ffmpeg")
        .args([
            "-i", file_path,
            "-f", "wav",
            "pipe:1"
        ])
        .stdout(Stdio::piped())
        .spawn()?;

    let mut aplay = Command::new("aplay")
        .stdin(ffmpeg.stdout.unwrap())
        .spawn()?;

    aplay.wait()?; // Wait for playback to finish
    Ok(())
}
