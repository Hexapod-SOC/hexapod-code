"""
NavigateBehavior – follow queued waypoints with A*.

Uses SLAM map + pose; replans on map changes or obstacles.
"""

from typing import List, Optional, Tuple
import logging
import math
import numpy as np


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
        self._fail_count: int = 0

    def reset(self):
        self.waypoints.clear()
        self.current_goal = None
        self.current_path = None
        self.path_index = 0
        self._map_signature = None
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

        if self.current_path is None or self.path_index >= len(self.current_path):
            path = self.planner.a_star(grid, pose_px, goal_px)
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

        next_px = self.current_path[self.path_index]
        next_world = self._grid_to_world(next_px)

        result = self.controller.step_to_goal(pose, next_world)
        if result == "REACHED":
            self.path_index += 1
        elif result == "BLOCKED":
            self.current_path = None

        return "RUNNING"

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
        while self.path_index < len(self.current_path) - 1:
            node = self.current_path[self.path_index]
            dr = pose_px[0] - node[0]
            dc = pose_px[1] - node[1]
            if (dr * dr + dc * dc) < 9:
                self.path_index += 1
            else:
                break
