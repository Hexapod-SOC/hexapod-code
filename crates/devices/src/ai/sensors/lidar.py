import numpy as np
from typing import Optional, Dict, Any, Tuple
from clients.hexapod import HexapodClient
from config import settings

class LidarSensor:
    def __init__(self, client: HexapodClient):
        self.client = client
        self.last_map: Optional[np.ndarray] = None
        self.origin: Tuple[float, float, float] = (0, 0, 0)
        self.resolution: float = settings.MAP_RESOLUTION
        self.width: int = 0
        self.height: int = 0

    def update_map(self) -> Optional[np.ndarray]:
        """Fetch and parse occupancy grid from API."""
        data = self.client.get_lidar_map()
        if not data:
            return None

        self.width = data.get("width", 0)
        self.height = data.get("height", 0)
        self.resolution = data.get("resolution", 0.05)
        
        origin_data = data.get("origin", {})
        self.origin = (origin_data.get("x", 0), origin_data.get("y", 0), origin_data.get("theta", 0))

        cells = data.get("cells", [])
        if len(cells) != self.width * self.height:
            return None

        # Convert to numpy array (row-major)
        # 0 = free, 100 = occupied, -1 = unknown
        # We'll normalize to 0.0-1.0 probability, -1 remains -1
        grid = np.array(cells, dtype=np.int8).reshape((self.height, self.width))
        self.last_map = grid
        return grid

    def get_scan(self) -> Optional[Dict[str, Any]]:
        """Get raw scan points."""
        return self.client.get_lidar_frame()

    def check_obstacle(self, distance_mm: int = 300) -> bool:
        """Simple safety check using raw scan data."""
        scan = self.get_scan()
        if not scan:
            return False # Conservative: assume safe if no data? Or unsafe?
                         # Better to use map for navigation, this is for e-stop

        points = scan.get("points", [])
        for p in points:
            dist = p.get("distance_mm", 10000)
            if 10 < dist < distance_mm:
                return True
        return False
