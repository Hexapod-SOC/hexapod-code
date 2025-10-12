/// Macro for speaking text in English using TTS
/// 
/// # Examples
/// ```
/// sayen!("Hello world");
/// sayen!("The robot is {}", status);
/// ```
#[macro_export]
macro_rules! sayen {
    ($($arg:tt)*) => {{
        use $crate::audio::{tts::TtsEngine, play::play_wav};
        let text = format!($($arg)*);
        let tts = TtsEngine::new(None);
        let path = tts.say(&text, Some("en_US-amy-medium"));
        if let Err(e) = play_wav(&path.to_string_lossy()) {
            eprintln!("Error playing English TTS: {}", e);
        }
    }};
}

/// Macro for speaking text in Slovak using TTS
/// 
/// # Examples
/// ```
/// saysk!("Ahoj svet");
/// saysk!("Robot je {}", status);
/// ```
#[macro_export]
macro_rules! saysk {
    ($($arg:tt)*) => {{
        use $crate::audio::{tts::TtsEngine, play::play_wav};
        let text = format!($($arg)*);
        let tts = TtsEngine::new(None);
        let path = tts.say(&text, Some("sk-lili-medium"));
        if let Err(e) = play_wav(&path.to_string_lossy()) {
            eprintln!("Error playing Slovak TTS: {}", e);
        }
    }};
}