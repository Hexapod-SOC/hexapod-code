"""
NavigateBehavior – follow queued waypoints with A*.

Uses SLAM map + pose; replans on map changes or obstacles.
"""

from typing import List, Optional, Tuple
import logging
import math
import numpy as np
from config import settings


class NavigateBehavior:
    def __init__(self, planner, controller, lidar):
        self.planner = planner
        self.controller = controller
        self.lidar = lidar

        self.waypoints: List[Tuple[float, float]] = []
        self.current_goal: Optional[Tuple[float, float]] = None
        self.current_path: Optional[List[Tuple[int, int]]] = None
        self.path_index: int = 0
        self._map_signature: Optional[Tuple[int, int, float, Tuple[float, float, float]]] = None
        self._map_frame: Optional[int] = None
        self._fail_count: int = 0

    def reset(self):
        self.waypoints.clear()
        self.current_goal = None
        self.current_path = None
        self.path_index = 0
        self._map_signature = None
        self._map_frame = None
        self._fail_count = 0

    def set_waypoints(self, waypoints: List[Tuple[float, float]], mode: str = "replace"):
        if mode == "append":
            self.waypoints.extend(waypoints)
        else:
            self.waypoints = list(waypoints)
        if self.waypoints:
            self.current_goal = self.waypoints[0]
            self.current_path = None
            self.path_index = 0
            self._fail_count = 0
        else:
            self.current_goal = None
            self.current_path = None
            self.path_index = 0
            self._fail_count = 0

    def clear_waypoints(self):
        self.reset()

    def step(self) -> str:
        if not self.waypoints:
            return "IDLE"

        grid = self.lidar.update_map()
        if grid is None:
            logging.warning("Navigate: no map available")
            return "NO_MAP"

        pose = self.lidar.get_pose()
        if not pose:
            logging.warning("Navigate: no pose available")
            return "NO_POSE"

        signature = (self.lidar.width, self.lidar.height, self.lidar.resolution, self.lidar.origin)
        if self._map_signature is None:
            self._map_signature = signature
        elif self._map_signature != signature:
            logging.info("Navigate: map changed, replanning")
            self._map_signature = signature
            self.current_path = None
            self.path_index = 0

        if self._map_frame is None:
            self._map_frame = self.lidar.map_frame
        elif self._map_frame != self.lidar.map_frame:
            self._map_frame = self.lidar.map_frame
            self.current_path = None
            self.path_index = 0

        if self.current_goal is None:
            self.current_goal = self.waypoints[0]
            self.current_path = None
            self.path_index = 0
            self._fail_count = 0

        # If close enough, pop and move to next goal
        if self._goal_reached(pose, self.current_goal):
            logging.info("Navigate: reached waypoint")
            self.waypoints.pop(0)
            self.current_goal = self.waypoints[0] if self.waypoints else None
            self.current_path = None
            self.path_index = 0
            self._fail_count = 0
            return "REACHED" if not self.waypoints else "RUNNING"

        pose_px = self._world_to_grid(pose)
        goal_px = self._world_to_grid({"x": self.current_goal[0], "y": self.current_goal[1]})

        inflation_px = self._safe_inflation_px(grid, pose_px)
        blocked = self.planner.get_blocked_map(grid, inflation_px)
        if blocked is not None and blocked[pose_px]:
            nudged = self._nearest_free_cell(blocked, pose_px, radius_px=max(3, inflation_px))
            if nudged is not None:
                logging.warning("Navigate: start blocked, nudging pose from %s to %s", pose_px, nudged)
                pose_px = nudged

        if self.current_path is None or self.path_index >= len(self.current_path):
            path = self.planner.a_star(
                grid,
                pose_px,
                goal_px,
                allow_unknown=False,
                unknown_penalty=3.0,
                inflation_radius_px=inflation_px,
                smooth=True,
            )
            if path is None or len(path) < 2:
                self._fail_count += 1
                logging.warning(f"Navigate: A* failed to {goal_px} (fails={self._fail_count})")
                if self._fail_count >= 5:
                    logging.warning("Navigate: dropping unreachable waypoint")
                    self.waypoints.pop(0)
                    self.current_goal = self.waypoints[0] if self.waypoints else None
                    self.current_path = None
                    self.path_index = 0
                    self._fail_count = 0
                    return "REPLANNING" if self.waypoints else "FINISHED"
                return "REPLANNING"
            self.current_path = path
            self.path_index = 1
            self._fail_count = 0

        self._advance_path_index(pose_px)
        if self.path_index >= len(self.current_path):
            self.current_path = None
            return "REPLANNING"

        target_index = self._lookahead_index(pose_px)
        next_px = self.current_path[target_index]
        next_world = self._grid_to_world(next_px)

        result = self.controller.step_to_goal(pose, next_world)
        if result == "REACHED":
            self.path_index += 1
        elif result == "BLOCKED":
            self.current_path = None

        return "RUNNING"

    def _safe_inflation_px(self, grid: np.ndarray, pose_px: Tuple[int, int]) -> int:
        inflation_px = self._inflation_radius_px()
        if inflation_px <= 0:
            return 0
        while inflation_px > 0:
            blocked = self.planner.get_blocked_map(grid, inflation_px)
            if not blocked[pose_px]:
                return inflation_px
            logging.warning("Navigate: start in obstacle with inflation=%d, reducing", inflation_px)
            inflation_px -= 1
        return 0

    def _nearest_free_cell(self, blocked: np.ndarray, origin: Tuple[int, int], radius_px: int = 3) -> Optional[Tuple[int, int]]:
        r0, c0 = origin
        rows, cols = blocked.shape
        for radius in range(1, radius_px + 1):
            for dr in range(-radius, radius + 1):
                for dc in range(-radius, radius + 1):
                    rr = r0 + dr
                    cc = c0 + dc
                    if rr < 1 or cc < 1 or rr >= rows - 1 or cc >= cols - 1:
                        continue
                    if not blocked[rr, cc]:
                        return (rr, cc)
        return None

    def _goal_reached(self, pose: dict, goal: Tuple[float, float]) -> bool:
        dx = goal[0] - pose.get("x", 0.0)
        dy = goal[1] - pose.get("y", 0.0)
        dist = math.sqrt(dx * dx + dy * dy)
        return dist < self.controller.XY_TOL_M

    def _world_to_grid(self, pose: dict) -> Tuple[int, int]:
        origin_x = self.lidar.origin[0]
        origin_y = self.lidar.origin[1]
        res = self.lidar.resolution
        px_x = int((pose['x'] - origin_x) / res)
        px_y = int((pose['y'] - origin_y) / res)
        return (px_y, px_x)

    def _grid_to_world(self, px: Tuple[int, int]) -> Tuple[float, float]:
        origin_x = self.lidar.origin[0]
        origin_y = self.lidar.origin[1]
        res = self.lidar.resolution
        world_x = px[1] * res + origin_x
        world_y = px[0] * res + origin_y
        return (world_x, world_y)

    def _advance_path_index(self, pose_px: Tuple[int, int]):
        if self.current_path is None:
            return
        advance_px = max(3, int(0.15 / max(self.lidar.resolution, 1e-6)))
        advance_sq = advance_px * advance_px
        while self.path_index < len(self.current_path) - 1:
            node = self.current_path[self.path_index]
            dr = pose_px[0] - node[0]
            dc = pose_px[1] - node[1]
            if (dr * dr + dc * dc) < advance_sq:
                self.path_index += 1
            else:
                break

    def _lookahead_index(self, pose_px: Tuple[int, int]) -> int:
        if self.current_path is None:
            return self.path_index
        lookahead_px = max(4, int(0.35 / max(self.lidar.resolution, 1e-6)))
        lookahead_sq = lookahead_px * lookahead_px
        idx = self.path_index
        while idx < len(self.current_path) - 1:
            node = self.current_path[idx]
            dr = pose_px[0] - node[0]
            dc = pose_px[1] - node[1]
            if (dr * dr + dc * dc) < lookahead_sq:
                idx += 1
            else:
                break
        return idx

    def _inflation_radius_px(self) -> int:
        inflation_m = (settings.ROBOT_RADIUS + settings.SAFETY_DISTANCE) / 1000.0
        if self.lidar.resolution <= 0:
            return 0
        return max(1, int(math.ceil(inflation_m / self.lidar.resolution)))
