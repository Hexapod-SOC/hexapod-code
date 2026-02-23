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
        pass

    def heuristic(self, a, b):
        return math.sqrt((b[0] - a[0]) ** 2 + (b[1] - a[1]) ** 2)

    def get_neighbors(self, grid, node):
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
                val = grid[nr, nc]
                if val < OBSTACLE_THRESH:  # Not occupied
                    cost = math.sqrt(dr * dr + dc * dc)
                    # Prefer known-free space over unknown
                    if val >= -5:  # Unknown or barely explored
                        cost *= 2.0
                    neighbors.append(((nr, nc), cost))
        return neighbors

    def a_star(self, grid: np.ndarray, start: tuple, goal: tuple) -> Optional[List[Tuple]]:
        """Standard A* implementation using log-odds map."""
        rows, cols = grid.shape

        if not (0 <= start[0] < rows and 0 <= start[1] < cols):
            logging.warning(f"Planner: start {start} out of bounds {rows}x{cols}")
            return None
        if not (0 <= goal[0] < rows and 0 <= goal[1] < cols):
            logging.warning(f"Planner: goal {goal} out of bounds {rows}x{cols}")
            return None

        # If goal is occupied, find nearest traversable cell
        if grid[goal] >= OBSTACLE_THRESH:
            found = False
            for r_search in range(1, 10):
                for dr in range(-r_search, r_search + 1):
                    for dc in range(-r_search, r_search + 1):
                        nr, nc = goal[0] + dr, goal[1] + dc
                        if 0 <= nr < rows and 0 <= nc < cols and grid[nr, nc] < OBSTACLE_THRESH:
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

            for next_node, move_cost in self.get_neighbors(grid, current):
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
        return path
