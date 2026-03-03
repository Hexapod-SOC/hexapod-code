"""
ExploreBehavior – frontier-based map exploration.

Architecture
------------
The class is meant to be called every ~0.5s from the agent run_loop.
On each call it:
  1. Fetches the SLAM map and current pose.
  2. Finds frontier cells (free cells adjacent to unknown cells).
  3. Picks the nearest un-blacklisted frontier as the current goal.
  4. Uses A* to plan a path.
  5. Delegates each step to MotionController.step_to_goal().

The controller sends a single velocity command per call.  The run_loop
must call step() frequently enough to keep the robot moving smoothly
(every 0.2 – 0.5 s is fine).

Map format
----------
The SLAM map uses log-odds i8 values:
  <= -10  →  Free
     0    →  Unknown (initial state)
  >= +10  →  Occupied
"""

from typing import List, Optional, Tuple
import logging
import numpy as np
from config import settings

# Log-odds thresholds (must match planner.py)
FREE_THRESH    = -10
OBSTACLE_THRESH = 10
UNKNOWN_MAX    =   5   # anything in (-5, 5) is "essentially unknown"


class ExploreBehavior:
    def __init__(self, planner, controller, lidar):
        self.planner    = planner
        self.controller = controller
        self.lidar      = lidar

        # Exploration memory
        self.frontier_blacklist: set = set()
        self.current_goal: Optional[Tuple[int, int]] = None
        self.current_path: Optional[List[Tuple[int, int]]] = None
        self.path_index: int = 0
        self._no_frontier_count: int = 0
        self._map_frame: Optional[int] = None

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def reset(self):
        """Clear all exploration state (call when task is restarted)."""
        self.frontier_blacklist.clear()
        self.current_goal = None
        self.current_path = None
        self.path_index   = 0
        self._no_frontier_count = 0
        self._map_frame = None

    def step(self) -> str:
        """
        One control step.  Returns a status string:
          'RUNNING'      – moving toward a frontier
          'REPLANNING'   – goal reached or blocked, selecting new frontier
          'SCAN'         – no frontiers visible yet, caller should rotate
          'FINISHED'     – fully explored (no frontiers + rotated full circle)
          'NO_MAP'       – SLAM map unavailable
          'NO_POSE'      – pose unavailable
        """
        # ── 1. Get map ────────────────────────────────────────────────
        grid = self.lidar.update_map()
        if grid is None:
            logging.warning("Explore: no map available")
            return "NO_MAP"

        # ── 2. Get pose ───────────────────────────────────────────────
        pose = self.lidar.get_pose()
        if not pose:
            logging.warning("Explore: no pose available")
            return "NO_POSE"

        if self._map_frame is None:
            self._map_frame = self.lidar.map_frame
        elif self._map_frame != self.lidar.map_frame:
            self._map_frame = self.lidar.map_frame
            self.current_goal = None
            self.current_path = None
            self.path_index = 0

        pose_px = self._world_to_grid(pose)
        logging.debug(f"Explore: pose_world=({pose['x']:.3f},{pose['y']:.3f}) pose_px={pose_px}")

        # ── 3. Check if current goal is still valid ───────────────────
        if self.current_goal is not None:
            # Accept goal if within 5px (about one resolution step)
            dr = pose_px[0] - self.current_goal[0]
            dc = pose_px[1] - self.current_goal[1]
            dist_px = (dr * dr + dc * dc) ** 0.5
            if dist_px < 5:
                logging.info(f"Explore: reached goal {self.current_goal}, replanning")
                self.current_goal = None
                self.current_path = None
                self.path_index   = 0

        # ── 4. Select / replan if we have no path ─────────────────────
        if self.current_path is None or self.path_index >= len(self.current_path):
            status = self._select_and_plan(grid, pose_px)
            if status != "OK":
                return status

        # ── 5. Advance path index to skip already-visited nodes ───────
        self._advance_path_index(pose_px)

        if self.path_index >= len(self.current_path):
            logging.info("Explore: path exhausted, replanning")
            self.current_path = None
            return "REPLANNING"

        # ── 6. Get next waypoint, convert to world coords ─────────────
        next_px = self.current_path[self.path_index]
        next_world = self._grid_to_world(next_px)

        logging.info(
            f"Explore: → waypoint {self.path_index}/{len(self.current_path)} "
            f"px={next_px} world=({next_world[0]:.3f},{next_world[1]:.3f})"
        )

        # ── 7. Send velocity command ───────────────────────────────────
        result = self.controller.step_to_goal(pose, next_world)
        if result == "REACHED":
            self.path_index += 1
        elif result == "BLOCKED":
            # Obstacle - blacklist current goal and replan
            if self.current_goal:
                self.frontier_blacklist.add(self.current_goal)
            self.current_goal = None
            self.current_path = None

        return "RUNNING"

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _select_and_plan(self, grid: np.ndarray, pose_px: Tuple[int, int]) -> str:
        """Find frontiers, pick best, plan path.  Returns 'OK' or status code."""
        frontiers = self.get_frontiers(grid)
        logging.info(f"Explore: {len(frontiers)} frontiers found")

        if not frontiers:
            self._no_frontier_count += 1
            if self._no_frontier_count > 10:
                logging.info("Explore: FINISHED (no frontiers after many attempts)")
                return "FINISHED"
            return "SCAN"

        self._no_frontier_count = 0
        goal = self.select_best_frontier(pose_px, frontiers)
        if goal is None:
            logging.info("Explore: all frontiers blacklisted")
            self.frontier_blacklist.clear()  # Reset to allow re-exploration
            return "SCAN"

        path = self.planner.a_star(
            grid,
            pose_px,
            goal,
            allow_unknown=True,
            unknown_penalty=2.0,
            inflation_radius_px=self._inflation_radius_px(),
            smooth=True,
        )
        if path is None or len(path) < 2:
            logging.warning(f"Explore: A* failed to {goal}, blacklisting")
            self.frontier_blacklist.add(goal)
            return "REPLANNING"

        self.current_goal = goal
        self.current_path = path
        self.path_index   = 1  # Skip [0] which is start (current pos)
        logging.info(f"Explore: new plan to {goal}, path length={len(path)}")
        return "OK"

    def _advance_path_index(self, pose_px: Tuple[int, int]):
        """Skip path nodes we've already passed (lookahead)."""
        if self.current_path is None:
            return
        # Find the furthest path node within 3px of our current position
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

    def _inflation_radius_px(self) -> int:
        inflation_m = (settings.ROBOT_RADIUS + settings.SAFETY_DISTANCE) / 1000.0
        if self.lidar.resolution <= 0:
            return 0
        return max(1, int(np.ceil(inflation_m / self.lidar.resolution)))

    def _world_to_grid(self, pose: dict) -> Tuple[int, int]:
        origin_x = self.lidar.origin[0]
        origin_y = self.lidar.origin[1]
        res = self.lidar.resolution
        px_x = int((pose['x'] - origin_x) / res)
        px_y = int((pose['y'] - origin_y) / res)
        return (px_y, px_x)  # (row, col) = (y, x)

    def _grid_to_world(self, px: Tuple[int, int]) -> Tuple[float, float]:
        origin_x = self.lidar.origin[0]
        origin_y = self.lidar.origin[1]
        res = self.lidar.resolution
        world_x = px[1] * res + origin_x
        world_y = px[0] * res + origin_y
        return (world_x, world_y)

    # ------------------------------------------------------------------
    # Frontier detection
    # ------------------------------------------------------------------

    def get_frontiers(self, grid: np.ndarray) -> List[Tuple[int, int]]:
        """
        Return list of (row, col) frontier cells.
        A frontier is a FREE cell (< FREE_THRESH) adjacent to an UNKNOWN
        cell (value in [-5, 5]).
        """
        rows, cols = grid.shape
        frontiers = []

        for r in range(1, rows - 1):
            for c in range(1, cols - 1):
                if grid[r, c] < FREE_THRESH:  # Free cell
                    for dr, dc in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                        n = grid[r + dr, c + dc]
                        if -UNKNOWN_MAX <= n <= UNKNOWN_MAX:  # Unknown cell
                            frontiers.append((r, c))
                            break
        return frontiers

    def select_best_frontier(
        self,
        pose_px: Tuple[int, int],
        frontiers: List[Tuple[int, int]],
    ) -> Optional[Tuple[int, int]]:
        """Return the nearest non-blacklisted frontier."""
        best = None
        best_dist = float("inf")
        for f in frontiers:
            if f in self.frontier_blacklist:
                continue
            d = (f[0] - pose_px[0]) ** 2 + (f[1] - pose_px[1]) ** 2
            if d < best_dist:
                best_dist = d
                best = f
        return best
