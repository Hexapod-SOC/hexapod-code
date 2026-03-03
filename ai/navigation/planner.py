import numpy as np
import heapq
import math
import logging
from typing import List, Tuple, Optional

# Log-odds map thresholds
# Map uses i8, initialized to 0 (unknown):
#   Negative (<= -10) = Free
#   0 = Unknown (not yet observed)
#   Positive (>= 10) = Occupied
FREE_THRESH = -10
OBSTACLE_THRESH = 10


class Pathfinder:
    def __init__(self):
        self._inflation_offsets = {}

    def heuristic(self, a, b):
        return math.sqrt((b[0] - a[0]) ** 2 + (b[1] - a[1]) ** 2)

    def get_neighbors(
        self,
        grid: np.ndarray,
        node: tuple,
        blocked: Optional[np.ndarray] = None,
        allow_unknown: bool = True,
        unknown_penalty: float = 2.0,
    ):
        rows, cols = grid.shape
        r, c = node
        # 8-connectivity
        moves = [
            (0, 1), (0, -1), (1, 0), (-1, 0),
            (1, 1), (1, -1), (-1, 1), (-1, -1)
        ]
        neighbors = []
        for dr, dc in moves:
            nr, nc = r + dr, c + dc
            if 0 <= nr < rows and 0 <= nc < cols:
                if blocked is not None and blocked[nr, nc]:
                    continue
                val = grid[nr, nc]
                if val >= OBSTACLE_THRESH:
                    continue
                if not allow_unknown and val >= -5:
                    continue
                cost = math.sqrt(dr * dr + dc * dc)
                if val >= -5:  # Unknown or barely explored
                    cost *= unknown_penalty
                neighbors.append(((nr, nc), cost))
        return neighbors

    def a_star(
        self,
        grid: np.ndarray,
        start: tuple,
        goal: tuple,
        allow_unknown: bool = True,
        unknown_penalty: float = 2.0,
        inflation_radius_px: int = 0,
        smooth: bool = False,
    ) -> Optional[List[Tuple]]:
        """Standard A* implementation using log-odds map."""
        rows, cols = grid.shape

        if not (0 <= start[0] < rows and 0 <= start[1] < cols):
            logging.warning(f"Planner: start {start} out of bounds {rows}x{cols}")
            return None
        if not (0 <= goal[0] < rows and 0 <= goal[1] < cols):
            logging.warning(f"Planner: goal {goal} out of bounds {rows}x{cols}")
            return None

        blocked = self._inflate_obstacles(grid, inflation_radius_px)

        if blocked is None:
            blocked = grid >= OBSTACLE_THRESH

        if blocked[start]:
            logging.warning("Planner: start is in obstacle after inflation")
            return None

        # If goal is occupied, find nearest traversable cell
        if blocked[goal]:
            found = False
            for r_search in range(1, 10):
                for dr in range(-r_search, r_search + 1):
                    for dc in range(-r_search, r_search + 1):
                        nr, nc = goal[0] + dr, goal[1] + dc
                        if 0 <= nr < rows and 0 <= nc < cols and not blocked[nr, nc]:
                            goal = (nr, nc)
                            found = True
                            break
                    if found:
                        break
                if found:
                    break
            if not found:
                logging.warning("Planner: goal is occupied with no traversable neighbor")
                return None

        frontier = []
        heapq.heappush(frontier, (0, start))
        came_from = {start: None}
        cost_so_far = {start: 0}

        iterations = 0
        max_iterations = 50000  # Prevent infinite loops on large grids

        while frontier and iterations < max_iterations:
            iterations += 1
            current = heapq.heappop(frontier)[1]

            if current == goal:
                break

            for next_node, move_cost in self.get_neighbors(
                grid,
                current,
                blocked=blocked,
                allow_unknown=allow_unknown,
                unknown_penalty=unknown_penalty,
            ):
                new_cost = cost_so_far[current] + move_cost
                if next_node not in cost_so_far or new_cost < cost_so_far[next_node]:
                    cost_so_far[next_node] = new_cost
                    priority = new_cost + self.heuristic(goal, next_node)
                    heapq.heappush(frontier, (priority, next_node))
                    came_from[next_node] = current

        if goal not in came_from:
            logging.warning(f"Planner: no path to {goal} from {start} (iterations={iterations})")
            return None

        # Reconstruct path
        path = []
        current = goal
        while current is not None:
            path.append(current)
            current = came_from[current]
        path.reverse()
        logging.info(f"Planner: path found, length={len(path)}")
        if smooth:
            path = self.smooth_path(path, blocked)
        return path

    def smooth_path(self, path: List[Tuple[int, int]], blocked: np.ndarray) -> List[Tuple[int, int]]:
        if not path or len(path) < 3:
            return path
        smoothed = [path[0]]
        i = 0
        last_index = len(path) - 1
        while i < last_index:
            j = last_index
            while j > i + 1:
                if self._line_of_sight(blocked, path[i], path[j]):
                    break
                j -= 1
            smoothed.append(path[j])
            i = j
        return smoothed

    def _line_of_sight(self, blocked: np.ndarray, a: Tuple[int, int], b: Tuple[int, int]) -> bool:
        r0, c0 = a
        r1, c1 = b
        dr = abs(r1 - r0)
        dc = abs(c1 - c0)
        r_step = 1 if r1 >= r0 else -1
        c_step = 1 if c1 >= c0 else -1
        err = dr - dc
        r, c = r0, c0
        while True:
            if blocked[r, c]:
                return False
            if (r, c) == (r1, c1):
                return True
            e2 = 2 * err
            if e2 > -dc:
                err -= dc
                r += r_step
            if e2 < dr:
                err += dr
                c += c_step

    def get_blocked_map(self, grid: np.ndarray, inflation_radius_px: int = 0) -> np.ndarray:
        blocked = self._inflate_obstacles(grid, inflation_radius_px)
        if blocked is None:
            blocked = grid >= OBSTACLE_THRESH
        return blocked

    def _inflate_obstacles(self, grid: np.ndarray, radius_px: int) -> Optional[np.ndarray]:
        if radius_px <= 0:
            return None
        occupied = grid >= OBSTACLE_THRESH
        offsets = self._get_inflation_offsets(radius_px)
        if not offsets:
            return occupied
        rows, cols = grid.shape
        inflated = occupied.copy()
        for dr, dc in offsets:
            if dr == 0 and dc == 0:
                continue
            r_src_start = max(0, -dr)
            r_src_end = rows - max(0, dr)
            c_src_start = max(0, -dc)
            c_src_end = cols - max(0, dc)
            r_dst_start = max(0, dr)
            r_dst_end = rows - max(0, -dr)
            c_dst_start = max(0, dc)
            c_dst_end = cols - max(0, -dc)
            inflated[r_dst_start:r_dst_end, c_dst_start:c_dst_end] |= occupied[
                r_src_start:r_src_end, c_src_start:c_src_end
            ]
        return inflated

    def _get_inflation_offsets(self, radius_px: int) -> List[Tuple[int, int]]:
        cached = self._inflation_offsets.get(radius_px)
        if cached is not None:
            return cached
        offsets = []
        r = int(radius_px)
        for dr in range(-r, r + 1):
            for dc in range(-r, r + 1):
                if dr * dr + dc * dc <= r * r:
                    offsets.append((dr, dc))
        self._inflation_offsets[radius_px] = offsets
        return offsets
