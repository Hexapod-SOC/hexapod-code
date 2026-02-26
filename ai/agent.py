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
from behaviors.navigate import NavigateBehavior
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

# Obstacle avoidance (local)
AVOID_DURATION_S = 0.8
AVOID_FWD_MM_S = 50.0
AVOID_BACK_MM_S = 60.0
AVOID_STRAFE_MM_S = 120.0
AVOID_ROT_RAD_S = 0.4


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
        self.exit_behavior = ExitFinder(self.planner, self.controller, self.lidar)
        self.navigate_behavior = NavigateBehavior(self.planner, self.controller, self.lidar)

        # State
        self.current_task: Optional[str] = None  # "explore" | "find_exit" | None
        self.is_running  = False
        self.status_message = "Idle"
        self.last_error: Optional[str] = None
        self.walk_velocity = (0.0, 0.0, 0.0)

        # Avoidance state (for walk/move)
        self._avoid_until: float = 0.0
        self._avoid_velocity = (0.0, 0.0, 0.0)

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
            result = self.exit_behavior.step()
            if result == "RUNNING":
                self.status_message = "Finding exit..."
            elif result == "SCAN":
                self.status_message = "Exit: scanning for route..."
            elif result == "NO_MAP":
                self.status_message = "Exit: no map"
            elif result == "NO_POSE":
                self.status_message = "Exit: no pose"
            elif result == "REPLANNING":
                self.status_message = "Exit: replanning..."
            elif result == "BLOCKED":
                self.status_message = "Exit: obstacle, escaping..."
            elif result == "FINISHED":
                self.status_message = "Exit: route complete."
                self.client.stop()
                self.current_task = None
        elif self.current_task == "walk":
            now = time.monotonic()

            if self._avoid_until > now:
                self.client.move(*self._avoid_velocity)
                return

            blocked, reason = self._movement_blocked(*self.walk_velocity)
            if blocked and self.walk_velocity[0] > 0 and reason == "front":
                avoidance = self._compute_avoidance_velocity()
                if avoidance:
                    self._avoid_velocity = avoidance
                    self._avoid_until = now + AVOID_DURATION_S
                    self.client.move(*avoidance)
                    self.status_message = "Avoiding obstacle..."
                    return

            if blocked:
                self.client.stop()
                self.current_task = None
                self.status_message = f"Stopped: obstacle {reason}"
            else:
                self.client.move(*self.walk_velocity)
        elif self.current_task == "navigate":
            result = self.navigate_behavior.step()
            if result == "RUNNING":
                self.status_message = "Navigating..."
            elif result == "REACHED":
                self.status_message = "Waypoint reached."
            elif result in ("REPLANNING", "NO_MAP", "NO_POSE"):
                self.status_message = f"Navigate: {result}"
            elif result == "FINISHED":
                self.status_message = "Navigation complete."
                self.client.stop()
                self.current_task = None
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

        blocked, reason = self._movement_blocked(fwd, strafe, rot)
        if blocked and fwd > 0 and reason == "front":
            avoidance = self._compute_avoidance_velocity()
            if avoidance:
                logging.info("Move blocked: avoiding obstacle")
                self.client.move(*avoidance)
                await asyncio.sleep(AVOID_DURATION_S)
            else:
                self.client.stop()
                logging.info(f"Move blocked: obstacle {reason}")
                return
        elif blocked:
            self.client.stop()
            logging.info(f"Move blocked: obstacle {reason}")
            return

        logging.info(f"Move {direction} {duration}s (f={fwd} s={strafe} r={rot})")
        self.client.move(fwd, strafe, rot)

        start = time.monotonic()
        while time.monotonic() - start < duration:
            blocked, reason = self._movement_blocked(fwd, strafe, rot)
            if blocked and fwd > 0 and reason == "front":
                avoidance = self._compute_avoidance_velocity()
                if avoidance:
                    logging.info("Move stopped early: avoiding obstacle")
                    self.client.move(*avoidance)
                    await asyncio.sleep(AVOID_DURATION_S)
                    continue

            if blocked:
                self.client.stop()
                logging.info(f"Move stopped early: obstacle {reason}")
                return
            await asyncio.sleep(0.2)

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
                self.exit_behavior.reset()
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
                blocked, reason = self._movement_blocked(fwd, strafe, rot)
                if blocked and fwd > 0 and reason == "front":
                    avoidance = self._compute_avoidance_velocity()
                    if avoidance:
                        self.current_task = "walk"
                        self.walk_velocity = (fwd, strafe, rot)
                        self._avoid_velocity = avoidance
                        self._avoid_until = time.monotonic() + AVOID_DURATION_S
                        self.client.move(*avoidance)
                        logging.info("Walk forward: avoiding obstacle")
                        reply += "\nObstacle ahead — avoiding while continuing forward."
                    else:
                        self.client.stop()
                        reply += f"\nBlocked by obstacle {reason}."
                elif blocked:
                    self.client.stop()
                    reply += f"\nBlocked by obstacle {reason}."
                else:
                    self.current_task = "walk"
                    self.walk_velocity = (fwd, strafe, rot)
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

        # Exit / leave room
        if any(has_word(w) for w in ("exit", "leave", "outside", "out")):
            return {"actions": [{"function": "find_exit", "arguments": {}}], "reply": "Finding the exit."}

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

    def set_navigation_waypoints(self, waypoints, mode: str = "replace"):
        self.navigate_behavior.set_waypoints(waypoints, mode=mode)
        if self.navigate_behavior.waypoints:
            self.current_task = "navigate"
            self.status_message = "Navigating..."
        else:
            if self.current_task == "navigate":
                self.current_task = None
            self.status_message = "Navigation cleared."

    def clear_navigation_waypoints(self):
        self.navigate_behavior.clear_waypoints()
        if self.current_task == "navigate":
            self.current_task = None
        self.status_message = "Navigation cleared."

    def get_navigation_status(self):
        return {
            "task": self.current_task,
            "waypoints": [
                {"x": wp[0], "y": wp[1]}
                for wp in self.navigate_behavior.waypoints
            ],
        }

    def _movement_blocked(self, fwd: float, strafe: float, rot: float) -> tuple[bool, str]:
        """Return (blocked, reason) based on LIDAR scan."""
        scan = self.lidar.get_scan()
        if not scan:
            return False, ""

        blocked_front = False
        blocked_left = False
        blocked_right = False
        blocked_back = False

        for p in scan.get("points", []):
            d = p.get("distance_mm", 99999)
            a = p.get("angle_deg", 0)
            if d < settings.SAFETY_DISTANCE:
                if -30 < a < 30:
                    blocked_front = True
                elif 30 <= a < 120:
                    blocked_left = True
                elif -120 < a <= -30:
                    blocked_right = True
                elif a >= 150 or a <= -150:
                    blocked_back = True

        if settings.INVERT_LIDAR_LEFT_RIGHT:
            blocked_left, blocked_right = blocked_right, blocked_left

        if fwd > 0 and blocked_front:
            return True, "front"
        if fwd < 0 and blocked_back:
            return True, "rear"
        if strafe > 0 and blocked_right:
            return True, "right"
        if strafe < 0 and blocked_left:
            return True, "left"

        return False, ""

    def _compute_avoidance_velocity(self) -> Optional[tuple[float, float, float]]:
        """Return an avoidance velocity (fwd, strafe, rot) based on LIDAR scan."""
        scan = self.lidar.get_scan()
        if not scan:
            return None

        front_min = settings.LIDAR_MAX_RANGE
        left_min = settings.LIDAR_MAX_RANGE
        right_min = settings.LIDAR_MAX_RANGE

        for p in scan.get("points", []):
            d = p.get("distance_mm", settings.LIDAR_MAX_RANGE)
            a = p.get("angle_deg", 0)
            if -30 < a < 30:
                front_min = min(front_min, d)
            elif 30 <= a < 120:
                left_min = min(left_min, d)
            elif -120 < a <= -30:
                right_min = min(right_min, d)

        if settings.INVERT_LIDAR_LEFT_RIGHT:
            left_min, right_min = right_min, left_min

        # Choose the clearer side
        if left_min >= right_min:
            side = "left"
            side_dir = -1.0
        else:
            side = "right"
            side_dir = 1.0

        if max(left_min, right_min) < settings.SAFETY_DISTANCE:
            logging.info("Avoidance: sides tight, backing up")
            return (-AVOID_BACK_MM_S, 0.0, side_dir * AVOID_ROT_RAD_S)

        logging.info(f"Avoidance: steering {side} (L={left_min:.0f}mm R={right_min:.0f}mm)")
        fwd = min(AVOID_FWD_MM_S, MAX_SPEED_MM_S * 0.3)
        strafe = side_dir * min(AVOID_STRAFE_MM_S, MAX_SPEED_MM_S * 0.7)
        rot = side_dir * AVOID_ROT_RAD_S
        return (fwd, strafe, rot)
