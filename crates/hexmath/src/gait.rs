use crate::hexapod::LegId;
use std::collections::HashSet;

/// Different gait patterns available
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GaitType {
    /// Classic tripod gait: 3 legs move while 3 support
    /// Groups: [LF, RM, LB] and [RF, LM, RB]
    #[default]
    Tripod,

    /// Tetrapod/Quad gait: 2 legs move while 4 support
    /// Used when a leg is disabled - provides more stability
    /// Groups cycle through pairs while rest balance
    Tetrapod,

    /// Wave gait: Single leg moves at a time
    /// Slowest but most stable, good for rough terrain
    Wave,

    /// Ripple gait: legs move in sequence, but with more overlap than wave
    /// 2-3 legs in swing at once, smoother than wave, slower than tripod
    Ripple,
}

impl GaitType {
    pub fn name(&self) -> &'static str {
        match self {
            GaitType::Tripod => "Tripod",
            GaitType::Tetrapod => "Tetrapod",
            GaitType::Wave => "Wave",
            GaitType::Ripple => "Ripple",
        }
    }

    /// Get number of phases in the gait cycle
    pub fn phase_count(&self) -> usize {
        match self {
            GaitType::Tripod => 2,
            GaitType::Tetrapod => 3,
            GaitType::Wave => 6,
            GaitType::Ripple => 6,
        }
    }

    /// Cycle to next gait type
    pub fn next(&self) -> GaitType {
        match self {
            GaitType::Tripod => GaitType::Tetrapod,
            GaitType::Tetrapod => GaitType::Wave,
            GaitType::Wave => GaitType::Ripple,
            GaitType::Ripple => GaitType::Tripod,
        }
    }
}

/// Configuration for gait parameters
#[derive(Clone, Debug)]
pub struct GaitConfig {
    /// Current gait type
    pub gait_type: GaitType,

    /// Step length in mm (how far each step travels)
    pub step_length: f32,

    /// Step height in mm (how high legs lift during swing)
    pub step_height: f32,

    /// Gait cycle speed multiplier
    pub speed: f32,

    /// Base body height relative to ground (negative = below leg mounts)
    pub base_height: f32,

    /// Set of disabled legs (won't be animated)
    pub disabled_legs: HashSet<LegId>,

    /// Duty factor: portion of cycle leg is on ground (0.5 = 50% stance, 50% swing)
    pub duty_factor: f32,

    /// Multiplier for how much stance legs push the body
    pub body_push_gain: f32,

    /// Optional per-leg phase overrides in order:
    /// [LeftFront, LeftMiddle, LeftBack, RightFront, RightMiddle, RightBack]
    pub phase_offsets_override: Option<[f32; 6]>,
}

impl Default for GaitConfig {
    fn default() -> Self {
        Self {
            gait_type: GaitType::Tripod,
            step_length: 125.0,
            step_height: 40.0,
            speed: 1.0,
            base_height: -70.0,
            disabled_legs: HashSet::new(),
            duty_factor: 0.5,
            body_push_gain: 2.5,
            phase_offsets_override: None,
        }
    }
}

impl GaitConfig {
    /// Check if a leg is enabled
    pub fn is_leg_enabled(&self, leg: LegId) -> bool {
        !self.disabled_legs.contains(&leg)
    }

    /// Toggle a leg's enabled state
    pub fn toggle_leg(&mut self, leg: LegId) {
        if self.disabled_legs.contains(&leg) {
            self.disabled_legs.remove(&leg);
        } else {
            self.disabled_legs.insert(leg);
        }

        // Auto-switch to tetrapod if a leg is disabled
        if !self.disabled_legs.is_empty() && self.gait_type == GaitType::Tripod {
            self.gait_type = GaitType::Tetrapod;
        }
    }

    /// Disable a specific leg
    pub fn disable_leg(&mut self, leg: LegId) {
        self.disabled_legs.insert(leg);
        // Auto-switch to appropriate gait
        if self.gait_type == GaitType::Tripod {
            self.gait_type = GaitType::Tetrapod;
        }
    }

    /// Enable a specific leg
    pub fn enable_leg(&mut self, leg: LegId) {
        self.disabled_legs.remove(&leg);
    }

    /// Get count of enabled legs
    pub fn enabled_leg_count(&self) -> usize {
        6 - self.disabled_legs.len()
    }
}

/// Represents a leg's state in the gait cycle
#[derive(Clone, Copy, Debug)]
pub struct LegGaitState {
    /// Phase offset for this leg (0.0 to 1.0)
    pub phase_offset: f32,
    /// Whether leg is currently in swing phase
    pub is_swing: bool,
    /// Current swing progress (0.0 to 1.0, only valid if is_swing)
    pub swing_progress: f32,
}

/// Get the phase offset for each leg based on gait type
pub fn get_leg_phase_offsets(
    gait_type: GaitType,
    disabled_legs: &HashSet<LegId>,
    overrides: Option<&[f32; 6]>,
) -> [(LegId, f32); 6] {
    if let Some(override_offsets) = overrides {
        return [
            (LegId::LeftFront, override_offsets[0]),
            (LegId::LeftMiddle, override_offsets[1]),
            (LegId::LeftBack, override_offsets[2]),
            (LegId::RightFront, override_offsets[3]),
            (LegId::RightMiddle, override_offsets[4]),
            (LegId::RightBack, override_offsets[5]),
        ];
    }
    match gait_type {
        GaitType::Tripod => [
            (LegId::LeftFront, 0.0),
            (LegId::LeftMiddle, 0.5),
            (LegId::LeftBack, 0.0),
            (LegId::RightFront, 0.5),
            (LegId::RightMiddle, 0.0),
            (LegId::RightBack, 0.5),
        ],
        GaitType::Tetrapod => {
            let enabled: Vec<LegId> = [
                LegId::LeftFront,
                LegId::LeftMiddle,
                LegId::LeftBack,
                LegId::RightFront,
                LegId::RightMiddle,
                LegId::RightBack,
            ]
            .into_iter()
            .filter(|l| !disabled_legs.contains(l))
            .collect();

            let phase_step = 1.0 / (enabled.len().max(1) as f32 / 2.0).ceil();

            let mut offsets = [
                (LegId::LeftFront, 0.0),
                (LegId::LeftMiddle, 0.0),
                (LegId::LeftBack, 0.0),
                (LegId::RightFront, 0.0),
                (LegId::RightMiddle, 0.0),
                (LegId::RightBack, 0.0),
            ];

            if disabled_legs.is_empty() {
                offsets = [
                    (LegId::LeftFront, 0.0),
                    (LegId::LeftMiddle, 0.333),
                    (LegId::LeftBack, 0.666),
                    (LegId::RightFront, 0.666),
                    (LegId::RightMiddle, 0.333),
                    (LegId::RightBack, 0.0),
                ];
            } else {
                for (i, leg) in enabled.iter().enumerate() {
                    let phase = (i as f32 * phase_step) % 1.0;
                    for offset in &mut offsets {
                        if offset.0 == *leg {
                            offset.1 = phase;
                        }
                    }
                }
            }

            offsets
        }
        GaitType::Wave => [
            (LegId::LeftFront, 0.0),
            (LegId::LeftMiddle, 0.167),
            (LegId::LeftBack, 0.333),
            (LegId::RightFront, 0.833),
            (LegId::RightMiddle, 0.667),
            (LegId::RightBack, 0.5),
        ],
        GaitType::Ripple => [
            (LegId::LeftFront, 0.0),
            (LegId::RightMiddle, 0.166),
            (LegId::LeftBack, 0.333),
            (LegId::RightFront, 0.5),
            (LegId::LeftMiddle, 0.666),
            (LegId::RightBack, 0.833),
        ],
    }
}

/// Calculate leg state for a given phase in the gait cycle
pub fn calculate_leg_state(
    phase: f32,
    phase_offset: f32,
    duty_factor: f32,
    is_enabled: bool,
) -> LegGaitState {
    if !is_enabled {
        return LegGaitState {
            phase_offset,
            is_swing: false,
            swing_progress: 0.0,
        };
    }

    let leg_phase = (phase + phase_offset) % 1.0;

    let swing_duration = 1.0 - duty_factor;
    let is_swing = leg_phase < swing_duration;

    let swing_progress = if is_swing {
        leg_phase / swing_duration
    } else {
        0.0
    };

    LegGaitState {
        phase_offset,
        is_swing,
        swing_progress,
    }
}

/// Calculate foot position offset based on gait state
/// Returns (stride_offset, height_offset) in mm
pub fn calculate_foot_offset(
    state: &LegGaitState,
    step_length: f32,
    step_height: f32,
    move_magnitude: f32,
) -> (f32, f32) {
    if state.is_swing {
        let t = state.swing_progress;
        let stride_offset = (t * 2.0 - 1.0) * step_length * 0.5 * move_magnitude;
        let height_offset = (t * std::f32::consts::PI).sin() * step_height;
        (stride_offset, height_offset)
    } else {
        let stance_progress = (state.phase_offset % 1.0).max(0.0);
        let stride_offset = (0.5 - stance_progress) * step_length * move_magnitude;
        (stride_offset, 0.0)
    }
}
