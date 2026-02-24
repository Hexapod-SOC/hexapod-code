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
    OPENAI_MODEL: str = "gpt-4-turbo-preview"
    OPENAI_WHISPER_MODEL: str = os.getenv("OPENAI_WHISPER_MODEL", "whisper-1")
    
    # Robot Physical Constraints (mm)
    ROBOT_RADIUS: int = 250  # mm (approximate standing radius)
    SAFETY_DISTANCE: int = 300  # mm
    CRITICAL_DISTANCE: int = 200  # mm
    
    # Navigation
    LIDAR_MAX_RANGE: int = 12000 # mm
    MAP_SIZE_PIXELS: int = 1000  # Assuming 10cm or similar resolution from SLAM
    MAP_RESOLUTION: float = 0.05 # Meters per pixel, synced with SLAM config
    
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
