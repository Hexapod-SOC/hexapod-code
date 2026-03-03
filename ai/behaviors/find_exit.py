from typing import List, Tuple, Optional
import logging
import math
import numpy as np
from config import settings

# Log-odds thresholds (must match planner.py)
FREE_THRESH = -10
UNKNOWN_MAX = 5

class ExitFinder:
    def __init__(self, planner, controller, lidar):
        self.planner = planner
        self.controller = controller
        self.lidar = lidar

        self.goal: Optional[Tuple[int, int]] = None
        self.path: Optional[List[Tuple[int, int]]] = None
        self.path_index: int = 0
        self.blacklist: set = set()
        self._roam_goal: Optional[Tuple[int, int]] = None
        self._map_frame: Optional[int] = None

        self._edge_bias_px = 30
        self._edge_weight = 18.0
        self._dist_weight = 1.0
        self._max_goal_tries = 8
        self._roam_radius_m = 2.5
        self._roam_min_radius_m = 0.6

    def reset(self):
        self.goal = None
        self.path = None
        self.path_index = 0
        self.blacklist.clear()
        self._roam_goal = None
        self._map_frame = None

    def step(self) -> str:
        grid = self.lidar.update_map()
        if grid is None:
            logging.warning("Exit: no map available")
            return "NO_MAP"

        pose = self.lidar.get_pose()
        if not pose:
            logging.warning("Exit: no pose available")
            return "NO_POSE"

        if self._map_frame is None:
            self._map_frame = self.lidar.map_frame
        elif self._map_frame != self.lidar.map_frame:
            self._map_frame = self.lidar.map_frame
            self.goal = None
            self.path = None
            self.path_index = 0

        pose_px = self._world_to_grid(pose)

        if self.goal is not None:
            dr = pose_px[0] - self.goal[0]
            dc = pose_px[1] - self.goal[1]
            if (dr * dr + dc * dc) ** 0.5 < 5:
                logging.info(f"Exit: reached goal {self.goal}, replanning")
                self.goal = None
                self.path = None
                self.path_index = 0

        if self.path is None or self.path_index >= len(self.path):
            status = self._select_and_plan(grid, pose_px)
            if status != "OK":
                return status

        self._advance_path_index(pose_px)

        if self.path_index >= len(self.path):
            self.path = None
            return "REPLANNING"

        next_px = self.path[self.path_index]
        next_world = self._grid_to_world(next_px)
        result = self.controller.step_to_goal(pose, next_world)
        if result == "REACHED":
            self.path_index += 1
        elif result == "BLOCKED":
            if self.goal:
                self.blacklist.add(self.goal)
            self.goal = None
            self.path = None
            escape_status = self._plan_escape(grid, pose)
            return escape_status
        return "RUNNING"

    def _select_and_plan(self, grid: np.ndarray, pose_px: Tuple[int, int]) -> str:
        frontiers = self._get_frontiers(grid)
        if not frontiers:
            logging.info("Exit: no frontiers; roaming")
            return self._plan_roam(grid, pose_px)

        candidates = self._rank_exit_frontiers(grid, pose_px, frontiers)
        if not candidates:
            self.blacklist.clear()
            return "SCAN"

        inflation_px = self._inflation_radius_px()
        for goal in candidates[: self._max_goal_tries]:
            path = self.planner.a_star(
                grid,
                pose_px,
                goal,
                allow_unknown=True,
                unknown_penalty=2.0,
                inflation_radius_px=inflation_px,
                smooth=True,
            )
            if path is None or len(path) < 2:
                logging.warning(f"Exit: A* failed to {goal}, blacklisting")
                self.blacklist.add(goal)
                continue

            self.goal = goal
            self.path = path
            self.path_index = 1
            logging.info(f"Exit: new plan to {goal}, path length={len(path)}")
            return "OK"

        return self._plan_roam(grid, pose_px)

    def _plan_roam(self, grid: np.ndarray, pose_px: Tuple[int, int]) -> str:
        roam_goal = self._pick_roam_goal(grid, pose_px)
        if roam_goal is None:
            return "REPLANNING"

        path = self.planner.a_star(
            grid,
            pose_px,
            roam_goal,
            allow_unknown=True,
            unknown_penalty=2.0,
            inflation_radius_px=self._inflation_radius_px(),
            smooth=True,
        )
        if path is None or len(path) < 2:
            if roam_goal:
                self.blacklist.add(roam_goal)
            return "REPLANNING"

        self.goal = roam_goal
        self._roam_goal = roam_goal
        self.path = path
        self.path_index = 1
        logging.info(f"Exit: roaming to {roam_goal}, path length={len(path)}")
        return "OK"

    def _plan_escape(self, grid: np.ndarray, pose: dict) -> str:
        escape_goal = self._pick_escape_cell(grid, pose)
        if escape_goal is None:
            return "SCAN"

        pose_px = self._world_to_grid(pose)
        path = self.planner.a_star(
            grid,
            pose_px,
            escape_goal,
            allow_unknown=True,
            unknown_penalty=2.0,
            inflation_radius_px=self._inflation_radius_px(),
            smooth=True,
        )
        if path is None or len(path) < 2:
            return "REPLANNING"

        self.goal = escape_goal
        self.path = path
        self.path_index = 1
        logging.info(f"Exit: escape to {escape_goal}, path length={len(path)}")
        return "BLOCKED"

    def _advance_path_index(self, pose_px: Tuple[int, int]):
        if self.path is None:
            return
        while self.path_index < len(self.path) - 1:
            node = self.path[self.path_index]
            dr = pose_px[0] - node[0]
            dc = pose_px[1] - node[1]
            if (dr * dr + dc * dc) < 9:
                self.path_index += 1
            else:
                break

    def _get_frontiers(self, grid: np.ndarray) -> List[Tuple[int, int]]:
        rows, cols = grid.shape
        frontiers = []
        for r in range(1, rows - 1):
            for c in range(1, cols - 1):
                if grid[r, c] < FREE_THRESH:
                    for dr, dc in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                        n = grid[r + dr, c + dc]
                        if -UNKNOWN_MAX <= n <= UNKNOWN_MAX:
                            frontiers.append((r, c))
                            break
        return frontiers

    def _rank_exit_frontiers(
        self,
        grid: np.ndarray,
        pose_px: Tuple[int, int],
        frontiers: List[Tuple[int, int]],
    ) -> List[Tuple[int, int]]:
        rows, cols = grid.shape
        scored = []
        for f in frontiers:
            if f in self.blacklist:
                continue
            dist = math.sqrt((f[0] - pose_px[0]) ** 2 + (f[1] - pose_px[1]) ** 2)
            edge_dist = min(f[0], f[1], rows - 1 - f[0], cols - 1 - f[1])
            edge_score = max(0, self._edge_bias_px - edge_dist)
            score = (edge_score * self._edge_weight) - (dist * self._dist_weight)
            scored.append((score, f))
        scored.sort(reverse=True, key=lambda x: x[0])
        return [f for _, f in scored]

    def _inflation_radius_px(self) -> int:
        inflation_m = (settings.ROBOT_RADIUS + settings.SAFETY_DISTANCE) / 1000.0
        if self.lidar.resolution <= 0:
            return 0
        return max(1, int(math.ceil(inflation_m / self.lidar.resolution)))

    def _pick_random_free_cell(
        self,
        grid: np.ndarray,
        pose_px: Tuple[int, int],
        radius_px: int = 60,
        max_tries: int = 60,
    ) -> Optional[Tuple[int, int]]:
        rows, cols = grid.shape
        r0, c0 = pose_px
        for _ in range(max_tries):
            dr = np.random.randint(-radius_px, radius_px + 1)
            dc = np.random.randint(-radius_px, radius_px + 1)
            rr = r0 + dr
            cc = c0 + dc
            if 1 <= rr < rows - 1 and 1 <= cc < cols - 1:
                if grid[rr, cc] < FREE_THRESH:
                    return (rr, cc)
        return None

    def _pick_roam_goal(self, grid: np.ndarray, pose_px: Tuple[int, int]) -> Optional[Tuple[int, int]]:
        rows, cols = grid.shape
        inflation_px = self._inflation_radius_px()
        blocked = self.planner.get_blocked_map(grid, inflation_px)

        res = max(self.lidar.resolution, 1e-6)
        radius_px = max(30, int(self._roam_radius_m / res))
        min_radius_px = max(10, int(self._roam_min_radius_m / res))

        r0, c0 = pose_px
        best = None
        best_score = float("-inf")
        max_tries = 120

        for _ in range(max_tries):
            angle = np.random.uniform(0.0, 2 * math.pi)
            radius = np.random.uniform(min_radius_px, radius_px)
            rr = r0 + int(math.sin(angle) * radius)
            cc = c0 + int(math.cos(angle) * radius)
            if rr <= 1 or cc <= 1 or rr >= rows - 2 or cc >= cols - 2:
                continue
            if blocked[rr, cc]:
                continue
            if (rr, cc) in self.blacklist:
                continue

            cell = grid[rr, cc]
            free_bonus = 6.0 if cell < FREE_THRESH else 0.0
            dist = math.sqrt((rr - r0) ** 2 + (cc - c0) ** 2)
            edge_dist = min(rr, cc, rows - 1 - rr, cols - 1 - cc)
            edge_score = max(0, self._edge_bias_px - edge_dist)
            score = (edge_score * self._edge_weight) + (dist * 0.5) + free_bonus
            if score > best_score:
                best_score = score
                best = (rr, cc)

        if best is not None:
            return best

        return self._pick_random_free_cell(grid, pose_px)

    def _pick_escape_cell(self, grid: np.ndarray, pose: dict) -> Optional[Tuple[int, int]]:
        # Try short moves: back, left, right, then forward
        offsets = [(-0.35, 0.0), (0.0, 0.35), (0.0, -0.35), (0.35, 0.0)]
        for dx, dy in offsets:
            target_world = self._local_offset_to_world(pose, dx, dy)
            target_px = self._world_xy_to_grid(target_world)
            if target_px and self._is_free_cell(grid, target_px):
                return target_px
        return self._pick_random_free_cell(grid, self._world_to_grid(pose), radius_px=20, max_tries=40)

    def _local_offset_to_world(self, pose: dict, forward_m: float, left_m: float) -> Tuple[float, float]:
        x = pose.get("x", 0.0)
        y = pose.get("y", 0.0)
        theta = pose.get("theta", 0.0)
        dx = forward_m * np.cos(theta) - left_m * np.sin(theta)
        dy = forward_m * np.sin(theta) + left_m * np.cos(theta)
        return (x + dx, y + dy)

    def _world_xy_to_grid(self, world_xy: Tuple[float, float]) -> Optional[Tuple[int, int]]:
        origin_x = self.lidar.origin[0]
        origin_y = self.lidar.origin[1]
        res = self.lidar.resolution
        px_x = int((world_xy[0] - origin_x) / res)
        px_y = int((world_xy[1] - origin_y) / res)
        if px_x < 0 or px_y < 0 or px_x >= self.lidar.width or px_y >= self.lidar.height:
            return None
        return (px_y, px_x)

    def _is_free_cell(self, grid: np.ndarray, cell: Tuple[int, int]) -> bool:
        r, c = cell
        if r < 0 or c < 0 or r >= grid.shape[0] or c >= grid.shape[1]:
            return False
        return grid[r, c] < FREE_THRESH

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
