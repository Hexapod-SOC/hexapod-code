use bevy::prelude::*;
use hexmath::{GaitConfig, GaitType};
use crate::GaitConfigRes;
use hexmath::hexapod::LegId;

/// UI display info for current gait
#[derive(Resource, Default)]
pub struct GaitDisplayInfo {
    pub current_gait_name: String,
    pub enabled_legs: usize,
    pub disabled_legs_list: String,
}

impl GaitDisplayInfo {
    pub fn update(&mut self, config: &GaitConfig) {
        self.current_gait_name = config.gait_type.name().to_string();
        self.enabled_legs = config.enabled_leg_count();

        let disabled: Vec<String> = config
            .disabled_legs
            .iter()
            .map(|l| format!("{:?}", l))
            .collect();

        self.disabled_legs_list = if disabled.is_empty() {
            "None".to_string()
        } else {
            disabled.join(", ")
        };
    }
}

/// Key bindings for gait control
pub struct GaitKeyBindings {
    /// Key to cycle through gait types
    pub cycle_gait: KeyCode,
    /// Key for tripod gait
    pub tripod: KeyCode,
    /// Key for tetrapod gait
    pub tetrapod: KeyCode,
    /// Key for wave gait
    pub wave: KeyCode,
    /// Key for ripple gait
    pub ripple: KeyCode,
    /// Keys to toggle individual legs (1-6)
    pub toggle_legs: [KeyCode; 6],
}

impl Default for GaitKeyBindings {
    fn default() -> Self {
        Self {
            cycle_gait: KeyCode::Tab,
            tripod: KeyCode::Digit1,
            tetrapod: KeyCode::Digit2,
            wave: KeyCode::Digit3,
            ripple: KeyCode::Digit4,
            toggle_legs: [
                KeyCode::F1,
                KeyCode::F2,
                KeyCode::F3,
                KeyCode::F4,
                KeyCode::F5,
                KeyCode::F6,
            ],
        }
    }
}

/// System to handle gait switching via keyboard
pub fn gait_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut gait_config: ResMut<GaitConfigRes>,
    mut display_info: ResMut<GaitDisplayInfo>,
) {
    let bindings = GaitKeyBindings::default();

    if keyboard.just_pressed(bindings.cycle_gait) {
        gait_config.0.gait_type = gait_config.0.gait_type.next();
        info!("Switched to {} gait", gait_config.0.gait_type.name());
    }

    if keyboard.just_pressed(bindings.tripod) {
        gait_config.0.gait_type = GaitType::Tripod;
        info!("Switched to Tripod gait");
    }
    if keyboard.just_pressed(bindings.tetrapod) {
        gait_config.0.gait_type = GaitType::Tetrapod;
        info!("Switched to Tetrapod gait");
    }
    if keyboard.just_pressed(bindings.wave) {
        gait_config.0.gait_type = GaitType::Wave;
        info!("Switched to Wave gait");
    }
    if keyboard.just_pressed(bindings.ripple) {
        gait_config.0.gait_type = GaitType::Ripple;
        info!("Switched to Ripple gait");
    }

    let leg_ids = [
        LegId::LeftFront,
        LegId::LeftMiddle,
        LegId::LeftBack,
        LegId::RightFront,
        LegId::RightMiddle,
        LegId::RightBack,
    ];

    for (i, key) in bindings.toggle_legs.iter().enumerate() {
        if keyboard.just_pressed(*key) {
            let leg = leg_ids[i];
            gait_config.0.toggle_leg(leg);
            let state = if gait_config.0.is_leg_enabled(leg) {
                "enabled"
            } else {
                "disabled"
            };
            info!("{:?} {}", leg, state);
        }
    }

    display_info.update(&gait_config.0);
}
        // Auto-switch to tetrapod if a leg is disabled
