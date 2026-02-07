use crate::gait::{calculate_leg_state, get_leg_phase_offsets, GaitConfig, GaitType};
use crate::hexapod::{Hexapod, Leg, LegId};
use crate::ik::{ik_solve_leg, Geometry};
use glam::Vec3;
use std::collections::HashMap;

const COXA_YAW_GAIN: f32 = 1.0;

#[derive(Clone, Debug, Default)]
pub struct InputState {
    /// Forward/backward movement (-1 to 1)
    pub move_forward: f32,
    /// Left/right strafe (-1 to 1)
    pub move_strafe: f32,
    /// Turn left/right (-1 to 1)
    pub turn: f32,
    /// Current body rotation (yaw) in radians
    pub body_yaw: f32,
}

#[derive(Clone, Debug)]
pub struct WalkState {
    pub time: f32,
    pub phase: f32,
    pub body_pos: Vec3,
    pub body_velocity: Vec3,
}

impl Default for WalkState {
    fn default() -> Self {
        Self {
            time: 0.0,
            phase: 0.0,
            body_pos: Vec3::ZERO,
            body_velocity: Vec3::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StepResult {
    pub body_pos: Vec3,
    pub body_yaw: f32,
    pub is_moving: bool,
    pub stance_count: usize,
}

pub fn step_hexapod(
    hexapod: &mut Hexapod,
    walk: &mut WalkState,
    input_state: &InputState,
    gait_config: &GaitConfig,
    dt: f32,
    scale: f32,
    ground_level: f32,
    foot_radius: f32,
) -> StepResult {
    let move_magnitude =
        (input_state.move_forward.powi(2) + input_state.move_strafe.powi(2)).sqrt();
    let is_moving = move_magnitude > 0.1 || input_state.turn.abs() > 0.1;

    if is_moving {
        let effective_speed = move_magnitude.max(input_state.turn.abs());
        walk.phase = (walk.phase + dt * gait_config.speed * effective_speed) % 1.0;
        walk.time += dt * gait_config.speed * effective_speed;
    }

    let phase_offsets = get_leg_phase_offsets(
        gait_config.gait_type,
        &gait_config.disabled_legs,
        gait_config.phase_offsets_override.as_ref(),
    );

    let mut body_push = Vec3::ZERO;
    let mut stance_count = 0;

    for (leg_id, phase_offset) in phase_offsets {
        let is_enabled = gait_config.is_leg_enabled(leg_id);

        let duty_factor = match gait_config.gait_type {
            GaitType::Ripple => 0.83,
            _ => gait_config.duty_factor,
        };

        let leg_state = calculate_leg_state(walk.phase, phase_offset, duty_factor, is_enabled);

        let (stride_offset, height_offset) = if is_enabled {
            if leg_state.is_swing {
                let t = leg_state.swing_progress.clamp(0.0, 1.0);
                let swing_curve = (t * std::f32::consts::PI).sin();
                let stride_ease = (1.0 - (t * std::f32::consts::PI).cos()) * 0.5;

                let stride = (-0.5 + stride_ease) * gait_config.step_length;
                let height = swing_curve * gait_config.step_height;
                (stride, height)
            } else {
                let stance_progress = (walk.phase + phase_offset) % 1.0;
                let swing_duration = 1.0 - duty_factor;
                let stance_normalized =
                    ((stance_progress - swing_duration) / duty_factor).clamp(0.0, 1.0);

                let stance_ease = (1.0 - (stance_normalized * std::f32::consts::PI).cos()) * 0.5;
                let stride = (0.5 - stance_ease) * gait_config.step_length;
                (stride, 0.0)
            }
        } else {
            (0.0, 0.0)
        };

        let move_dir_forward = input_state.move_forward;
        let move_dir_strafe = input_state.move_strafe;

        let is_left = matches!(
            leg_id,
            LegId::LeftFront | LegId::LeftMiddle | LegId::LeftBack
        );
        let turn_differential = if is_left {
            -input_state.turn
        } else {
            input_state.turn
        };

        let effective_stride = stride_offset * (move_dir_forward + turn_differential * 0.5);
        let strafe_stride = stride_offset * move_dir_strafe;

        let effective_stride = if is_left {
            -effective_stride
        } else {
            effective_stride
        };
        let strafe_stride = if is_left { strafe_stride } else { -strafe_stride };

        let leg = get_leg_mut(hexapod, leg_id);
        let reach = leg.femur_length * 0.6 + leg.tibia_length * 0.4;

        let mount_rad = if is_left {
            (-leg.mount_angle).to_radians()
        } else {
            leg.mount_angle.to_radians()
        };

        let stride_local_x = effective_stride * mount_rad.sin() + strafe_stride * mount_rad.cos();
        let stride_local_y = effective_stride * mount_rad.cos() - strafe_stride * mount_rad.sin();

        let foot = Vec3::new(
            leg.coxa_length + reach + stride_local_x,
            stride_local_y,
            gait_config.base_height + height_offset,
        );

        let geom = Geometry {
            coxa_len: leg.coxa_length,
            femur_len: leg.femur_length,
            tibia_len: leg.tibia_length,
            coxa_attach_angle: 0.0,
            femur_attach_angle: 0.0,
            tibia_attach_angle: 0.0,
        };

        let mut angles = ik_solve_leg(geom, foot);
        angles.coxa *= COXA_YAW_GAIN;
        leg.set_target_angles(angles.coxa, angles.femur, angles.tibia);

        if !leg_state.is_swing && is_enabled && is_moving {
            let phase_velocity = gait_config.speed * move_magnitude.max(input_state.turn.abs());
            let stride_velocity = -phase_velocity * gait_config.step_length;

            let push_forward =
                -stride_velocity * input_state.move_forward * gait_config.body_push_gain;
            let push_strafe = stride_velocity * input_state.move_strafe * gait_config.body_push_gain;

            body_push.z += push_forward * scale;
            body_push.x += push_strafe * scale;
            stance_count += 1;
        }
    }

    let mut stance_foot_heights: Vec<f32> = Vec::new();

    let all_legs = [
        (&hexapod.legs.left_front, LegId::LeftFront),
        (&hexapod.legs.left_middle, LegId::LeftMiddle),
        (&hexapod.legs.left_back, LegId::LeftBack),
        (&hexapod.legs.right_front, LegId::RightFront),
        (&hexapod.legs.right_middle, LegId::RightMiddle),
        (&hexapod.legs.right_back, LegId::RightBack),
    ];

    let phase_offsets_map: HashMap<_, _> = phase_offsets.iter().cloned().collect();

    for (leg, leg_id) in &all_legs {
        if !gait_config.is_leg_enabled(*leg_id) {
            continue;
        }

        let phase_offset = phase_offsets_map.get(leg_id).copied().unwrap_or(0.0);
        let duty_factor = match gait_config.gait_type {
            GaitType::Ripple => 0.83,
            _ => gait_config.duty_factor,
        };

        let leg_state = calculate_leg_state(walk.phase, phase_offset, duty_factor, true);

        if !leg_state.is_swing {
            let joints = compute_leg_joints(leg, scale);
            stance_foot_heights.push(joints[3].y);
        }
    }

    let body_height = if !stance_foot_heights.is_empty() {
        let lowest_foot_offset = stance_foot_heights
            .iter()
            .cloned()
            .fold(f32::INFINITY, f32::min);
        ground_level - lowest_foot_offset + foot_radius
    } else {
        walk.body_pos.y
    };

    if stance_count > 0 && is_moving {
        let avg_push = body_push / stance_count as f32;

        let yaw = input_state.body_yaw;
        let world_push_x = avg_push.x * yaw.cos() - avg_push.z * yaw.sin();
        let world_push_z = avg_push.x * yaw.sin() + avg_push.z * yaw.cos();

        walk.body_pos.x += world_push_x * dt;
        walk.body_pos.z += world_push_z * dt;
    }
    walk.body_pos.y = body_height;

    StepResult {
        body_pos: walk.body_pos,
        body_yaw: input_state.body_yaw,
        is_moving,
        stance_count,
    }
}

pub fn compute_leg_joints(leg: &Leg, scale: f32) -> [Vec3; 4] {
    let anchor = Vec3::new(
        leg.location.x * scale,
        leg.location.z * scale,
        leg.location.y * scale,
    );

    let coxa_len = leg.coxa_length * scale;
    let femur_len = leg.femur_length * scale;
    let tibia_len = leg.tibia_length * scale;

    let base_yaw = if leg.location.x >= 0.0 { 0.0 } else { std::f32::consts::PI };
    let mount_yaw = leg.mount_angle.to_radians();
    let coxa_yaw = base_yaw + mount_yaw + leg.target_coxa_angle.to_radians();
    let femur_pitch = leg.target_femur_angle.to_radians();
    let tibia_pitch = -leg.target_tibia_angle.to_radians();

    let forward = Vec3::new(coxa_yaw.cos(), 0.0, coxa_yaw.sin());
    let up = Vec3::Y;

    let coxa_end = anchor + forward * coxa_len;
    let femur_dir = forward * femur_pitch.cos() + up * femur_pitch.sin();
    let femur_end = coxa_end + femur_dir * femur_len;

    let tibia_total_pitch = femur_pitch + tibia_pitch;
    let tibia_dir = forward * tibia_total_pitch.cos() + up * tibia_total_pitch.sin();
    let tibia_end = femur_end + tibia_dir * tibia_len;

    [anchor, coxa_end, femur_end, tibia_end]
}

pub fn get_leg_mut(hexapod: &mut Hexapod, leg_id: LegId) -> &mut Leg {
    match leg_id {
        LegId::LeftFront => &mut hexapod.legs.left_front,
        LegId::LeftMiddle => &mut hexapod.legs.left_middle,
        LegId::LeftBack => &mut hexapod.legs.left_back,
        LegId::RightFront => &mut hexapod.legs.right_front,
        LegId::RightMiddle => &mut hexapod.legs.right_middle,
        LegId::RightBack => &mut hexapod.legs.right_back,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_hexapod_idle_keeps_phase() {
        let mut hexapod = Hexapod::new();
        let mut walk = WalkState::default();
        let input = InputState::default();
        let gait = GaitConfig::default();

        let result = step_hexapod(
            &mut hexapod,
            &mut walk,
            &input,
            &gait,
            0.016,
            1.0,
            0.0,
            0.0,
        );

        assert!(!result.is_moving);
        assert_eq!(walk.phase, 0.0);
        assert!(walk.body_pos.is_finite());
    }
}
