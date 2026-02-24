use crate::hexapod::{LegStances, ServoAngleTriplet, ServoAngleTweaks};
use crate::config::{
    calibration_gait_configs_path, calibration_leg_stance_path, calibration_servo_tweaks_path,
};
use glam::Vec3;
use hexmath::{GaitConfig, GaitType};
use std::collections::HashMap;
use std::path::PathBuf;

/// Convert a Unix timestamp (seconds) to (year, month, day, hour, min, sec) UTC.
/// Uses Howard Hinnant's civil calendar algorithm. No external crate required.
pub fn epoch_to_datetime(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let sec  = (secs % 60) as u32;
    let min  = ((secs / 60) % 60) as u32;
    let hour = ((secs / 3600) % 24) as u32;

    // Shift days-since-1970-01-01 to days-since-0000-03-01 (Hinnant's civil epoch)
    let z: i64 = (secs / 86400) as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;                          // day of era  [0, 146096]
    let yoe = (doe - doe/1460 + doe/36524 - doe/146096) / 365;   // year of era [0, 399]
    let y   = yoe as i64 + era * 400;                             // year (proleptic Gregorian)
    let doy = doe - (365*yoe + yoe/4 - yoe/100);                  // day of year from Mar 1 [0, 365]
    let mp  = (5*doy + 2) / 153;                                  // month part  [0, 11]
    let day = (doy - (153*mp + 2)/5 + 1) as u32;                 // day         [1, 31]
    let month = if mp < 10 { (mp + 3) as u32 } else { (mp - 9) as u32 };
    let year  = if month <= 2 { y + 1 } else { y } as u32;

    (year, month, day, hour, min, sec)
}

pub fn load_saved_servo_tweaks() -> Option<ServoAngleTweaks> {
    let path = calibration_servo_tweaks_path();
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    #[derive(serde::Deserialize)]
    struct FileTweaks {
        left_front: [f32; 3],
        left_middle: [f32; 3],
        left_back: [f32; 3],
        right_front: [f32; 3],
        right_middle: [f32; 3],
        right_back: [f32; 3],
    }
    let parsed: FileTweaks = serde_json::from_str(&content).ok()?;
    Some(ServoAngleTweaks {
        left_front: ServoAngleTriplet {
            coxa: parsed.left_front[0],
            femur: parsed.left_front[1],
            tibia: parsed.left_front[2],
        },
        left_middle: ServoAngleTriplet {
            coxa: parsed.left_middle[0],
            femur: parsed.left_middle[1],
            tibia: parsed.left_middle[2],
        },
        left_back: ServoAngleTriplet {
            coxa: parsed.left_back[0],
            femur: parsed.left_back[1],
            tibia: parsed.left_back[2],
        },
        right_front: ServoAngleTriplet {
            coxa: parsed.right_front[0],
            femur: parsed.right_front[1],
            tibia: parsed.right_front[2],
        },
        right_middle: ServoAngleTriplet {
            coxa: parsed.right_middle[0],
            femur: parsed.right_middle[1],
            tibia: parsed.right_middle[2],
        },
        right_back: ServoAngleTriplet {
            coxa: parsed.right_back[0],
            femur: parsed.right_back[1],
            tibia: parsed.right_back[2],
        },
    })
}
pub fn load_saved_leg_stance() -> Option<LegStances> {
    let path = calibration_leg_stance_path();
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    #[derive(serde::Deserialize)]
    struct FileStance {
        left_front: [f32; 3],
        left_middle: [f32; 3],
        left_back: [f32; 3],
        right_front: [f32; 3],
        right_middle: [f32; 3],
        right_back: [f32; 3],
    }
    let parsed: FileStance = serde_json::from_str(&content).ok()?;
    Some(LegStances {
        left_front: Vec3::from_array(parsed.left_front),
        left_middle: Vec3::from_array(parsed.left_middle),
        left_back: Vec3::from_array(parsed.left_back),
        right_front: Vec3::from_array(parsed.right_front),
        right_middle: Vec3::from_array(parsed.right_middle),
        right_back: Vec3::from_array(parsed.right_back),
    })
}

#[derive(serde::Deserialize, Clone)]
pub struct FileGaitConfig {
    duty_factor: f32,
    speed: f32,
    step_length_mm: f32,
    step_height_mm: f32,
    base_height_mm: f32,
    body_push_gain: f32,
    phase_offsets: [f32; 6],
    max_step_length: f32,
    max_speed: f32,
}

pub fn parse_gait_name(name: &str) -> Option<GaitType> {
    match name.to_lowercase().as_str() {
        "tripod" | "tri" | "t"     => Some(GaitType::Tripod),
        "tetrapod" | "quad" | "bi" => Some(GaitType::Tetrapod),
        "wave" | "w"               => Some(GaitType::Wave),
        "ripple" | "r"             => Some(GaitType::Ripple),
        "crawl" | "c" | "slope"    => Some(GaitType::Crawl),
        _ => None,
    }
}

pub fn gait_config_from_file(gait_type: GaitType, file: &FileGaitConfig) -> GaitConfig {
    let mut config = GaitConfig::default();
    let mut offsets = file.phase_offsets;
    for value in offsets.iter_mut() {
        *value = value.clamp(0.0, 1.0);
    }

    config.gait_type = gait_type;
    config.phase_offsets_override = Some(offsets);
    config.duty_factor = file.duty_factor.clamp(0.05, 0.95);
    config.step_length = file
        .step_length_mm
        .clamp(0.0, file.max_step_length.max(0.0));
    config.step_height = file.step_height_mm.max(0.0);
    config.speed = file.speed.clamp(0.1, file.max_speed.max(0.1));
    config.base_height = file.base_height_mm.clamp(-300.0, 0.0);
    config.body_push_gain = file.body_push_gain.clamp(0.0, 10.0);
    config
}

pub fn load_saved_gait_configs() -> HashMap<GaitType, FileGaitConfig> {
    let path = calibration_gait_configs_path();
    if !path.exists() {
        return HashMap::new();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let parsed: HashMap<String, FileGaitConfig> = match serde_json::from_str(&content) {
        Ok(p) => p,
        Err(_) => return HashMap::new(),
    };

    let mut result = HashMap::new();
    for (name, cfg) in parsed {
        if let Some(gait_type) = parse_gait_name(&name) {
            result.insert(gait_type, cfg);
        }
    }
    result
}
