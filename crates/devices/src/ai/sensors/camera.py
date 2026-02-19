import logging
from typing import Optional
try:
    import cv2
    import numpy as np
except ImportError:
    cv2 = None
    np = None

class CameraSensor:
    def __init__(self, device_id: int = 0):
        self.available = cv2 is not None
        self.device_id = device_id
        self.cap = None
        
        if self.available:
            try:
                # Iterate to find working camera? Or assume 0/1
                self.cap = cv2.VideoCapture(self.device_id)
                if not self.cap.isOpened():
                    logging.warning(f"Camera {device_id} could not be opened.")
                    self.available = False
            except Exception as e:
                logging.error(f"Failed to init camera: {e}")
                self.available = False

    def get_frame(self):
        if not self.available or not self.cap:
            return None
        
        ret, frame = self.cap.read()
        if ret:
            return frame
        return None

    def release(self):
        if self.cap:
            self.cap.release()
