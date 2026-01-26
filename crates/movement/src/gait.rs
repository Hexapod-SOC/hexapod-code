use crate::gaits::GaitTemplate;
use crate::legs::Leg;
use glam::Vec3;

/// Represents the current state of a walking gait cycle
pub struct Gait {
    template: &'static GaitTemplate,
    phase: f32, // Current cycle phase [0.0, 1.0]
    default_stance: LegStances,
}

/// Default foot positions for all legs relative to body center
pub struct LegStances {
    pub left_front: Vec3,
    pub left_middle: Vec3,
    pub left_back: Vec3,
    pub right_front: Vec3,
    pub right_middle: Vec3,
    pub right_back: Vec3,
}

impl Default for LegStances {
    fn default() -> Self {
        // Default hexapod stance positions
        // X: forward/back, Y: left/right, Z: up/down
        LegStances {
            //FIXME why front/back X is 0 it should be a offset forward/back
            left_front: Vec3::new(0.0, -45.0, -70.0),
            left_middle: Vec3::new(0.0, -55.0, -50.0),
            left_back: Vec3::new(0.0, -45.0, -70.0),
            right_front: Vec3::new(0.0, 45.0, -70.0),
            right_middle: Vec3::new(0.0, 55.0, -50.0),
            right_back: Vec3::new(0.0, 45.0, -70.0),
        }
    }
}

impl LegStances {
    pub fn get(&self, leg: Leg) -> Vec3 {
        match leg {
            Leg::LeftFront => self.left_front,
            Leg::LeftMiddle => self.left_middle,
            Leg::LeftBack => self.left_back,
            Leg::RightFront => self.right_front,
            Leg::RightMiddle => self.right_middle,
            Leg::RightBack => self.right_back,
        }
    }

    pub fn set(&mut self, leg: Leg, pos: Vec3) {
        match leg {
            Leg::LeftFront => self.left_front = pos,
            Leg::LeftMiddle => self.left_middle = pos,
            Leg::LeftBack => self.left_back = pos,
            Leg::RightFront => self.right_front = pos,
            Leg::RightMiddle => self.right_middle = pos,
            Leg::RightBack => self.right_back = pos,
        }
    }

    /// Convert Vec3 positions to array format for API responses
    pub fn to_array(&self, leg: Leg) -> [f32; 3] {
        let pos = self.get(leg);
        [pos.x, pos.y, pos.z]
    }
}

impl Gait {
    pub fn new(template: &'static GaitTemplate) -> Self {
        Self {
            template,
            phase: 0.0,
            default_stance: LegStances::default(),
        }
    }

    pub fn with_stance(template: &'static GaitTemplate, stance: LegStances) -> Self {
        Self {
            template,
            phase: 0.0,
            default_stance: stance,
        }
    }

    /// Update the default stance configuration
    pub fn set_default_stance(&mut self, stance: LegStances) {
        self.default_stance = stance;
    }

    /// Get the current default stance
    pub fn get_default_stance(&self) -> &LegStances {
        &self.default_stance
    }

    /// Update gait phase based on time delta
    pub fn update(&mut self, dt: f32) {
        self.phase += dt * self.template.speed_multiplier;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
    }

    /// Get the cycle phase for a specific leg
    fn get_leg_phase(&self, leg: Leg) -> f32 {
        let offset = self.get_leg_offset(leg);
        let mut phase = self.phase + offset;
        if phase >= 1.0 {
            phase -= 1.0;
        }
        phase
    }

    fn get_leg_offset(&self, leg: Leg) -> f32 {
        match leg {
            Leg::LeftFront => self.template.leg_cycle_offsets.left_front,
            Leg::LeftMiddle => self.template.leg_cycle_offsets.left_middle,
            Leg::LeftBack => self.template.leg_cycle_offsets.left_back,
            Leg::RightFront => self.template.leg_cycle_offsets.right_front,
            Leg::RightMiddle => self.template.leg_cycle_offsets.right_middle,
            Leg::RightBack => self.template.leg_cycle_offsets.right_back,
        }
    }

    /// Calculate the position of a leg during walking
    /// velocity: direction and speed of movement (X=forward/back, Y=left/right, Z=up/down)
    /// rotation: body rotation speed (yaw, positive = counter-clockwise when viewed from above)
    pub fn calculate_leg_position(&self, leg: Leg, velocity: Vec3, rotation: f32) -> Vec3 {
        let leg_phase = self.get_leg_phase(leg);
        let default_pos = self.default_stance.get(leg);

        // Calculate step vector based on velocity and rotation
        let step_length = velocity.length() * self.template.step_length_multiplier;
        let step_length = step_length.min(self.template.max_step_length);

        // Direction of step
        let step_dir = if velocity.length() > 0.01 {
            velocity.normalize()
        } else {
            Vec3::ZERO
        };

        // Add rotation component to step (rotate around Z axis - vertical)
        let rotation_offset = Vec3::new(
            -default_pos.y * rotation, // Y position affects X velocity (tangential)
            default_pos.x * rotation,  // X position affects Y velocity (tangential)
            0.0,
        );

        let total_step = step_dir * step_length + rotation_offset;

        // Determine if leg is in swing (lifting) or stance (pushing) phase
        let push_fraction = self.template.push_fraction;

        if leg_phase < push_fraction {
            // STANCE PHASE: Leg is on ground, pushing backward
            // Normalize progress to 0.0 -> 1.0 within the stance phase
            let stance_progress = leg_phase / push_fraction;

            // Apply slight easing to stance phase too for smoother ground contact transitions
            // (Linear is usually fine for stance, but slight easing reduces jerk at transitions)
            // Using a very subtle S-curve or keeping it linear to maintain constant ground speed.
            // Constant ground speed is preferred for stability, so we keep it mostly linear.
            let current_offset = total_step * (0.5 - stance_progress);
            
            default_pos + current_offset
        } else {
            // SWING PHASE: Leg is lifting and moving forward
            // Normalize progress to 0.0 -> 1.0 within the swing phase
            let swing_progress = (leg_phase - push_fraction) / (1.0 - push_fraction);

            // Horizontal Movement: Ease-In-Out for smooth acceleration/deceleration of the leg
            // Using Cosine interpolation: 0.5 * (1.0 - cos(t * pi))
            let pi = std::f32::consts::PI;
            let ease_progress = 0.5 * (1.0 - (swing_progress * pi).cos());

            // Start position (end of stance) and End position (start of stance)
            // We want to move from -0.5 * total_step to +0.5 * total_step (relative to center)
            let start_offset = total_step * -0.5;
            let end_offset = total_step * 0.5;
            
            // Interpolate smoothly
            let horizontal_offset = start_offset.lerp(end_offset, ease_progress);

            // Vertical Movement (Lift): Sinusoidal trajectory
            // We want a curve that goes 0 -> 1 -> 0
            // sin(t * pi) gives us exactly that for t in 0..1
            let lift_height = self.template.lift_height_multiplier * 50.0; // Adjusted base height
            let lift = (swing_progress * pi).sin() * lift_height;

            default_pos + horizontal_offset + Vec3::new(0.0, 0.0, lift)
        }
    }

    /// Get positions for all legs
    pub fn calculate_all_leg_positions(&self, velocity: Vec3, rotation: f32) -> LegStances {
        let mut stances = LegStances::default();
        stances.left_front = self.calculate_leg_position(Leg::LeftFront, velocity, rotation);
        stances.left_middle = self.calculate_leg_position(Leg::LeftMiddle, velocity, rotation);
        stances.left_back = self.calculate_leg_position(Leg::LeftBack, velocity, rotation);
        stances.right_front = self.calculate_leg_position(Leg::RightFront, velocity, rotation);
        stances.right_middle = self.calculate_leg_position(Leg::RightMiddle, velocity, rotation);
        stances.right_back = self.calculate_leg_position(Leg::RightBack, velocity, rotation);
        stances
    }

    pub fn get_phase(&self) -> f32 {
        self.phase
    }

    pub fn get_template(&self) -> &GaitTemplate {
        self.template
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gaits::GAITS;

    #[test]
    fn test_trajectory_smoothness() {
        let template = &GAITS[0]; // Tripod
        let mut gait = Gait::new(template);
        let dt = 0.05;
        let mut t = 0.0;
        
        let velocity = Vec3::new(100.0, 0.0, 0.0);
        
        // Run a cycle
        while t < 2.0 {
            gait.update(dt);
            let phase = gait.get_phase();
            let pos = gait.calculate_leg_position(Leg::LeftFront, velocity, 0.0);
            
            // Just ensure values are not NaN and reasonable
            assert!(!pos.z.is_nan());
            assert!(!pos.x.is_nan());
            assert!(!pos.y.is_nan());
            
            // Check lift height during swing vs stance
            let push_fraction = template.push_fraction;
             if phase > push_fraction && phase < 0.99 {
                // Let's print for manual verification in output
                println!("Time: {:.2}, Phase: {:.2}, Z: {:.2}", t, phase, pos.z);
            }
            
            t += dt;
        }
    }
}
