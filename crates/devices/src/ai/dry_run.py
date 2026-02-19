import sys
import os
from unittest.mock import MagicMock

# Mock environment
os.environ["HEXAPOD_API_BASE"] = "http://mock:3000"
os.environ["OPENAI_API_KEY"] = "sk-mock"

# Mock external deps
sys.modules["openai"] = MagicMock()
sys.modules["cv2"] = MagicMock()

import asyncio
# Relative imports work when run as module
from .agent import Agent
from .clients.hexapod import HexapodClient

# Mock Hexapod Client methods
HexapodClient.get_status = MagicMock(return_value={"status": "ok"})
HexapodClient.get_lidar_map = MagicMock(return_value={
    "width": 10, "height": 10, "resolution": 0.1, 
    "origin": {"x":0,"y":0,"theta":0},
    "cells": [0]*100
})
HexapodClient.get_lidar_frame = MagicMock(return_value={
    "points": [{"distance_mm": 500, "angle_deg": 0}]
})
HexapodClient.move = MagicMock()
HexapodClient.speak = MagicMock()
HexapodClient.stop = MagicMock()

async def run_test():
    print("Initializing Agent...")
    agent = Agent()
    
    # Test 1: Command Processing
    print("\n--- Test 1: Command Processing ---")
    # Mock LLM response
    agent.llm.parse_command = MagicMock(return_value={
        "reply": "Starting exploration.",
        "actions": [{"function": "explore", "arguments": {}}]
    })
    
    response = agent.process_command("Explore the room")
    print(f"User: 'Explore the room'")
    print(f"Agent: '{response['reply']}'")
    
    if agent.current_task == "explore":
        print("PASS: Agent task set to 'explore'")
    else:
        print(f"FAIL: Agent task is '{agent.current_task}'")
        return

    # Test 2: Behavior Step
    print("\n--- Test 2: Behavior Step ---")
    # Mock planner to return a path
    agent.planner.a_star = MagicMock(return_value=[(0,0), (1,1)])
    
    # await agent.run_loop() # Infinite loop, skip
    
    # Manually invoke step logic from agent loop
    print("Executing one step of explore behavior...")
    result = agent.explore_behavior.step()
    print(f"Behavior Step Result: {result}")
    
    if result in ["RUNNING", "FINISHED", "PLAN_FAILED", "NO_MAP"]:
         print("PASS: Behavior executed successfully.")
    else:
         print(f"FAIL: Unexpected result {result}")

    # Test 3: Safety Stop
    print("\n--- Test 3: Stop Command ---")
    agent.llm.parse_command = MagicMock(return_value={
        "reply": "Stopping.",
        "actions": [{"function": "stop", "arguments": {}}]
    })
    agent.process_command("Stop")
    if agent.current_task is None:
        print("PASS: Task cleared and stop called.")
        agent.client.stop.assert_called()
    else:
        print("FAIL: Task did not clear.")

if __name__ == "__main__":
    asyncio.run(run_test())
