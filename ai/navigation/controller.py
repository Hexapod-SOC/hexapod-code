import math
import logging
from typing import Tuple
from clients.hexapod import HexapodClient
from config import settings
from sensors.lidar import LidarSensor


class MotionController:
    """
    Proportional controller that moves the robot toward a goal.
    Sends movement commands to the Hexapod API.

    API units:
        forward/strafe: mm/s  (we use ~200 max)
        rotation:       rad/s (we use ~1.0 max)
    """

    # Maximum speeds – Rust API accepts forward/strafe in [-100, 100] mm/s
    MAX_LINEAR_MM_S = 160.0
    MAX_ANGULAR_RAD_S = 0.7

    # P-gains (output is in mm/s for linear, rad/s for angular)
    KP_LINEAR = 800.0   # dist in meters -> mm/s.  e.g. 0.2m * 800 = 160mm/s (capped)
    KP_ANGULAR = 0.8    # angle in radians -> rad/s

    MIN_FORWARD_MM_S = 30.0

    # Tolerances
    XY_TOL_M = 0.08     # 8cm – considered "at goal"
    YAW_TOL_RAD = 0.15  # ~9 degrees

    # Safety
    SAFETY_DIST_MM = 200  # 20cm

    def __init__(self, client: HexapodClient, lidar: LidarSensor):
        self.client = client
        self.lidar = lidar

    @staticmethod
    def _wrap_angle(a: float) -> float:
        while a > math.pi:
            a -= 2 * math.pi
        while a < -math.pi:
            a += 2 * math.pi
        return a

    def _check_obstacles(self):
        """Returns (blocked_front, blocked_left, blocked_right)."""
        blocked_front = False
        blocked_left = False
        blocked_right = False

        scan = self.lidar.get_scan()
        if not scan:
            return blocked_front, blocked_left, blocked_right

        for p in scan.get("points", []):
            d = p.get("distance_mm", 99999)
            a = p.get("angle_deg", 0)
            if d < self.SAFETY_DIST_MM:
                if -30 < a < 30:
                    blocked_front = True
                elif 30 <= a < 120:
                    blocked_left = True
                elif -120 < a <= -30:
                    blocked_right = True

        if settings.INVERT_LIDAR_LEFT_RIGHT:
            blocked_left, blocked_right = blocked_right, blocked_left

        return blocked_front, blocked_left, blocked_right

    def step_to_goal(self, current_pose: dict, goal_world: Tuple[float, float]) -> str:
        """
        Issue one velocity command to move toward goal_world (in meters).

        Returns:
            'REACHED'  – robot is within tolerance of goal
            'MOVING'   – command sent successfully
            'BLOCKED'  – obstacle in the way, turning
        """
        cx = current_pose.get("x", 0.0)
        cy = current_pose.get("y", 0.0)
        ctheta = current_pose.get("theta", 0.0)

        gx, gy = goal_world
        dx = gx - cx
        dy = gy - cy
        dist = math.sqrt(dx * dx + dy * dy)

        logging.debug(f"Controller: pose=({cx:.3f},{cy:.3f}) goal=({gx:.3f},{gy:.3f}) dist={dist:.3f}m")

        if dist < self.XY_TOL_M:
            self.client.move(0, 0, 0)
            logging.info("Controller: REACHED goal")
            return "REACHED"

        target_heading = math.atan2(dy, dx)
        heading_error = self._wrap_angle(target_heading - ctheta)
        if settings.INVERT_HEADING_ERROR:
            heading_error = -heading_error

        bf, bl, br = self._check_obstacles()

        v_lin_mm = 0.0
        v_ang = 0.0

        if bf:
            logging.warning("Controller: BLOCKED front, stopping for replan")
            self.client.move(0.0, 0.0, 0.0)
            return "BLOCKED"

        if abs(heading_error) > self.YAW_TOL_RAD:
            # Turn toward goal
            v_ang = self.KP_ANGULAR * heading_error
            v_ang = max(-self.MAX_ANGULAR_RAD_S, min(self.MAX_ANGULAR_RAD_S, v_ang))

            # Move forward too if we're roughly aligned
            if abs(heading_error) < 1.0:
                v_lin_mm = min(self.MAX_LINEAR_MM_S * 0.6, self.KP_LINEAR * dist)
                v_lin_mm = max(self.MIN_FORWARD_MM_S, v_lin_mm)
        else:
            # Aligned – move forward, small steering correction
            v_lin_mm = self.KP_LINEAR * dist
            v_lin_mm = min(self.MAX_LINEAR_MM_S, v_lin_mm)
            v_ang = self.KP_ANGULAR * heading_error
            v_ang = max(-0.5, min(0.5, v_ang))

        logging.info(f"Controller: MOVING fwd={v_lin_mm:.1f}mm/s ang={v_ang:.2f}rad/s head_err={math.degrees(heading_error):.1f}deg dist={dist:.3f}m")
        self.client.move(v_lin_mm, 0.0, v_ang)
        return "MOVING"
