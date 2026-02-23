"""
Hexapod AI Agent
================
The Agent owns the run_loop which ticks every LOOP_INTERVAL seconds.
It delegates navigation to ExploreBehavior (frontier-based exploration)
and motion to MotionController.
"""

import asyncio
import logging
import time
from typing import Optional

from config import settings
from clients.hexapod import HexapodClient
from sensors.lidar import LidarSensor
from navigation.planner import Pathfinder
from navigation.controller import MotionController
from behaviors.explore import ExploreBehavior
from behaviors.find_exit import ExitFinder
from evaluator.llm import LLMClient


# How often the control loop ticks (seconds).
# Shorter = smoother movement; longer = less CPU.
LOOP_INTERVAL = 0.3

# How long to rotate during a SCAN step (seconds).
SCAN_DURATION = 1.5

# Speed constants for timed moves
MAX_SPEED_MM_S = 200.0
MAX_ROT_RAD_S  = 1.0


class Agent:
    def __init__(self):
        self.client   = HexapodClient()
        self.lidar    = LidarSensor(self.client)
        self.planner  = Pathfinder()
        self.controller = MotionController(self.client, self.lidar)
        self.llm      = LLMClient()

        # Behaviors
        self.explore_behavior = ExploreBehavior(
            self.planner, self.controller, self.lidar
        )
        self.exit_behavior = ExitFinder(self.client, self.lidar)

        # State
        self.current_task: Optional[str] = None  # "explore" | "find_exit" | None
        self.is_running  = False
        self.status_message = "Idle"
        self.last_error: Optional[str] = None

        # Internal counters
        self._tick = 0
        self._scan_until: float = 0.0   # epoch time when scan rotation ends

    # ------------------------------------------------------------------
    # Run loop
    # ------------------------------------------------------------------

    async def run_loop(self):
        """Main control loop."""
        self.is_running = True
        logging.info("Agent loop started.")

        while self.is_running:
            loop_start = time.monotonic()

            try:
                await self._tick_task()
            except Exception as e:
                logging.error(f"Agent loop error: {e}", exc_info=True)
                self.last_error = str(e)

            # Heartbeat every ~5 s
            self._tick += 1
            if self._tick % max(1, int(5.0 / LOOP_INTERVAL)) == 0:
                logging.info(
                    f"Agent heartbeat | task={self.current_task} | "
                    f"{self.status_message}"
                )

            # Sleep for the rest of the interval
            elapsed = time.monotonic() - loop_start
            sleep_time = max(0.0, LOOP_INTERVAL - elapsed)
            await asyncio.sleep(sleep_time)

    async def _tick_task(self):
        """Called once per loop tick."""
        if self.current_task == "explore":
            await self._tick_explore()
        elif self.current_task == "find_exit":
            self.status_message = "Searching for exit..."
            # TODO: exit finder implementation
        else:
            # Idle – do nothing
            pass

    async def _tick_explore(self):
        """One exploration tick."""
        now = time.monotonic()

        # If we're mid-scan rotation, don't call step()
        if now < self._scan_until:
            remaining = self._scan_until - now
            logging.debug(f"Explore: scanning… {remaining:.1f}s left")
            return

        result = self.explore_behavior.step()
        logging.info(f"Explore step → {result}")

        if result == "RUNNING":
            self.status_message = "Exploring…"

        elif result in ("REPLANNING", "NO_POSE", "NO_MAP"):
            self.status_message = f"Explore: {result}"

        elif result == "SCAN":
            # Rotate in place to gather more map data
            self.status_message = "Scanning for frontiers…"
            logging.info(f"Explore: SCAN – rotating {SCAN_DURATION}s")
            self.client.move(0.0, 0.0, MAX_ROT_RAD_S)
            self._scan_until = now + SCAN_DURATION
            # The next tick will return early until scan is done,
            # then stop is issued by the move(0,0,0) in the following step.

        elif result == "FINISHED":
            self.status_message = "Exploration complete."
            logging.info("Explore: FINISHED")
            self.client.stop()
            self.current_task = None

        else:
            self.status_message = f"Explore: unknown result '{result}'"

    # ------------------------------------------------------------------
    # Timed movement (used by chat commands)
    # ------------------------------------------------------------------

    async def execute_move(self, direction: str, duration: float, speed: float = 1.0):
        """Execute a timed movement command."""
        fwd, strafe, rot = 0.0, 0.0, 0.0

        if direction == "forward":
            fwd = 1.0
        elif direction == "backward":
            fwd = -1.0
        elif direction == "left":
            strafe = -1.0
        elif direction == "right":
            strafe = 1.0
        elif direction == "turn_left":
            rot = -1.0
        elif direction == "turn_right":
            rot = 1.0

        fwd    = fwd    * speed * MAX_SPEED_MM_S
        strafe = strafe * speed * MAX_SPEED_MM_S
        rot    = rot    * speed * MAX_ROT_RAD_S

        logging.info(f"Move {direction} {duration}s (f={fwd} s={strafe} r={rot})")
        self.client.move(fwd, strafe, rot)
        await asyncio.sleep(duration)
        self.client.stop()
        logging.info("Move finished.")

    # ------------------------------------------------------------------
    # Command processing (from web panel / chat)
    # ------------------------------------------------------------------

    async def process_command(self, text: str):
        """Parse and execute a natural-language command."""
        logging.info(f"User command: {text}")

        # Run blocking OpenAI call in a thread so we don't freeze the event loop
        loop = asyncio.get_event_loop()
        result = await loop.run_in_executor(None, self.llm.parse_command, text)

        # If LLM failed/timed out, fall back to simple keyword matching
        if "error" in result or not result.get("actions"):
            if "error" in result:
                logging.warning(f"LLM failed ({result['error']}), using keyword fallback")
            kw = self._keyword_parse(text)
            if kw:
                result = kw

        reply   = result.get("reply") or ""
        actions = result.get("actions", [])
        logging.info(f"Agent: Parsed actions: {actions}")

        for action in actions:
            name = action["function"]
            args = action.get("arguments", {})

            if name == "explore":
                self.explore_behavior.reset()
                self.current_task   = "explore"
                self._scan_until    = 0.0
                reply += "\nStarting exploration."

            elif name == "stop":
                self.current_task = None
                self.client.stop()
                reply += "\nStopped."

            elif name == "find_exit":
                self.current_task = "find_exit"
                reply += "\nSearching for exit."

            elif name == "speak":
                self.client.speak(args.get("text", ""))

            elif name == "walk":
                # Indefinite movement — just set velocity, no auto-stop
                direction = args.get("direction", "forward")
                speed     = float(args.get("speed", 1.0))
                fwd, strafe, rot = 0.0, 0.0, 0.0
                if direction == "forward":     fwd    =  speed * MAX_SPEED_MM_S
                elif direction == "backward":  fwd    = -speed * MAX_SPEED_MM_S
                elif direction == "left":      strafe = -speed * MAX_SPEED_MM_S
                elif direction == "right":     strafe =  speed * MAX_SPEED_MM_S
                elif direction == "turn_left": rot    = -speed * MAX_ROT_RAD_S
                elif direction == "turn_right":rot    =  speed * MAX_ROT_RAD_S
                self.current_task = "walk"
                self.client.move(fwd, strafe, rot)
                logging.info(f"Walk {direction} indefinitely (f={fwd} s={strafe} r={rot})")
                reply += f"\nWalking {direction} — say 'stop' to halt."

            elif name == "move":
                direction = args.get("direction", "forward")
                duration  = float(args.get("duration", 1.0))
                speed     = float(args.get("speed", 1.0))
                asyncio.ensure_future(self.execute_move(direction, duration, speed))
                reply += f"\nMoving {direction} for {duration}s."

        return {"reply": reply, "actions": actions}

    # ------------------------------------------------------------------
    # Keyword fallback (no LLM needed)
    # ------------------------------------------------------------------

    def _keyword_parse(self, text: str) -> dict | None:
        """
        Dead-simple keyword parser used when OpenAI is unavailable.
        Uses word-boundary matching so 'stopped' doesn't trigger 'stop'.
        Returns an actions dict or None if nothing matched.
        """
        import re
        t = text.lower().strip()

        def has_word(word):
            return bool(re.search(rf'\b{re.escape(word)}\b', t))

        # Stop / halt — must be a standalone word, not part of "stopped"/"unstoppable" etc
        if any(has_word(w) for w in ("stop", "halt", "cease", "freeze", "pause")):
            return {"actions": [{"function": "stop", "arguments": {}}], "reply": "Stopping."}

        # Explore
        if has_word("explore") or has_word("exploration"):
            return {"actions": [{"function": "explore", "arguments": {}}], "reply": "Starting exploration."}

        # Determine direction (order matters: check compounds before singles)
        direction = None
        if has_word("forward") or has_word("ahead") or has_word("straight"):
            direction = "forward"
        elif has_word("backward") or has_word("back") or has_word("reverse"):
            direction = "backward"
        elif "turn left" in t or "rotate left" in t:
            direction = "turn_left"
        elif "turn right" in t or "rotate right" in t:
            direction = "turn_right"
        elif has_word("left"):
            direction = "left"
        elif has_word("right"):
            direction = "right"

        if direction is None:
            return None

        # Check for explicit duration (e.g. "5 seconds", "3s")
        m = re.search(r"(\d+(?:\.\d+)?)\s*(?:seconds?|secs?)\b", t)
        if m:
            duration = float(m.group(1))
            return {
                "actions": [{"function": "move", "arguments": {"direction": direction, "duration": duration, "speed": 1.0}}],
                "reply": f"Moving {direction} for {duration}s."
            }

        # No duration → walk indefinitely
        return {
            "actions": [{"function": "walk", "arguments": {"direction": direction, "speed": 1.0}}],
            "reply": f"Walking {direction} until stopped."
        }

    # ------------------------------------------------------------------
    # Health / status
    # ------------------------------------------------------------------

    def get_health(self):
        return {
            "status":  "running" if self.is_running else "stopped",
            "task":    self.current_task,
            "message": self.status_message,
            "model":   settings.OPENAI_MODEL,
        }
