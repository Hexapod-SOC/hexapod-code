pub mod hexapod;
pub mod ik;
pub mod gait;
pub mod motion;

pub use gait::{
	calculate_foot_offset, calculate_leg_state, get_leg_phase_offsets, GaitConfig, GaitType,
	LegGaitState,
};
pub use motion::{
	compute_leg_joints, get_leg_mut, step_hexapod, InputState, StepResult, WalkState,
};