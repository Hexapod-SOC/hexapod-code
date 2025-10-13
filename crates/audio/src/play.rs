use std::process::{Command, Stdio};
use std::path::Path;
use std::io::{self, Write};

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
