import os
from pathlib import Path
from pydantic import BaseModel

class Settings(BaseModel):
    # API Configuration
    HEXAPOD_API_BASE: str = os.getenv("HEXAPOD_API_BASE", "http://127.0.0.1:3000/api")
    AI_HOST: str = "0.0.0.0"
    AI_PORT: int = int(os.getenv("AI_CHAT_PORT", "3001"))
    
    # OpenAI
    OPENAI_API_KEY: str = ""
    OPENAI_MODEL: str = "gpt-5-mini"
    OPENAI_WHISPER_MODEL: str = os.getenv("OPENAI_WHISPER_MODEL", "gpt-4o-transcribe")
    OPENAI_TRANSCRIBE_LANGUAGE: str = os.getenv("OPENAI_TRANSCRIBE_LANGUAGE", "en")
    OPENAI_TRANSCRIBE_PROMPT: str = os.getenv("OPENAI_TRANSCRIBE_PROMPT", "hexapod ninja")
    OPENAI_TRANSCRIBE_RETRY_MODEL: str = os.getenv("OPENAI_TRANSCRIBE_RETRY_MODEL", "whisper-1")
    OPENAI_TRANSCRIBE_MIN_CHARS: int = int(os.getenv("OPENAI_TRANSCRIBE_MIN_CHARS", "3"))
    OPENAI_TRANSCRIBE_MIN_ALPHA_RATIO: float = float(os.getenv("OPENAI_TRANSCRIBE_MIN_ALPHA_RATIO", "0.3"))
    SAVE_TRANSCRIBE_AUDIO: bool = os.getenv("SAVE_TRANSCRIBE_AUDIO", "false").lower() == "true"
    TRANSCRIBE_AUDIO_DIR: str = os.getenv("TRANSCRIBE_AUDIO_DIR", "./transcribe_samples")

    # Picovoice Porcupine (wake word detection)
    PICOVOICE_ACCESS_KEY: str = os.getenv("PICOVOICE_ACCESS_KEY", "")
    PORCUPINE_KEYWORD_PATHS: str = os.getenv("PORCUPINE_KEYWORD_PATHS", "")
    PORCUPINE_KEYWORDS: str = os.getenv("PORCUPINE_KEYWORDS", "bumblebee")
    PORCUPINE_SENSITIVITIES: str = os.getenv("PORCUPINE_SENSITIVITIES", "0.65,0.65")

    # Wake/utterance detection (ms)
    WAKE_PRE_ROLL_MS: int = int(os.getenv("WAKE_PRE_ROLL_MS", "500"))
    WAKE_TRIM_MS: int = int(os.getenv("WAKE_TRIM_MS", "2"))
    WAKE_SILENCE_MS: int = int(os.getenv("WAKE_SILENCE_MS", "500"))
    WAKE_MAX_UTTERANCE_MS: int = int(os.getenv("WAKE_MAX_UTTERANCE_MS", "8000"))
    WAKE_MIN_UTTERANCE_MS: int = int(os.getenv("WAKE_MIN_UTTERANCE_MS", "400"))
    WAKE_RMS_THRESHOLD: int = int(os.getenv("WAKE_RMS_THRESHOLD", "500"))

    # Microphone input source ("web" for browser mic, "onboard" for future hardware mic)
    MIC_SOURCE: str = os.getenv("AI_MIC_SOURCE", "web")
    MIC_SAMPLE_RATE: int = int(os.getenv("AI_MIC_SAMPLE_RATE", "24000"))
    
    # Robot Physical Constraints (mm)
    ROBOT_RADIUS: int = 250  # mm (approximate standing radius)
    SAFETY_DISTANCE: int = 200  # mm
    CRITICAL_DISTANCE: int = 200  # mm
    
    # Navigation
    LIDAR_MAX_RANGE: int = 12000 # mm
    MAP_SIZE_PIXELS: int = 1000  # Assuming 10cm or similar resolution from SLAM
    MAP_RESOLUTION: float = 0.05 # Meters per pixel, synced with SLAM config
    INVERT_LIDAR_LEFT_RIGHT: bool = os.getenv("INVERT_LIDAR_LEFT_RIGHT", "false").lower() == "true"
    INVERT_HEADING_ERROR: bool = os.getenv("INVERT_HEADING_ERROR", "true").lower() == "true"
    
    # Behavior
    MAX_SPIN_RETRIES: int = 3
    
    class Config:
        env_file = ".env"

def load_api_key() -> str:
    """Load OpenAI API key from file or env."""
    key = os.getenv("OPENAI_API_KEY")
    if key:
        return key
        
    key_file = Path(__file__).parent / "apikey.txt"
    if key_file.exists():
        return key_file.read_text().strip()
    return ""

settings = Settings()
settings.OPENAI_API_KEY = load_api_key()
