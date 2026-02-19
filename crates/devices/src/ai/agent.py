import asyncio
import logging
from typing import Dict, Any, Optional

from config import settings
from clients.hexapod import HexapodClient
from sensors.lidar import LidarSensor
from navigation.planner import Pathfinder
from navigation.controller import MotionController
from behaviors.explore import ExploreBehavior
from behaviors.find_exit import ExitFinder
from evaluator.llm import LLMClient

class Agent:
    def __init__(self):
        self.client = HexapodClient()
        self.lidar = LidarSensor(self.client)
        self.planner = Pathfinder()
        self.controller = MotionController(self.client, self.lidar)
        self.llm = LLMClient()
        
        # Behaviors
        self.explore_behavior = ExploreBehavior(self.planner, self.controller, self.lidar)
        self.exit_behavior = ExitFinder(self.client, self.lidar)
        
        # State
        self.current_task = None # "explore", "find_exit", "idle"
        self.is_running = False
        self.status_message = "Idle"
        self.last_error = None

    async def run_loop(self):
        """Main control loop."""
        self.is_running = True
        logging.info("Agent loop started.")
        
        while self.is_running:
            try:
                if self.current_task == "explore":
                    result = self.explore_behavior.step()
                    if result == "FINISHED":
                        self.status_message = "Exploration complete."
                        self.current_task = None
                    elif result == "PLAN_FAILED":
                        self.status_message = "Exploration stuck."
                        self.current_task = None
                    else:
                        self.status_message = "Exploring..."
                
                elif self.current_task == "find_exit":
                    # TODO: Implement exit finding loop
                    self.status_message = "Finding exit..."
                    
                else:
                    # Idle
                    pass
                    
            except Exception as e:
                logging.error(f"Agent loop error: {e}")
                self.last_error = str(e)
                
            await asyncio.sleep(0.1)

    def process_command(self, text: str):
        """Handle user chat command."""
        logging.info(f"User command: {text}")
        
        # 1. LLM Parse
        result = self.llm.parse_command(text)
        
        reply = result.get("reply", "")
        actions = result.get("actions", [])
        
        # 2. Execute Actions
        for action in actions:
            name = action["function"]
            args = action["arguments"]
            
            if name == "explore":
                self.current_task = "explore"
                reply += "\nStarting exploration."
            elif name == "stop":
                self.current_task = None
                self.client.stop()
                reply += "\nStopped."
            elif name == "find_exit":
                self.current_task = "find_exit"
                reply += "\nSearching for exit."
            elif name == "goto_pose":
                # TODO: Queue goto task
                pass
            elif name == "speak":
                text_to_say = args.get("text", "")
                self.client.speak(text_to_say)

        return {"reply": reply, "actions": actions}

    def get_health(self):
        return {
            "status": "running" if self.is_running else "stopped",
            "task": self.current_task,
            "message": self.status_message,
            "model": settings.OPENAI_MODEL
        }
