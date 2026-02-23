import requests
import time
from typing import Dict, Any, Optional
from config import settings

class HexapodClient:
    def __init__(self):
        self.base_url = settings.HEXAPOD_API_BASE
        self.session = requests.Session()

    def get_status(self) -> Dict[str, Any]:
        try:
            resp = self.session.get(f"{self.base_url}/status", timeout=1.0)
            return resp.json() if resp.status_code == 200 else {}
        except Exception:
            return {}

    def get_battery(self) -> Dict[str, Any]:
        try:
            resp = self.session.get(f"{self.base_url}/battery", timeout=1.0)
            return resp.json() if resp.status_code == 200 else {}
        except Exception:
            return {}

    def get_pose(self) -> Dict[str, float]:
        """Get current body pose (roll, pitch, yaw)."""
        # The /status endpoint returns body pose in some versions, or we might need /legs
        # Based on routes.rs, /legs returns BodyPoseData
        try:
            resp = self.session.get(f"{self.base_url}/legs", timeout=1.0)
            if resp.status_code == 200:
                data = resp.json()
                return data.get("body_pose", {})
            return {}
        except Exception:
            return {}

    def move(self, forward: float, strafe: float, rotation: float):
        """Send movement command."""
        payload = {
            "forward": float(forward),
            "strafe": float(strafe),
            "rotation": float(rotation)
        }
        try:
            self.session.post(f"{self.base_url}/move", json=payload, timeout=0.5)
        except Exception as e:
            print(f"Error sending move command: {e}")

    def stop(self):
        """Emergency stop."""
        try:
            self.session.post(f"{self.base_url}/stop", json={}, timeout=0.5)
        except Exception as e:
            print(f"Error sending stop command: {e}")

    def set_gait(self, gait_name: str):
        try:
            self.session.post(f"{self.base_url}/gait", json={"gait_name": gait_name}, timeout=1.0)
        except Exception as e:
            print(f"Error setting gait: {e}")

    def get_lidar_map(self) -> Optional[Dict[str, Any]]:
        """Get the latest SLAM map."""
        try:
            resp = self.session.get(f"{self.base_url}/lidar/map", timeout=2.0)
            if resp.status_code == 200:
                return resp.json()
            return None
        except Exception as e:
            print(f"Error getting lidar map: {e}")
            return None

    def get_lidar_frame(self) -> Optional[Dict[str, Any]]:
        """Get the latest LIDAR scan points."""
        try:
            resp = self.session.get(f"{self.base_url}/lidar/frame", timeout=1.0)
            if resp.status_code == 200:
                return resp.json()
            return None
        except Exception:
            return None

    def speak(self, text: str):
        """Send TTS command."""
        try:
            self.session.post(f"{self.base_url}/tts", json={"text": text}, timeout=1.0)
        except Exception as e:
            print(f"Error sending TTS: {e}")
