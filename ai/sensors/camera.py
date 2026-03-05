import logging
import io
import threading
import time
from typing import Optional
from config import settings
try:
    import cv2
    import numpy as np
except ImportError:
    cv2 = None
    np = None

if cv2 is not None and not hasattr(cv2, "VideoCapture"):
    logging.warning("OpenCV module missing VideoCapture; disabling OpenCV backend.")
    cv2 = None

try:
    from picamera2 import Picamera2
except Exception:
    Picamera2 = None

try:
    from PIL import Image
except Exception:
    Image = None

class CameraSensor:
    def __init__(self, device_id: int = 0, width: int = 1280, height: int = 720, fps: int = 15):
        self.available = cv2 is not None
        self.device_id = device_id
        self.width = width
        self.height = height
        self.fps = fps
        self.cap = None
        self.backend = None
        self._lock = threading.Lock()
        
        if self.available:
            try:
                # Iterate to find working camera? Or assume 0/1
                self.cap = cv2.VideoCapture(self.device_id)
                if not self.cap.isOpened():
                    logging.warning(f"Camera {device_id} could not be opened.")
                    self.available = False
                else:
                    if self.width > 0:
                        self.cap.set(cv2.CAP_PROP_FRAME_WIDTH, self.width)
                    if self.height > 0:
                        self.cap.set(cv2.CAP_PROP_FRAME_HEIGHT, self.height)
                    if self.fps > 0:
                        self.cap.set(cv2.CAP_PROP_FPS, self.fps)
                    self.backend = "opencv"
            except Exception as e:
                logging.error(f"Failed to init camera: {e}")
                self.available = False

        if not self.available and Picamera2 is not None:
            try:
                self.cap = Picamera2()
                config = self.cap.create_video_configuration(
                    main={"size": (self.width, self.height), "format": "RGB888"}
                )
                self.cap.configure(config)
                self.cap.start()
                self.available = True
                self.backend = "picamera2"
                logging.info("Camera initialized with Picamera2 backend.")
            except Exception as e:
                logging.error(f"Failed to init Picamera2: {e}")
                self.available = False

    def _capture_frame(self):
        if self.backend == "opencv":
            ret, frame = self.cap.read()
            if ret:
                return frame
            return None

        if self.backend == "picamera2":
            try:
                return self.cap.capture_array()
            except Exception as e:
                logging.warning(f"Picamera2 capture failed: {e}")
                return None

        return None

    def _capture_with_timeout(self) -> Optional[object]:
        result = {"frame": None}

        def _run():
            try:
                result["frame"] = self._capture_frame()
            except Exception as e:
                logging.warning(f"Camera capture failed: {e}")

        thread = threading.Thread(target=_run, daemon=True)
        thread.start()
        thread.join(timeout=max(0.1, settings.CAMERA_CAPTURE_TIMEOUT_S))
        if thread.is_alive():
            logging.warning("Camera capture timeout; marking unavailable")
            self.available = False
            return None
        return result["frame"]

    def get_frame(self):
        if not self.available or not self.cap:
            return None
        with self._lock:
            return self._capture_with_timeout()

    def get_jpeg_bytes(self, quality: int = 85) -> Optional[bytes]:
        if not self.available or not self.cap or cv2 is None:
            if self.backend != "picamera2":
                return None
            if Image is None:
                logging.warning("Pillow not available; cannot encode JPEG.")
                return None
        with self._lock:
            frame = self._capture_with_timeout()
            if frame is None:
                return None

            if cv2 is not None:
                encode_params = [int(cv2.IMWRITE_JPEG_QUALITY), int(quality)]
                ok, buf = cv2.imencode(".jpg", frame, encode_params)
                if not ok:
                    return None
                return buf.tobytes()

            try:
                image = Image.fromarray(frame)
                out = io.BytesIO()
                image.save(out, format="JPEG", quality=int(quality))
                return out.getvalue()
            except Exception as e:
                logging.warning(f"JPEG encoding failed: {e}")
                return None

    def release(self):
        if not self.cap:
            return
        if self.backend == "opencv":
            self.cap.release()
        elif self.backend == "picamera2":
            try:
                self.cap.stop()
            except Exception:
                pass
