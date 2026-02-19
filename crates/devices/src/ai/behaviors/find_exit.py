import math
import heapq
from typing import List, Tuple, Optional
import numpy as np
from clients.hexapod import HexapodClient
from sensors.lidar import LidarSensor

class ExitFinder:
    def __init__(self, client: HexapodClient, lidar: LidarSensor):
        self.client = client
        self.lidar = lidar

    def find_gaps(self, scan_points: List[dict]) -> List[Tuple[float, float, float]]:
        """
        Analyze lidar scan for discontinuities (gaps).
        Returns list of candidate exit points (angle, distance, width).
        """
        # Sort points by angle
        points = sorted(scan_points, key=lambda p: p['angle_deg'])
        gaps = []
        
        # Config
        MIN_GAP_WIDTH = 400 # mm (clearance for robot)
        MAX_GAP_DIST = 3000 # mm
        
        for i in range(len(points) - 1):
            p1 = points[i]
            p2 = points[i+1]
            
            # Check angle difference continuity (skip wrap-around)
            if abs(p1['angle_deg'] - p2['angle_deg']) > 5.0:
                continue
                
            dist1 = p1['distance_mm']
            dist2 = p2['distance_mm']
            
            # Detect depth jump
            # A gap often looks like a sudden jump from wall to empty space (max range)
            # or wall to far wall.
            if abs(dist1 - dist2) > MIN_GAP_WIDTH:
                # Potential gap edge
                # Check if it's a gap wide enough
                # This simple check is insufficient, need to group points.
                pass
                
        # Better approach:
        # Find segments of "max range" readings or depth discontinuities
        # Iterate and look for runs of "Open Space"
        
        return gaps

    def run(self):
        # 1. Scan
        scan = self.lidar.get_scan()
        if not scan:
            return "NO_DATA"
            
        # 2. Identify Gaps
        # 3. Score Gaps (width, distance, straightness)
        # 4. Navigate to best
        pass
