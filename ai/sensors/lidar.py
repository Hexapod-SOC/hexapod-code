import numpy as np
import threading
from typing import Optional, Dict, Any, Tuple
from clients.hexapod import HexapodClient
from config import settings


class LidarSensor:
    def __init__(self, client: HexapodClient):
        self.client = client
        self.last_map: Optional[np.ndarray] = None
        self.last_pose: Optional[Dict[str, float]] = None
        self.last_scan: Optional[Dict] = None
        self.map_frame: int = 0
        self.origin: Tuple[float, float, float] = (0.0, 0.0, 0.0)
        self.resolution: float = settings.MAP_RESOLUTION
        self.width: int = 0
        self.height: int = 0

    def update_map(self) -> Optional[np.ndarray]:
        """Fetch and parse occupancy grid from the API."""
        data = self._fetch_map_with_timeout()
        if not data:
            return None

        self.width = data.get("width", 0)
        self.height = data.get("height", 0)
        self.resolution = data.get("resolution", 0.05)
        self.map_frame = data.get("frame", 0)

        origin_data = data.get("origin", {})
        self.origin = (
            origin_data.get("x", 0.0),
            origin_data.get("y", 0.0),
            origin_data.get("theta", 0.0),
        )

        cells = data.get("cells", [])
        if not cells or len(cells) != self.width * self.height:
            return None

        # Map uses log-odds i8 values (negative=free, 0=unknown, positive=occupied)
        grid = np.array(cells, dtype=np.int8).reshape((self.height, self.width))
        self.last_map = grid

        # Store pose included in map response
        pose_data = data.get("pose", {})
        if pose_data:
            self.last_pose = {
                "x":     pose_data.get("x",     0.0),
                "y":     pose_data.get("y",     0.0),
                "theta": pose_data.get("theta", 0.0),
            }

        return grid

    def _fetch_map_with_timeout(self) -> Optional[Dict[str, Any]]:
        result: Dict[str, Any] = {"data": None}

        def _run():
            try:
                result["data"] = self.client.get_lidar_map()
            except Exception:
                result["data"] = None

        thread = threading.Thread(target=_run, daemon=True)
        thread.start()
        thread.join(timeout=max(0.1, settings.LIDAR_MAP_TIMEOUT_S))
        if thread.is_alive():
            return None
        return result["data"]

    def get_pose(self) -> Optional[Dict[str, float]]:
        """Return the last known robot pose from SLAM (updated by update_map)."""
        return self.last_pose

    def get_scan(self) -> Optional[Dict]:
        """Return raw LIDAR scan points."""
        self.last_scan = self.client.get_lidar_frame()
        return self.last_scan

    def get_map_stats(self) -> Dict[str, Any]:
        """Return basic map stats for debugging."""
        stats = {
            "frame": self.map_frame,
            "width": self.width,
            "height": self.height,
            "resolution": self.resolution,
            "has_map": self.last_map is not None,
        }
        if self.last_map is None:
            return stats

        grid = self.last_map
        total = grid.size
        unknown = int((grid == 0).sum())
        occupied = int((grid > 0).sum())
        free = int((grid < 0).sum())
        stats.update({
            "cells_total": int(total),
            "cells_unknown": unknown,
            "cells_free": free,
            "cells_occupied": occupied,
        })
        return stats

    def check_obstacle(self, distance_mm: int = 300) -> bool:
        """Return True if any scan point is closer than distance_mm."""
        scan = self.get_scan()
        if not scan:
            return False
        for p in scan.get("points", []):
            dist = p.get("distance_mm", 99999)
            if 10 < dist < distance_mm:
                return True
        return False
