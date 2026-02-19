from typing import List, Tuple
import numpy as np

class ExploreBehavior:
    def __init__(self, planner, controller, lidar):
        self.planner = planner
        self.controller = controller
        self.lidar = lidar
        self.visited = set()
        self.frontier_blacklist = set()

    def get_frontiers(self, grid: np.ndarray) -> List[Tuple[int, int]]:
        """
        Identify frontiers: boundary between free space and unknown space.
        grid: 0-100 probability. -1 unknown?
        Actually our LidarSensor normalized it?
        Let's assume:
        0-49: Free
        50-100: Occupied
        -1: Unknown
        
        A frontier cell is a Free cell adjacent to an Unknown cell.
        """
        rows, cols = grid.shape
        frontiers = []
        
        # Optimize by only checking near known free space?
        # Brute force for now (small 50x50 or 100x100 grid ok)
        for r in range(1, rows-1):
            for c in range(1, cols-1):
                if 0 <= grid[r, c] < 50: # Free
                    # Check neighbors for unknown (-1)
                    is_frontier = False
                    for dr, dc in [(-1,0), (1,0), (0,-1), (0,1)]:
                        if grid[r+dr, c+dc] == -1:
                            is_frontier = True
                            break
                    if is_frontier:
                        frontiers.append((r, c))
        return frontiers

    def select_best_frontier(self, current_pose: Tuple[int, int], frontiers: List[Tuple[int, int]]) -> Tuple[int, int]:
        # Nearest frontier
        if not frontiers:
            return None
            
        best = None
        min_dist = float('inf')
        
        for f in frontiers:
            if f in self.frontier_blacklist:
                continue
                
            dist = (f[0]-current_pose[0])**2 + (f[1]-current_pose[1])**2
            if dist < min_dist:
                min_dist = dist
                best = f
        return best

    def step(self):
        # 1. Get Map
        grid = self.lidar.update_map() # This might be heavy
        if grid is None:
            return "NO_MAP"
            
        # 2. Get Pose (pixel coords)
        # We need to convert from world pose (meters) to grid (pixels)
        # Using origin and resolution from LidarSensor
        # pose_world = self.controller.client.get_pose() (wait, get_lidar_frame returns pose in meters)
        # We need synchronization. LidarSensor stores origin.
        # Let's assume we can get pose in grid coords from LidarSensor or Client
        
        # Simplified:
        # scan = self.lidar.get_scan()
        # pose_m = scan['pose']
        # pose_px_x = int((pose_m.x - origin.x) / resolution)
        # pose_px_y = int((pose_m.y - origin.y) / resolution)
        
        # For this skeleton, assume we have pose_px
        pose_px = (50, 50) # Mock
        
        # 3. Find Frontiers
        frontiers = self.get_frontiers(grid)
        
        # 4. Select Goal
        goal = self.select_best_frontier(pose_px, frontiers)
        if not goal:
            return "FINISHED"
            
        # 5. Plan Path
        path = self.planner.a_star(grid, pose_px, goal)
        if not path:
            self.frontier_blacklist.add(goal)
            return "PLAN_FAILED"
            
        # 6. Execute (just first step for now, main loop handles frequency)
        # Convert next path node to world coords
        # next_node = path[1]
        # target_x = next_node[0] * resolution + origin.x
        # target_y = next_node[1] * resolution + origin.y
        # self.controller.step_to_goal(current_pose, (target_x, target_y))
        
        return "RUNNING"
