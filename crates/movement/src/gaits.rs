pub struct LegCycleOffsets {
    pub left_front: f32,
    pub left_middle: f32,
    pub left_back: f32,
    pub right_front: f32,
    pub right_middle: f32,
    pub right_back: f32,
}

pub struct GaitTemplate {
    pub name: &'static str,
    pub leg_cycle_offsets: LegCycleOffsets,
    pub push_fraction: f32,
    pub speed_multiplier: f32,
    pub step_length_multiplier: f32,
    pub lift_height_multiplier: f32,
    pub max_step_length: f32,
    pub max_speed: f32,
}

const GAIT_TRI: GaitTemplate = GaitTemplate {
    name: "tri",
    leg_cycle_offsets: LegCycleOffsets {
        left_front: 0.0,
        left_middle: 0.5,
        left_back: 0.0,
        right_front: 0.5,
        right_middle: 0.0,
        right_back: 0.5,
    },
    push_fraction: 3.75 / 6.0,
    speed_multiplier: 1.0,
    step_length_multiplier: 0.75,
    lift_height_multiplier: 1.0,
    max_step_length: 240.0,
    max_speed: 200.0,
};

const GAIT_WAVE: GaitTemplate = GaitTemplate {
    name: "wave",
    leg_cycle_offsets: LegCycleOffsets {
        left_front: 0.0,
        left_middle: 1.0 / 6.0,
        left_back: 2.0 / 6.0,
        right_front: 5.0 / 6.0,
        right_middle: 4.0 / 6.0,
        right_back: 3.0 / 6.0,
    },
    push_fraction: 4.9 / 6.0,
    speed_multiplier: 0.40,
    step_length_multiplier: 2.0,
    lift_height_multiplier: 1.2,
    max_step_length: 150.0,
    max_speed: 160.0,
};

const GAIT_RIPPLE: GaitTemplate = GaitTemplate {
    name: "ripple",
    leg_cycle_offsets: LegCycleOffsets {
        left_front: 0.0,
        left_middle: 4.0 / 6.0,
        left_back: 2.0 / 6.0,
        right_front: 5.0 / 6.0,
        right_middle: 1.0 / 6.0,
        right_back: 3.0 / 6.0,
    },
    push_fraction: 3.2 / 6.0,
    speed_multiplier: 1.0,
    step_length_multiplier: 1.3,
    lift_height_multiplier: 1.1,
    max_step_length: 220.0,
    max_speed: 200.0,
};

const GAIT_BI: GaitTemplate = GaitTemplate {
    name: "bi",
    leg_cycle_offsets: LegCycleOffsets {
        left_front: 0.0,
        left_middle: 1.0 / 3.0,
        left_back: 2.0 / 3.0,
        right_front: 0.0,
        right_middle: 1.0 / 3.0,
        right_back: 2.0 / 3.0,
    },
    push_fraction: 2.1 / 6.0,
    speed_multiplier: 4.0,
    step_length_multiplier: 1.0,
    lift_height_multiplier: 1.8,
    max_step_length: 230.0,
    max_speed: 130.0,
};

const GAIT_QUAD: GaitTemplate = GaitTemplate {
    name: "quad",
    leg_cycle_offsets: LegCycleOffsets {
        left_front: 0.0,
        left_middle: 1.0 / 3.0,
        left_back: 2.0 / 3.0,
        right_front: 0.0,
        right_middle: 1.0 / 3.0,
        right_back: 2.0 / 3.0,
    },
    push_fraction: 4.1 / 6.0,
    speed_multiplier: 1.0,
    step_length_multiplier: 1.2,
    lift_height_multiplier: 1.1,
    max_step_length: 220.0,
    max_speed: 200.0,
};

const GAIT_HOP: GaitTemplate = GaitTemplate {
    name: "hop",
    leg_cycle_offsets: LegCycleOffsets {
        left_front: 0.0,
        left_middle: 0.0,
        left_back: 0.0,
        right_front: 0.0,
        right_middle: 0.0,
        right_back: 0.0,
    },
    push_fraction: 3.0 / 6.0,
    speed_multiplier: 1.0,
    step_length_multiplier: 1.6,
    lift_height_multiplier: 2.5,
    max_step_length: 240.0,
    max_speed: 200.0,
};

pub const GAITS: [GaitTemplate; 6] = [
    GAIT_TRI,
    GAIT_WAVE,
    GAIT_RIPPLE,
    GAIT_BI,
    GAIT_QUAD,
    GAIT_HOP,
];