import numpy as np
import heapq
import math
from typing import List, Tuple, Optional

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
                # 0 is free, 100 is occupied, -1 is unknown (treat as obstacle for planning?)
                # We'll treat unknown as free for exploration but costly?
                if grid[nr, nc] < 50 and grid[nr, nc] != -1: 
                    cost = math.sqrt(dr*dr + dc*dc)
                    neighbors.append(((nr, nc), cost))
        return neighbors

    def a_star(self, grid: np.ndarray, start: tuple, goal: tuple):
        """Standard A* implementation."""
        # Check bounds
        rows, cols = grid.shape
        if not (0 <= start[0] < rows and 0 <= start[1] < cols):
            return None
        if not (0 <= goal[0] < rows and 0 <= goal[1] < cols):
            return None
        
        # If goal is occupied, find nearest free cell
        if grid[goal] > 50:
            # Simple spiral search for free neighbor
            found = False
            for r in range(1, 10):
                for dr in range(-r, r+1):
                    for dc in range(-r, r+1):
                        nr, nc = goal[0] + dr, goal[1] + dc
                        if 0 <= nr < rows and 0 <= nc < cols and grid[nr, nc] < 50:
                            goal = (nr, nc)
                            found = True
                            break
                    if found: break
                if found: break
            if not found:
                return None

        frontier = []
        heapq.heappush(frontier, (0, start))
        came_from = {start: None}
        cost_so_far = {start: 0}

        while frontier:
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
            return None

        # Reconstruct path
        path = []
        current = goal
        while current != start:
            path.append(current)
            current = came_from[current]
        path.append(start)
        path.reverse()
        return path
