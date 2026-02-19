import math
from typing import Tuple, Optional
from clients.hexapod import HexapodClient
from config import settings
from sensors.lidar import LidarSensor

class MotionController:
    def __init__(self, client: HexapodClient, lidar: LidarSensor):
        self.client = client
        self.lidar = lidar
        
        # P-Controller Gains
        self.kp_linear = 0.5
        self.kp_angular = 1.0
        
        # Tolerances
        self.xy_tolerance = 0.05 # meters (~5cm) # Using SLAM coordinates
        self.yaw_tolerance = 0.1 # radians (~5 deg)

    def normalize_angle(self, angle):
        while angle > math.pi:
            angle -= 2 * math.pi
        while angle < -math.pi:
            angle += 2 * math.pi
        return angle

    def step_to_goal(self, current_pose: dict, goal_world: Tuple[float, float]) -> bool:
        """
        Calculates and sends velocity command to move towards goal.
        Returns True if reached.
        """
        # Parse current pose
        # Pose is from SLAM/Odometry. Units: meters? 
        # routes.rs says pose is from ubec... wait, Scan matching gives pose.
        # Let's assume pose X/Y are in meters if from SLAM map metadata, 
        # or we need to align with map resolution (0.05m/pixel).
        # The API /lidar/frame returns pose. Let's use that.
        
        # We need the robot's world position. 
        # If we use the map's origin and resolution, we can convert.
        # But let's assume the goal is passed in WORLD coordinates (meters).
        
        cx = current_pose.get("x", 0)
        cy = current_pose.get("y", 0)
        ctheta = current_pose.get("theta", 0)
        
        gx, gy = goal_world
        
        dx = gx - cx
        dy = gy - cy
        dist = math.sqrt(dx*dx + dy*dy)
        
        if dist < self.xy_tolerance:
            self.client.move(0, 0, 0)
            return True
            
        # Target heading
        target_heading = math.atan2(dy, dx)
        heading_error = self.normalize_angle(target_heading - ctheta)
        
        # Simple logic: 
        # If heading error is large, rotate in place.
        # If small, move forward + rotate.
        
        # Check obstacles first (Safety Layer)
        # Using raw lidar scan for immediate safety
        # We can implement a simplified Dynamic Window Approach (DWA) here later
        # For now, simplistic "stop if value close"
        scan = self.lidar.get_scan()
        blocked_front = False
        blocked_left = False
        blocked_right = False
        
        if scan:
            points = scan.get("points", [])
            for p in points:
                d = p.get("distance_mm", 10000)
                a = p.get("angle_deg", 0)
                
                if d < settings.SAFETY_DISTANCE: # 30cm
                    # Front sector: -30 to +30 deg
                    if -30 < a < 30:
                        blocked_front = True
                    # Left: 30 to 90
                    elif 30 < a < 90:
                        blocked_left = True
                    # Right: -90 to -30
                    elif -90 < a < -30:
                        blocked_right = True
        
        # Calculate velocities
        v_lin = 0.0
        v_ang = 0.0
        
        if abs(heading_error) > 0.5: # ~30 degrees
            # Rotate in place
            v_ang = self.kp_angular * heading_error
            # Clamp
            v_ang = max(-1.0, min(1.0, v_ang))
        else:
            # Move and steer
            v_lin = self.kp_linear * dist
            v_lin = max(0.0, min(1.0, v_lin)) # limit speed
            v_ang = self.kp_angular * heading_error
            v_ang = max(-0.5, min(0.5, v_ang))
            
        # Obstacle avoidance override
        if blocked_front and v_lin > 0:
            v_lin = 0
            # Try to rotate away from obstacle?
            if blocked_left and not blocked_right:
                v_ang = -0.5 # turn right
            elif blocked_right and not blocked_left:
                v_ang = 0.5 # turn left
            else:
                v_ang = 0.5 # turn left default
        
        # Convert to Hexapod API units
        # Forward: mm/s (-100 to 100) -> v_lin * 100
        # Strafe: mm/s -> 0 for now (differential drive style)
        # Rotation: rad/s? API says -1.0 to 1.0. 
        
        cmd_forward = v_lin * 100.0 * 2.0 # Scale up, max speed usually ~200mm/s
        cmd_rot = v_ang 
        
        self.client.move(cmd_forward, 0.0, cmd_rot)
        return False
