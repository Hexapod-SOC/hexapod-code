use anyhow::{anyhow, Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex, OnceLock,
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(feature = "real")]
use crate::play;
#[cfg(feature = "real")]
use reqwest::blocking::Client;
#[cfg(feature = "real")]
use serde_json::json;

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

struct TtsShared {
    url: String,
    tmp_dir: String,
    cache_dir: String,
    cache_file: String,
    cache: Mutex<CacheData>,
}

#[cfg(feature = "real")]
struct TtsJob {
    text: String,
    voice: String,
    blocking: bool,
    respond_to: Option<Sender<Result<(), Error>>>,
}

static TTS_SHARED: OnceLock<Arc<TtsShared>> = OnceLock::new();
#[cfg(feature = "real")]
static TTS_SENDER: OnceLock<Sender<TtsJob>> = OnceLock::new();

pub fn init(url: &str, tmp_dir: &str) {
    if TTS_SHARED.get().is_some() {
        return;
    }

    let cache_file = format!("{}/tts/cache.toml", tmp_dir);
    let cache_dir = format!("{}/tts/cache", tmp_dir);
    let tmp_dir_path = format!("{}/tts/tmp", tmp_dir);

    std::fs::create_dir_all(&cache_dir).ok();
    std::fs::create_dir_all(&tmp_dir_path).ok();

    let cache = if std::path::Path::new(&cache_file).exists() {
        match std::fs::read_to_string(&cache_file) {
            Ok(content) => {
                toml::from_str::<CacheData>(&content).unwrap_or_else(|_| CacheData::new())
            }
            Err(_) => CacheData::new(),
        }
    } else {
        CacheData::new()
    };

    let shared = Arc::new(TtsShared {
        url: url.to_string(),
        tmp_dir: tmp_dir_path,
        cache_dir,
        cache_file,
        cache: Mutex::new(cache),
    });

    let _ = TTS_SHARED.set(shared.clone());

    #[cfg(feature = "real")]
    {
        let (tx, rx) = mpsc::channel::<TtsJob>();
        let _ = TTS_SENDER.set(tx);

        thread::spawn(move || {
            worker_loop(shared, rx);
        });
    }
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

fn save_cache_to_file(tts: &TtsShared) -> Result<()> {
    let cache = tts
        .cache
        .lock()
        .map_err(|_| anyhow!("Cache lock poisoned"))?;
    let toml_string = toml::to_string(&*cache)?;
    std::fs::write(&tts.cache_file, toml_string)?;
    Ok(())
}
#[cfg(not(any(feature = "dummy", feature = "real")))]
compile_error!("You must enable either `dummy` or `real` feature for tts!");

#[cfg(feature = "real")]
fn say_impl(text: &str, voice: Option<&str>, blocking: bool) -> Result<(), Error> {
    let sender = TTS_SENDER
        .get()
        .ok_or_else(|| anyhow!("TTS not initialized"))?;
    let voice = voice.unwrap_or("en_US-ryan-medium").to_string();

    if blocking {
        let (tx, rx) = mpsc::channel();
        sender
            .send(TtsJob {
                text: text.to_string(),
                voice,
                blocking,
                respond_to: Some(tx),
            })
            .map_err(|e| anyhow!("Failed to enqueue TTS job: {}", e))?;

        rx.recv()
            .map_err(|e| anyhow!("Failed to receive TTS result: {}", e))?
    } else {
        sender
            .send(TtsJob {
                text: text.to_string(),
                voice,
                blocking,
                respond_to: None,
            })
            .map_err(|e| anyhow!("Failed to enqueue TTS job: {}", e))?;
        Ok(())
    }
}

#[cfg(feature = "real")]
fn worker_loop(shared: Arc<TtsShared>, rx: Receiver<TtsJob>) {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build TTS HTTP client");

    for job in rx {
        let result = process_job(&shared, &client, &job);
        if let Some(responder) = job.respond_to {
            let _ = responder.send(result);
        }
    }
}

#[cfg(feature = "real")]
fn process_job(shared: &Arc<TtsShared>, client: &Client, job: &TtsJob) -> Result<(), Error> {
    let cache_key = generate_cache_key(&job.text, &job.voice);

    // Fast-path: try cache first
    {
        let mut cache = shared
            .cache
            .lock()
            .map_err(|_| anyhow!("Cache lock poisoned"))?;

        if let Some(entry) = cache.entries.get_mut(&cache_key) {
            let cache_file_path = format!("{}/{}.wav", shared.cache_dir, cache_key);
            if std::path::Path::new(&cache_file_path).exists() {
                entry.use_count += 1;
                entry.last_used = get_current_timestamp();
                drop(cache);

                let bytes = std::fs::read(&cache_file_path)?;
                if job.blocking {
                    play::play_wav_bytes(&bytes)?;
                } else {
                    play::play_wav_bytes_detached(&bytes)?;
                }

                save_cache_to_file(shared)?;
                return Ok(());
            }
        }
    }

    // Not cached, generate via HTTP
    let payload = json!({
        "text": job.text,
        "voice": job.voice,
    });

    let response = client
        .post(&shared.url)
        .json(&payload)
        .send()
        .map_err(|e| anyhow!("Failed to send TTS request: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "TTS request failed with status: {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .map_err(|e| anyhow!("Failed to read TTS response bytes: {}", e))?
        .to_vec();

    {
        let mut cache = shared
            .cache
            .lock()
            .map_err(|_| anyhow!("Cache lock poisoned"))?;
        let current_time = get_current_timestamp();
        let cache_file_path = format!("{}/{}.wav", shared.cache_dir, cache_key);

        // Write file immediately so subsequent calls are instant
        if let Err(e) = std::fs::write(&cache_file_path, &bytes) {
            eprintln!("Failed to write cache file {}: {}", cache_file_path, e);
        }

        let entry = cache
            .entries
            .entry(cache_key.clone())
            .or_insert(CacheEntry {
                text: job.text.clone(),
                voice: job.voice.clone(),
                file_path: cache_file_path.clone(),
                use_count: 0,
                last_used: current_time,
            });

        entry.use_count += 1;
        entry.last_used = current_time;
        entry.file_path = cache_file_path;

        drop(cache);
    }

    save_cache_to_file(shared)?;

    if job.blocking {
        play::play_wav_bytes(&bytes)?;
    } else {
        play::play_wav_bytes_detached(&bytes)?;
    }

    Ok(())
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
pub fn say(text: &str, voice: Option<&str>) -> Result<(), Error> {
    println!("(Dummy TTS) Would say: '{}' with voice: {:?}", text, voice);
    Ok(())
}

pub fn sayen(text: &str) -> Result<(), Error> {
    say(text, Some("en_US-ryan-medium"))
}
pub fn saysk(text: &str) -> Result<(), Error> {
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
    println!(
        "(Dummy TTS - blocking) Would say: '{}' with voice: {:?}",
        text, voice
    );
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
    let tts = TTS_SHARED.get().expect("TTS not initialized");
    let current_time = get_current_timestamp();
    let threshold_seconds = days_threshold * 24 * 60 * 60;

    let mut cache = tts
        .cache
        .lock()
        .map_err(|_| anyhow!("Cache lock poisoned"))?;
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
