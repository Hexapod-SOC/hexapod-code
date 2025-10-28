use crate::gaits::GaitTemplate;
use crate::legs::Leg;
use glam::Vec3;
use std::f32::consts::PI;

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
            left_front: Vec3::new(0.0, -55.0, -70.0),
            left_middle: Vec3::new(0.0, -65.0, -65.0),
            left_back: Vec3::new(0.0, -55.0, -70.0),
            right_front: Vec3::new(0.0, 55.0, -70.0),
            right_middle: Vec3::new(0.0, 65.0, -65.0),
            right_back: Vec3::new(0.0, 55.0, -70.0),
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
    /// velocity: direction and speed of movement (X=forward, Y=strafe left/right, Z=vertical)
    /// rotation: body rotation speed (yaw)
    pub fn calculate_leg_position(
        &self,
        leg: Leg,
        velocity: Vec3,
        rotation: f32,
    ) -> Vec3 {
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

        // Add rotation component to step (rotate around Z axis)
        // For rotation: tangential velocity = angular_velocity × radius
        let rotation_offset = Vec3::new(
            -default_pos.y * rotation,  // Y affects X movement
            default_pos.x * rotation,   // X affects Y movement
            0.0,
        );

        let total_step = step_dir * step_length + rotation_offset;

        // Determine if leg is in swing (lifting) or stance (pushing) phase
        let push_fraction = self.template.push_fraction;
        
        if leg_phase < push_fraction {
            // STANCE PHASE: Leg is on ground, pushing backward
            let stance_progress = leg_phase / push_fraction;
            let step_offset = total_step * (0.5 - stance_progress);
            default_pos + step_offset
        } else {
            // SWING PHASE: Leg is lifting and moving forward
            let swing_progress = (leg_phase - push_fraction) / (1.0 - push_fraction);
            
            // Start position (end of stance)
            let start_offset = total_step * -0.5;
            
            // End position (start of stance)
            let end_offset = total_step * 0.5;
            
            // Interpolate horizontally
            let horizontal_offset = start_offset.lerp(end_offset, swing_progress);
            
            // Lift trajectory (square wave with rounded corners) - Z is up
            let lift_height = self.template.lift_height_multiplier * 60.0;
            
            // Create a square wave with smooth transitions using smoothstep
            let lift = if swing_progress < 0.15 {
                // Rising edge - smooth ramp up
                let t = swing_progress / 0.15;
                let smoothed = t * t * (3.0 - 2.0 * t); // smoothstep
                lift_height * smoothed
            } else if swing_progress < 0.85 {
                // Flat top - stay at maximum height
                lift_height
            } else {
                // Falling edge - smooth ramp down
                let t = (swing_progress - 0.85) / 0.15;
                let smoothed = t * t * (3.0 - 2.0 * t); // smoothstep
                lift_height * (1.0 - smoothed)
            };
            
            default_pos + horizontal_offset + Vec3::new(0.0, 0.0, lift)
        }
    }

    /// Get positions for all legs
    pub fn calculate_all_leg_positions(
        &self,
        velocity: Vec3,
        rotation: f32,
    ) -> LegStances {
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