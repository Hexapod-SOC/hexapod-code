import numpy as np
import logging
from unittest.mock import MagicMock

# Mock dependencies
class MockLidar:
    def __init__(self):
        self.resolution = 0.05
        self.origin = (0.0, 0.0)
        
    def update_map(self):
        # Create 100x100 grid (5m x 5m)
        grid = np.zeros((100, 100), dtype=np.int8)
        # Add walls
        grid[0, :] = 100
        grid[-1, :] = 100
        grid[:, 0] = 100
        grid[:, -1] = 100
        # Add some free space (-20)
        grid[1:99, 1:99] = -20
        # Add some unknown (0) block
        grid[40:60, 40:60] = 0
        return grid
        
    def get_pose(self):
        return {"x": 0.5, "y": 0.5, "theta": 0.0}

class MockClient:
    def move(self, fwd, strafe, rot):
        print(f"CMD: Fwd={fwd:.2f}, Rot={rot:.2f}")

# Import actual modules (adjust paths if needed or mock)
# We need to test the logic we just wrote. 
# Since we can't easily import from crates/devices... via python path
# I will paste the relevant logic classes here for the simulation test
# or I can try to import them if I set PYTHONPATH? 
# Let's try to mock the environment to run the actual files.

import sys
import os
sys.path.append(os.path.abspath("crates/devices/src/ai"))
sys.path.append(os.path.abspath("crates/devices/src/ai/navigation"))
sys.path.append(os.path.abspath("crates/devices/src/ai/behaviors"))

# Actually, let's just assume we can import IF we run from specific dir.
# But for reliability, I will mock the detailed imports in the script itself 
# if I can't run it.
# Let's try running it with python path.

from navigation.controller import MotionController
from navigation.planner import Pathfinder
from behaviors.explore import ExploreBehavior

def test_simulation():
    logging.basicConfig(level=logging.INFO)
    
    lidar = MockLidar()
    client = MockClient()
    controller = MotionController(client, lidar)
    planner = Pathfinder()
    explore = ExploreBehavior(planner, controller, lidar)
    
    print("--- Step 1: IDLE -> PLAN ---")
    status = explore.step()
    print(f"Status: {status}")
    print(f"State: {explore.state}")
    if explore.state == "FOLLOWING":
        print(f"Path len: {len(explore.current_path)}")
        
    print("\n--- Step 2: FOLLOWING ---")
    # Simulation loop
    for i in range(10):
        status = explore.step()
        # Mock movement (update mocked pose based on command?)
        # For this test, we just want to see non-zero commands.
        
if __name__ == "__main__":
    test_simulation()
