use std::sync::{OnceLock, Mutex};
use anyhow::{Error, Result};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};


#[cfg(feature = "real")]
use serde_json::json;
#[cfg(feature = "real")]
use reqwest::blocking::Client;
#[cfg(feature = "real")]
use crate::play;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CacheEntry {
    text: String,
    voice: String,
    file_path: String,
    use_count: u32,
    last_used: u64, // Unix timestamp in seconds
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheData {
    entries: HashMap<String, CacheEntry>,
}

impl CacheData {
    fn new() -> Self {
        CacheData {
            entries: HashMap::new(),
        }
    }
}

pub struct TTS {
    pub url: String,
    pub tmp_dir: String,
    #[allow(dead_code)]
    cache_dir: String,
    cache_file: String,
    cache: Mutex<CacheData>,
}

static TTS_INSTANCE: OnceLock<TTS> = OnceLock::new();

pub fn init(url: &str, tmp_dir: &str) {
    let _ = TTS_INSTANCE.get_or_init(|| {
        let cache_file = format!("{}/tts/cache.toml", tmp_dir);
        let cache_dir = format!("{}/tts/cache", tmp_dir);
        
        // Create cache directory if it doesn't exist
        std::fs::create_dir_all(&cache_dir).ok();
        
        // Load cache from file or create new
        let cache = if std::path::Path::new(&cache_file).exists() {
            match std::fs::read_to_string(&cache_file) {
                Ok(content) => match toml::from_str::<CacheData>(&content) {
                    Ok(data) => data,
                    Err(_) => CacheData::new(),
                },
                Err(_) => CacheData::new(),
            }
        } else {
            CacheData::new()
        };
        
        TTS { 
            url: url.to_string(),
            tmp_dir: format!("{}/tts/tmp", tmp_dir),
            cache_dir,
            cache_file,
            cache: Mutex::new(cache),
        }
    });
}
#[allow(dead_code)]
fn generate_cache_key(text: &str, voice: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}", text, voice));
    format!("{:x}", hasher.finalize())
}

fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn save_cache_to_file(tts: &TTS) -> Result<()> {
    let cache = tts.cache.lock().unwrap();
    let toml_string = toml::to_string(&*cache)?;
    std::fs::write(&tts.cache_file, toml_string)?;
    Ok(())
}
#[cfg(not(any(feature = "dummy", feature = "real")))]
compile_error!("You must enable either `dummy` or `real` feature for tts!");

#[cfg(feature = "real")]
fn say_impl(text: &str, voice: Option<&str>, blocking: bool) -> Result<(), Error> {
    let tts = TTS_INSTANCE.get().expect("TTS not initialized");
    let voice = voice.unwrap_or("en_US-ryan-medium");
    let cache_key = generate_cache_key(text, voice);
    //println!("{}", format!("{}", cache_key));
    
    // Check cache first
    {
        let mut cache = tts.cache.lock().unwrap();
        
        if let Some(entry) = cache.entries.get_mut(&cache_key) {
            // Found in cache, play from file
            let cache_file_path = format!("{}/{}.wav", tts.cache_dir, cache_key);
            
            if std::path::Path::new(&cache_file_path).exists() {
                entry.use_count += 1;
                entry.last_used = get_current_timestamp();
                drop(cache); // Release lock before playing
                
                let bytes = std::fs::read(&cache_file_path)?;
                if blocking {
                    play::play_wav_bytes(&bytes)?;
                } else {
                    play::play_wav_bytes_detached(&bytes)?;
                }
                
                // Save updated cache
                save_cache_to_file(tts)?;
                return Ok(());
            }
        }
    }
    
    // Generate TTS audio
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
        
        // Check if this is the second time we're saying this
        {
            let mut cache = tts.cache.lock().unwrap();
            let current_time = get_current_timestamp();
            
            if let Some(entry) = cache.entries.get_mut(&cache_key) {
                println!("Second time saying this, caching the WAV file.");
                entry.use_count += 1;
                entry.last_used = current_time;
                let cache_file_path = format!("{}/{}.wav", tts.cache_dir, cache_key);
                std::fs::write(&cache_file_path, &bytes)?;
                entry.file_path = cache_file_path;
            } else {
                // First use - just record it
                println!("First time saying this, not caching yet.");
                cache.entries.insert(cache_key.clone(), CacheEntry {
                    text: text.to_string(),
                    voice: voice.to_string(),
                    file_path: String::new(),
                    use_count: 1,
                    last_used: current_time,
                });
                println!("Cache entry added, will cache on next use.");
            }
            
            // Drop lock before saving to avoid deadlock
            drop(cache);
        }
        
        // Save cache metadata (after releasing the lock)
        save_cache_to_file(tts)?;
        
        println!("Playing generated TTS audio.");
        if blocking {
            play::play_wav_bytes(&bytes)?;
        } else {
            play::play_wav_bytes_detached(&bytes)?;
        }
        Ok(())
    } else {
        eprintln!("TTS request failed with status: {}", response.status());
        Err(anyhow::anyhow!("TTS request failed"))
    }
}

#[cfg(feature = "real")]
pub fn say(text: &str, voice: Option<&str>) -> Result<(), Error> {
    say_impl(text, voice, false)
}

#[cfg(feature = "real")]
pub fn say_blocking(text: &str, voice: Option<&str>) -> Result<(), Error> {
    say_impl(text, voice, true)
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
pub fn sayen_blocking(text: &str) -> Result<(), Error> {
    say_blocking(text, Some("en_US-ryan-medium"))
}

#[cfg(feature = "real")]
pub fn saysk_blocking(text: &str) -> Result<(), Error> {
    say_blocking(text, Some("sk_SK-lili-medium"))
}

#[cfg(feature = "dummy")]
pub fn say_blocking(text: &str, voice: Option<&str>) -> Result<(), Error> {
    println!("(Dummy TTS - blocking) Would say: '{}' with voice: {:?}", text, voice);
    Ok(())
}

#[cfg(feature = "dummy")]
pub fn sayen_blocking(text: &str) -> Result<(), Error> {
    say_blocking(text, Some("en_US-ryan-medium"))
}

#[cfg(feature = "dummy")]
pub fn saysk_blocking(text: &str) -> Result<(), Error> {
    say_blocking(text, Some("sk_SK-lili-medium"))
}

/// Remove cache entries that were used only once and haven't been accessed in the specified number of days
pub fn cleanup_cache(days_threshold: u64) -> Result<()> {
    let tts = TTS_INSTANCE.get().expect("TTS not initialized");
    let current_time = get_current_timestamp();
    let threshold_seconds = days_threshold * 24 * 60 * 60;
    
    let mut cache = tts.cache.lock().unwrap();
    let mut to_remove = Vec::new();
    
    for (key, entry) in cache.entries.iter() {
        // Only remove entries that were used once and haven't been accessed recently
        if entry.use_count == 1 && (current_time - entry.last_used) > threshold_seconds {
            to_remove.push(key.clone());
            
            // Delete the cached file if it exists
            if !entry.file_path.is_empty() && std::path::Path::new(&entry.file_path).exists() {
                std::fs::remove_file(&entry.file_path).ok();
            }
        }
    }
    
    // Remove entries from cache
    for key in &to_remove {
        cache.entries.remove(key);
    }
    
    drop(cache);
    
    if !to_remove.is_empty() {
        println!("Cleaned up {} cache entries", to_remove.len());
        save_cache_to_file(tts)?;
    }
    
    Ok(())
}