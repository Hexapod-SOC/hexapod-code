// audio/tts.rs
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use once_cell::sync::Lazy;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use toml;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CacheEntry {
    file: String,
    last_used: u64,
    hits: u32,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct TtsCache {
    entries: HashMap<String, CacheEntry>,
}

static CACHE: Lazy<Mutex<TtsCache>> = Lazy::new(|| Mutex::new(load_cache()));

const CACHE_FILE: &str = "/home/dietpi/tts_cache.toml";
const CACHE_DIR: &str = "/home/dietpi/tts_cache/";
const DEFAULT_VOICE: &str = "en_US-amy-medium";

pub struct TtsEngine {
    client: Client,
    api_url: String,
    default_voice: String,
}

impl TtsEngine {
    pub fn new(api_url: Option<&str>) -> Self {
        fs::create_dir_all(CACHE_DIR).unwrap();
        
        // Get URL from parameter, environment variable, or default
        // Use 127.0.0.1 instead of localhost to avoid DNS issues
        let url = api_url
            .map(|s| s.to_string())
            .or_else(|| std::env::var("TTS_SERVER_URL").ok())
            .unwrap_or_else(|| "http://127.0.0.1:5000".to_string());
        
        // Get default voice from environment variable or use constant
        let voice = std::env::var("TTS_DEFAULT_VOICE")
            .unwrap_or_else(|_| DEFAULT_VOICE.to_string());
        
        Self {
            client: Client::new(),
            api_url: url,
            default_voice: voice,
        }
    }

    pub fn say(&self, text: &str, voice: Option<&str>) -> PathBuf {
        let mut cache = CACHE.lock().unwrap();
        if let Some(entry) = cache.entries.get(text).cloned() {
            if Path::new(&entry.file).exists() {
                cache.entries.insert(
                    text.to_string(),
                    CacheEntry {
                        last_used: current_ts(),
                        hits: entry.hits + 1,
                        file: entry.file.clone(),
                    },
                );
                save_cache(&cache);
                return PathBuf::from(entry.file);
            }
        }

        // request new TTS
        let tmp_file = format!("/tmp/tts_{}.wav", current_ts());
        let voice_to_use = voice.unwrap_or(&self.default_voice);
        let payload = json!({
            "text": text,
            "voice": voice_to_use
        });

        let response = self
            .client
            .post(&self.api_url)
            .json(&payload)
            .send()
            .expect("Failed to send request");

        let bytes = response.bytes().expect("Failed to get WAV bytes");
        fs::write(&tmp_file, &bytes).expect("Failed to write temp WAV file");

        // move to cache dir (use copy+delete for cross-device compatibility)
        let cached_file = format!("{}/{}.wav", CACHE_DIR, sanitize_filename(text));
        fs::copy(&tmp_file, &cached_file).expect("Failed to copy to cache");
        fs::remove_file(&tmp_file).ok(); // Clean up temp file

        cache.entries.insert(
            text.to_string(),
            CacheEntry {
                file: cached_file.clone(),
                last_used: current_ts(),
                hits: 1,
            },
        );
        save_cache(&cache);

        PathBuf::from(cached_file)
    }
}

/// Helpers
fn current_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn sanitize_filename(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

fn load_cache() -> TtsCache {
    if let Ok(contents) = fs::read_to_string(CACHE_FILE) {
        toml::from_str(&contents).unwrap_or_default()
    } else {
        TtsCache::default()
    }
}

fn save_cache(cache: &TtsCache) {
    let toml_str = toml::to_string(&cache).unwrap();
    fs::write(CACHE_FILE, toml_str).unwrap();
}

pub fn say(text: &str, voice: Option<&str>) -> PathBuf {
    let tts = TtsEngine::new(None);
    tts.say(text, voice)
}