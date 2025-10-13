use std::sync::OnceLock;
use anyhow::{Error, Result};

#[cfg(feature = "real")]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(feature = "real")]
use serde_json::json;
#[cfg(feature = "real")]
use reqwest::blocking::Client;
#[cfg(feature = "real")]
use crate::play;

pub struct TTS {
    pub url: String,
    pub tmp_dir: String,
}

static TTS_INSTANCE: OnceLock<TTS> = OnceLock::new();

pub fn init(url: &str, tmp_dir: &str) {
    let _ = TTS_INSTANCE.get_or_init(|| TTS { 
        url: url.to_string(),
        tmp_dir: tmp_dir.to_string(),
    });
}
#[cfg(not(any(feature = "dummy", feature = "real")))]
compile_error!("You must enable either `dummy` or `real` feature for tts!");

#[cfg(feature = "real")]
pub fn say(text: &str, voice: Option<&str>) -> Result<(),Error> {
    let tts = TTS_INSTANCE.get().expect("TTS not initialized");
    let voice = voice.unwrap_or("en_US-ryan-medium");
    let payload = json!({
        "text": text,
        "voice": voice,
    });

    let response = Client::new()
        .post(&tts.url)
        .json(&payload)
        .send()
        .expect("Failed to send request");

    if response.status().is_success() {
        let bytes = response.bytes().expect("Failed to get WAV bytes");
        //let file_path = format!("{}/tts_{}.wav", tts.tmp_dir, current_ts());
        //std::fs::write(&file_path, &bytes).expect("Failed to write WAV file");
        play::play_wav_bytes(&bytes)?;
        Ok(())
    } else {
        eprintln!("TTS request failed with status: {}", response.status());
        Err(anyhow::anyhow!("TTS request failed"))
    }
}

#[cfg(feature = "dummy")]
pub fn say(text: &str, voice: Option<&str>) -> Result<(),Error> {
    println!("(Dummy TTS) Would say: '{}' with voice: {:?}", text, voice);
    Ok(())
}

pub fn sayen(text: &str) -> Result<(),Error> {
    say(text, Some("en_US-ryan-medium"))
}
pub fn saysk(text: &str) -> Result<(),Error> {
    say(text, Some("sk_SK-lili-medium"))
}

#[cfg(feature = "real")]
fn current_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}