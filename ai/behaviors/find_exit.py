from typing import List, Tuple, Optional
import logging
import math
import time
import threading
import numpy as np
from config import settings

# Log-odds thresholds (must match planner.py)
FREE_THRESH = -10
UNKNOWN_MAX = 5

class ExitFinder:
    def __init__(self, planner, controller, lidar, camera=None, llm=None):
        self.planner = planner
        self.controller = controller
        self.lidar = lidar
        self.camera = camera
        self.llm = llm

        self.goal: Optional[Tuple[int, int]] = None
        self.path: Optional[List[Tuple[int, int]]] = None
        self.path_index: int = 0
        self.blacklist: set = set()
        self._roam_goal: Optional[Tuple[int, int]] = None
        self._escape_goal: Optional[Tuple[int, int]] = None
        self._map_frame: Optional[int] = None
        self._goal_kind: Optional[str] = None
        self._bias_points: List[Tuple[float, float]] = []

        self._edge_bias_px = settings.EXIT_EDGE_BIAS_PX
        self._edge_weight = settings.EXIT_EDGE_WEIGHT
        self._dist_weight = 1.0
        self._max_goal_tries = 8
        self._roam_radius_m = 2.5
        self._roam_min_radius_m = 0.6
        self._vision_hint: Optional[str] = None
        self._vision_hint_ts: float = 0.0
        self._vision_pending: bool = False
        self._vision_last_attempt_ts: float = 0.0
        self._vision_last_attempt_ts: float = 0.0
        self._fail_count: int = 0
        self._fail_reset: int = 12
        self._last_plan_ts: float = 0.0
        self._last_scan_ts: float = 0.0
        self._roam_edge_margin_px: int = settings.EXIT_ROAM_EDGE_MARGIN_PX

    def reset(self):
        self.goal = None
        self.path = None
        self.path_index = 0
        self.blacklist.clear()
        self._roam_goal = None
        self._escape_goal = None
        self._map_frame = None
        self._goal_kind = None
        self._fail_count = 0
        self._last_plan_ts = 0.0
        self._last_scan_ts = 0.0
        self._bias_points = []

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
            self._last_plan_ts = 0.0

        now = time.monotonic()
        if self._last_plan_ts and (now - self._last_plan_ts) > settings.EXIT_REPLAN_INTERVAL_S:
            self.goal = None
            self.path = None
            self.path_index = 0
            self._last_plan_ts = 0.0

        pose_px = self._world_to_grid(pose)
        inflation_px = self._safe_inflation_px(grid, pose_px)
        blocked = self.planner.get_blocked_map(grid, inflation_px)
        if blocked is not None and blocked[pose_px]:
            nudged = self._nearest_free_cell(blocked, pose_px, radius_px=max(3, inflation_px))
            if nudged is not None:
                logging.warning("Exit: start blocked, nudging pose from %s to %s", pose_px, nudged)
                pose_px = nudged

        if self.goal is not None:
            dr = pose_px[0] - self.goal[0]
            dc = pose_px[1] - self.goal[1]
            if (dr * dr + dc * dc) ** 0.5 < 5:
                logging.info(f"Exit: reached goal {self.goal}, replanning")
                self.goal = None
                self.path = None
                self.path_index = 0

        if self.path is None or self.path_index >= len(self.path):
            status = self._select_and_plan(grid, pose, pose_px)
            if status != "OK":
                return status

        self._advance_path_index(pose_px)

        if self.path_index >= len(self.path):
            self.path = None
            return "REPLANNING"

        next_px = self.path[self.path_index]
        next_world = self._grid_to_world(next_px)
        logging.info(
            "Exit: next waypoint px=%s world=(%.2f, %.2f) goal_kind=%s",
            next_px,
            next_world[0],
            next_world[1],
            self._goal_kind,
        )
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

    def _select_and_plan(self, grid: np.ndarray, pose: dict, pose_px: Tuple[int, int]) -> str:
        frontiers = self._get_frontiers(grid)
        logging.info("Exit: %d frontiers detected", len(frontiers))
        if not frontiers:
            logging.info("Exit: no frontiers; roaming")
            return self._plan_roam(grid, pose_px)

        candidates = self._rank_exit_frontiers(grid, pose, pose_px, frontiers)
        logging.info("Exit: %d candidates after ranking", len(candidates))
        if not candidates:
            self.blacklist.clear()
            now = time.monotonic()
            if now - self._last_scan_ts >= settings.EXIT_SCAN_INTERVAL_S:
                self._last_scan_ts = now
                return "SCAN"
            return self._plan_roam(grid, pose_px)

        inflation_px = self._safe_inflation_px(grid, pose_px)
        for goal in candidates[: self._max_goal_tries]:
            path = self.planner.a_star(
                grid,
                pose_px,
                goal,
                allow_unknown=settings.EXIT_ALLOW_UNKNOWN,
                unknown_penalty=2.0,
                inflation_radius_px=inflation_px,
                smooth=True,
            )
            if (path is None or len(path) < 2) and not settings.EXIT_ALLOW_UNKNOWN:
                path = self.planner.a_star(
                    grid,
                    pose_px,
                    goal,
                    allow_unknown=True,
                    unknown_penalty=1.2,
                    inflation_radius_px=inflation_px,
                    smooth=True,
                )
            if path is None or len(path) < 2:
                goal_world = self._grid_to_world(goal)
                logging.warning(
                    "Exit: A* failed to %s (world=%.2f, %.2f), blacklisting",
                    goal,
                    goal_world[0],
                    goal_world[1],
                )
                self.blacklist.add(goal)
                self._fail_count += 1
                if self._fail_count >= self._fail_reset:
                    logging.warning("Exit: too many failures, resetting blacklist and roaming")
                    self.blacklist.clear()
                    self._fail_count = 0
                    return self._plan_roam(grid, pose_px)
                continue

            self.goal = goal
            self.path = path
            self.path_index = 1
            self._goal_kind = "frontier"
            self._fail_count = 0
            self._last_plan_ts = time.monotonic()
            logging.info(f"Exit: new plan to {goal}, path length={len(path)}")
            return "OK"

        return self._plan_roam(grid, pose_px)

    def _plan_roam(self, grid: np.ndarray, pose_px: Tuple[int, int]) -> str:
        roam_goal = self._pick_roam_goal(grid, pose_px)
        if roam_goal is None:
            return "REPLANNING"

        inflation_px = self._safe_inflation_px(grid, pose_px)
        path = self.planner.a_star(
            grid,
            pose_px,
            roam_goal,
            allow_unknown=settings.EXIT_ALLOW_UNKNOWN,
            unknown_penalty=2.0,
                inflation_radius_px=inflation_px,
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
        self._goal_kind = "roam"
        self._last_plan_ts = time.monotonic()
        logging.info(f"Exit: roaming to {roam_goal}, path length={len(path)}")
        return "OK"

    def _plan_escape(self, grid: np.ndarray, pose: dict) -> str:
        escape_goal = self._pick_escape_cell(grid, pose)
        if escape_goal is None:
            return "SCAN"

        pose_px = self._world_to_grid(pose)
        inflation_px = self._safe_inflation_px(grid, pose_px)
        path = self.planner.a_star(
            grid,
            pose_px,
            escape_goal,
            allow_unknown=settings.EXIT_ALLOW_UNKNOWN,
            unknown_penalty=2.0,
            inflation_radius_px=inflation_px,
            smooth=True,
        )
        if path is None or len(path) < 2:
            return "REPLANNING"

        self.goal = escape_goal
        self._escape_goal = escape_goal
        self.path = path
        self.path_index = 1
        self._goal_kind = "escape"
        self._last_plan_ts = time.monotonic()
        logging.info(f"Exit: escape to {escape_goal}, path length={len(path)}")
        return "BLOCKED"

    def get_goal_world(self) -> Optional[dict]:
        if not self.goal:
            return None
        world = self._grid_to_world(self.goal)
        return {
            "x": world[0],
            "y": world[1],
            "kind": self._goal_kind or "frontier",
        }

    def get_debug(self) -> dict:
        return {
            "goal": self.goal,
            "goal_kind": self._goal_kind,
            "path_len": len(self.path) if self.path else 0,
            "path_index": self.path_index,
            "blacklist_size": len(self.blacklist),
            "fail_count": self._fail_count,
            "last_plan_ts": self._last_plan_ts,
            "last_scan_ts": self._last_scan_ts,
        }

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
        pose: dict,
        pose_px: Tuple[int, int],
        frontiers: List[Tuple[int, int]],
    ) -> List[Tuple[int, int]]:
        rows, cols = grid.shape
        inflation_px = self._inflation_radius_px()
        blocked = self.planner.get_blocked_map(grid, inflation_px)
        vision_hint = self._maybe_get_vision_hint()
        scored = []
        for f in frontiers:
            if f in self.blacklist:
                continue
            goal = f
            if blocked[f]:
                fallback = self._nearest_free_cell(blocked, f, radius_px=max(2, inflation_px))
                if fallback is None:
                    continue
                goal = fallback
            bias_bonus = 0.0
            if self._bias_points:
                goal_world = self._grid_to_world(goal)
                min_dist = min(
                    math.hypot(goal_world[0] - bx, goal_world[1] - by)
                    for bx, by in self._bias_points
                )
                if min_dist <= settings.EXIT_BIAS_MAX_DIST_M:
                    bias_bonus = (settings.EXIT_BIAS_MAX_DIST_M - min_dist) * settings.EXIT_BIAS_WEIGHT
            unknown_ratio = self._unknown_ratio_around(grid, f, radius_px=6)
            dist = math.sqrt((f[0] - pose_px[0]) ** 2 + (f[1] - pose_px[1]) ** 2)
            edge_dist = min(f[0], f[1], rows - 1 - f[0], cols - 1 - f[1])
            edge_score = max(0, self._edge_bias_px - edge_dist)
            score = (edge_score * self._edge_weight) - (dist * self._dist_weight)
            score += unknown_ratio * settings.EXIT_UNKNOWN_BONUS
            score += bias_bonus

            if vision_hint:
                direction = self._direction_to_frontier(pose, f)
                if direction == vision_hint:
                    score += settings.EXIT_VISION_BONUS
            scored.append((score, goal))
        scored.sort(reverse=True, key=lambda x: x[0])
        return [f for _, f in scored]

    def _unknown_ratio_around(self, grid: np.ndarray, cell: Tuple[int, int], radius_px: int = 5) -> float:
        r0, c0 = cell
        rows, cols = grid.shape
        r_min = max(0, r0 - radius_px)
        r_max = min(rows - 1, r0 + radius_px)
        c_min = max(0, c0 - radius_px)
        c_max = min(cols - 1, c0 + radius_px)
        window = grid[r_min : r_max + 1, c_min : c_max + 1]
        if window.size == 0:
            return 0.0
        unknown = (window >= -UNKNOWN_MAX) & (window <= UNKNOWN_MAX)
        return float(np.count_nonzero(unknown)) / float(window.size)

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

    def set_bias_points(self, points: List[Tuple[float, float]]) -> None:
        self._bias_points = list(points)

    def _safe_inflation_px(self, grid: np.ndarray, pose_px: Tuple[int, int]) -> int:
        inflation_px = self._inflation_radius_px()
        if inflation_px <= 0:
            return 0
        while inflation_px > 0:
            blocked = self.planner.get_blocked_map(grid, inflation_px)
            if not blocked[pose_px]:
                return inflation_px
            logging.warning("Exit: start in obstacle with inflation=%d, reducing", inflation_px)
            inflation_px -= 1
        return 0

    def _maybe_get_vision_hint(self) -> Optional[str]:
        if not settings.EXIT_USE_VISION:
            return None
        if not self.camera or not self.camera.available:
            return None
        if not self.llm:
            return None

        now = time.monotonic()
        if self._vision_last_attempt_ts and (now - self._vision_last_attempt_ts) < settings.EXIT_VISION_MIN_INTERVAL_S:
            return self._vision_hint
        if self._vision_hint and (now - self._vision_hint_ts) < settings.EXIT_VISION_REFRESH_S:
            return self._vision_hint

        if not self._vision_pending:
            self._vision_pending = True
            threading.Thread(target=self._update_vision_hint, daemon=True).start()

        return self._vision_hint

    def _update_vision_hint(self) -> None:
        try:
            self._vision_last_attempt_ts = time.monotonic()
            image_bytes = self.camera.get_jpeg_bytes(settings.CAMERA_JPEG_QUALITY)
            if not image_bytes:
                return

            response = self.llm.describe_image(image_bytes, settings.EXIT_VISION_PROMPT)
            hint = self._parse_direction_hint(response)
            if hint:
                self._vision_hint = hint
                self._vision_hint_ts = time.monotonic()
                logging.info(f"Exit vision hint: {hint}")
        finally:
            self._vision_pending = False

    def _parse_direction_hint(self, text: str) -> Optional[str]:
        if not text:
            return None
        lowered = text.lower()
        for token in ("forward", "left", "right", "back", "backward"):
            if token in lowered:
                return "back" if token == "backward" else token
        return None

    def _direction_to_frontier(self, pose: dict, frontier_px: Tuple[int, int]) -> Optional[str]:
        world = self._grid_to_world(frontier_px)
        dx = world[0] - pose.get("x", 0.0)
        dy = world[1] - pose.get("y", 0.0)
        if abs(dx) < 1e-6 and abs(dy) < 1e-6:
            return None
        heading = pose.get("theta", 0.0)
        angle = math.atan2(dy, dx) - heading
        while angle > math.pi:
            angle -= 2 * math.pi
        while angle < -math.pi:
            angle += 2 * math.pi

        if -math.pi / 4 <= angle <= math.pi / 4:
            return "forward"
        if math.pi / 4 < angle < 3 * math.pi / 4:
            return "left"
        if -3 * math.pi / 4 < angle < -math.pi / 4:
            return "right"
        return "back"

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
            edge_dist = min(rr, cc, rows - 1 - rr, cols - 1 - cc)
            if edge_dist < self._roam_edge_margin_px:
                continue
            if blocked[rr, cc]:
                continue
            if (rr, cc) in self.blacklist:
                continue

            cell = grid[rr, cc]
            free_bonus = 6.0 if cell < FREE_THRESH else 0.0
            unknown_ratio = self._unknown_ratio_around(grid, (rr, cc), radius_px=6)
            dist = math.sqrt((rr - r0) ** 2 + (cc - c0) ** 2)
            edge_score = max(0, self._edge_bias_px - edge_dist)
            score = (edge_score * self._edge_weight) + (dist * 0.5) + free_bonus
            score += unknown_ratio * settings.EXIT_UNKNOWN_BONUS
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
