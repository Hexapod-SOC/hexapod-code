use bevy::prelude::*;
use movement::controller::GaitController;
use movement::gaits::GAITS;
use movement::ik::SimpleIK;
use movement::legs::Leg;

pub const CONSTRAINTS: movement::ik::Constraints = movement::ik::Constraints {
    coxa_length:  43.0,  // Length of the coxa segment in mm
    femur_length: 60.0,  // Length of the femur segment in mm
    tibia_length: 104.0, // Length of the tibia segment in mm

    coxa_soffset:  0.0, // Offset to align coxa angle to 0 degrees forward
    femur_soffset: 0.0, // Offset to align femur angle to horizontal
    tibia_soffset: 0.0, // Offset to align tibia angle to straight down
};

// Constants matching your IK code
const COXA_LENGTH: f32 = 44.0;
const FEMUR_LENGTH: f32 = 61.0;
const TIBIA_LENGTH: f32 = 100.0;

// Visual scaling for better viewing
const SCALE: f32 = 0.01; // Convert mm to manageable units

// Hexapod body parameters
const BODY_RADIUS: f32 = 60.0;
const BODY_HEIGHT: f32 = 20.0;

// Movement parameters
const BASE_SPEED: f32 = 50.0; // Base movement speed in mm/s
const ROTATION_SPEED: f32 = 0.5; // Rotation speed in radians/s

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(HexapodState::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (update_hexapod, keyboard_input, update_camera))
        .run();
}

#[derive(Resource)]
struct HexapodState {
    controller: GaitController,
    current_gait_index: usize,
    velocity: Vec3,
    rotation: f32,
    paused: bool,
}

impl Default for HexapodState {
    fn default() -> Self {
        let ik = SimpleIK::new(CONSTRAINTS);
        Self {
            controller: GaitController::new(&GAITS[0], ik),
            current_gait_index: 0,
            velocity: Vec3::ZERO,
            rotation: 0.0,
            paused: false,
        }
    }
}

#[derive(Component)]
struct HexapodBody;

#[derive(Component)]
struct LegSegment {
    leg_index: usize,
    segment_type: SegmentType,
}

#[derive(Clone, Copy)]
enum SegmentType {
    Coxa,
    Femur,
    Tibia,
}

#[derive(Component)]
struct LegTarget {
    leg_index: usize,
}

#[derive(Component)]
struct InfoText;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Create materials
    let body_material = materials.add(Color::srgb(0.7, 0.7, 0.7));
    let coxa_material = materials.add(Color::srgb(0.8, 0.2, 0.2));
    let femur_material = materials.add(Color::srgb(0.2, 0.8, 0.2));
    let tibia_material = materials.add(Color::srgb(0.2, 0.2, 0.8));
    let target_material = materials.add(Color::srgb(1.0, 1.0, 0.0));

    // Ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
        Transform::from_xyz(0.0, -2.0, 0.0),
    ));

    // Hexapod body (hexagonal cylinder)
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(BODY_RADIUS * SCALE, BODY_HEIGHT * SCALE))),
        MeshMaterial3d(body_material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        HexapodBody,
    ));

    // Create 6 legs
    let leg_angles: [f32; 6] = [60.0, 0.0, -60.0, -120.0, 180.0, 120.0]; // Angles around the body
    
    for (i, &angle) in leg_angles.iter().enumerate() {
        let angle_rad = angle.to_radians();
        let attachment_x = angle_rad.sin() * BODY_RADIUS * SCALE;
        let attachment_z = angle_rad.cos() * BODY_RADIUS * SCALE;
        
        // Coxa segment
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(2.0 * SCALE, COXA_LENGTH * SCALE))),
            MeshMaterial3d(coxa_material.clone()),
            Transform::from_xyz(attachment_x, 0.0, attachment_z),
            LegSegment {
                leg_index: i,
                segment_type: SegmentType::Coxa,
            },
        ));

        // Femur segment
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(1.5 * SCALE, FEMUR_LENGTH * SCALE))),
            MeshMaterial3d(femur_material.clone()),
            Transform::from_xyz(attachment_x, 0.0, attachment_z),
            LegSegment {
                leg_index: i,
                segment_type: SegmentType::Femur,
            },
        ));

        // Tibia segment
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(1.0 * SCALE, TIBIA_LENGTH * SCALE))),
            MeshMaterial3d(tibia_material.clone()),
            Transform::from_xyz(attachment_x, 0.0, attachment_z),
            LegSegment {
                leg_index: i,
                segment_type: SegmentType::Tibia,
            },
        ));

        // Target indicator (small sphere)
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(3.0 * SCALE))),
            MeshMaterial3d(target_material.clone()),
            Transform::from_xyz(0.0, -0.8, 1.2),
            LegTarget { leg_index: i },
        ));
    }

    // Lighting
    commands.spawn((
        PointLight {
            intensity: 2000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.5, 0.0)),
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-5.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // UI Text - Controls
    commands.spawn((
        Text::new("Controls:\nArrows: Forward/Back/Strafe\nQ/E: Rotate\nG: Cycle Gait\nP: Pause/Play\nR: Reset\nWASD: Camera"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        TextColor(Color::WHITE),
        TextFont {
            font_size: 18.0,
            ..default()
        },
    ));

    // UI Text - Gait info display
    commands.spawn((
        Text::new("Gait: Tripod\nPhase: 0.0\nVelocity: (0, 0)\nRotation: 0.0"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 0.0)),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        InfoText,
    ));
}

fn keyboard_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<HexapodState>,
    time: Res<Time>,
) {
    // Movement controls
    let mut velocity = state.velocity;
    let speed = BASE_SPEED;
    
    // Forward/backward
    if keyboard.pressed(KeyCode::ArrowUp) {
        velocity.x = speed;
    } else if keyboard.pressed(KeyCode::ArrowDown) {
        velocity.x = -speed;
    } else {
        velocity.x = 0.0;
    }
    
    // Strafe left/right
    if keyboard.pressed(KeyCode::ArrowLeft) {
        velocity.y = speed;
    } else if keyboard.pressed(KeyCode::ArrowRight) {
        velocity.y = -speed;
    } else {
        velocity.y = 0.0;
    }
    
    state.velocity = velocity;

    // Rotation controls
    if keyboard.pressed(KeyCode::KeyQ) {
        state.rotation = ROTATION_SPEED;
    } else if keyboard.pressed(KeyCode::KeyE) {
        state.rotation = -ROTATION_SPEED;
    } else {
        state.rotation = 0.0;
    }

    // Cycle through gaits
    if keyboard.just_pressed(KeyCode::KeyG) {
        let new_gait_index = (state.current_gait_index + 1) % GAITS.len();
        state.controller.set_gait(&GAITS[new_gait_index]);
        info!("Switched to gait: {}", GAITS[new_gait_index].name);
        state.current_gait_index = new_gait_index;
    }

    // Pause/unpause
    if keyboard.just_pressed(KeyCode::KeyP) {
        state.paused = !state.paused;
        info!("Gait {}", if state.paused { "paused" } else { "playing" });
    }

    // Reset velocity and rotation
    if keyboard.just_pressed(KeyCode::KeyR) {
        state.velocity = Vec3::ZERO;
        state.rotation = 0.0;
        info!("Reset velocity and rotation");
    }

    // Update gait controller
    if !state.paused {
        state.controller.update(time.delta_secs());
    }
}

fn update_hexapod(
    state: Res<HexapodState>,
    mut leg_query: Query<(&mut Transform, &LegSegment)>,
    mut target_query: Query<(&mut Transform, &LegTarget), Without<LegSegment>>,
    mut text_query: Query<&mut Text, With<InfoText>>,
) {
    // Leg angles around the body (in degrees)
    let leg_angles_global: [f32; 6] = [60.0, 0.0, -60.0, -120.0, 180.0, 120.0];
    
    // Leg ordering matching the body angles
    let legs_ordered = [
        Leg::RightFront,   // 60°
        Leg::RightMiddle,  // 0°
        Leg::RightBack,    // -60°
        Leg::LeftBack,     // -120°
        Leg::LeftMiddle,   // 180°
        Leg::LeftFront,    // 120°
    ];

    // Update UI text with gait info
    for mut text in text_query.iter_mut() {
        let gait_template = state.controller.get_template();
        let phase = state.controller.get_gait_phase();
        let status = if state.paused { " [PAUSED]" } else { "" };
        
        **text = format!(
            "Gait: {}{}\nPhase: {:.2}\nVelocity: ({:.0}, {:.0}) mm/s\nRotation: {:.2} rad/s",
            gait_template.name,
            status,
            phase,
            state.velocity.x,
            state.velocity.y,
            state.rotation
        );
    }

    // Get leg angles from gait controller
    let leg_angles = state.controller.calculate_walking_angles(state.velocity, state.rotation);

    // Calculate and apply leg positions
    for (leg_idx, leg_type) in legs_ordered.iter().enumerate() {
        // Find the angles for this leg
        let angles = leg_angles.iter()
            .find(|(leg, _)| leg == leg_type)
            .map(|(_, angles)| angles)
            .unwrap();
        
        let body_angle = leg_angles_global[leg_idx];
        let body_angle_rad = body_angle.to_radians();
        let attachment_x = body_angle_rad.sin() * BODY_RADIUS * SCALE;
        let attachment_z = body_angle_rad.cos() * BODY_RADIUS * SCALE;
        let attachment_point = Vec3::new(attachment_x, 0.0, attachment_z);

        // Calculate coxa angle in world space
        let coxa_world_angle = (angles.coxa + body_angle).to_radians();
        
        // Calculate joint positions
        let coxa_end = attachment_point + Vec3::new(
            coxa_world_angle.sin() * COXA_LENGTH * SCALE,
            0.0,
            coxa_world_angle.cos() * COXA_LENGTH * SCALE,
        );
        
        let femur_servo_angle_rad = angles.femur.to_radians();
        
        let femur_end = coxa_end + Vec3::new(
            coxa_world_angle.sin() * femur_servo_angle_rad.cos() * FEMUR_LENGTH * SCALE,
            femur_servo_angle_rad.sin() * FEMUR_LENGTH * SCALE,
            coxa_world_angle.cos() * femur_servo_angle_rad.cos() * FEMUR_LENGTH * SCALE,
        );
        
        let tibia_servo_angle_rad = femur_servo_angle_rad - (180.0_f32 - angles.tibia).to_radians();
        
        let tibia_end = femur_end + Vec3::new(
            coxa_world_angle.sin() * tibia_servo_angle_rad.cos() * TIBIA_LENGTH * SCALE,
            tibia_servo_angle_rad.sin() * TIBIA_LENGTH * SCALE,
            coxa_world_angle.cos() * tibia_servo_angle_rad.cos() * TIBIA_LENGTH * SCALE,
        );

        // Update target indicator (foot position)
        for (mut transform, target) in target_query.iter_mut() {
            if target.leg_index == leg_idx {
                transform.translation = tibia_end;
                transform.scale = Vec3::splat(1.0);
            }
        }

        // Update leg segments
        for (mut transform, segment) in leg_query.iter_mut() {
            if segment.leg_index != leg_idx {
                continue;
            }

            match segment.segment_type {
                SegmentType::Coxa => {
                    transform.translation = (attachment_point + coxa_end) * 0.5;
                    let direction = (coxa_end - attachment_point).normalize();
                    transform.rotation = Quat::from_rotation_arc(Vec3::Y, direction);
                }
                SegmentType::Femur => {
                    transform.translation = (coxa_end + femur_end) * 0.5;
                    let direction = (femur_end - coxa_end).normalize();
                    transform.rotation = Quat::from_rotation_arc(Vec3::Y, direction);
                }
                SegmentType::Tibia => {
                    transform.translation = (femur_end + tibia_end) * 0.5;
                    let direction = (tibia_end - femur_end).normalize();
                    transform.rotation = Quat::from_rotation_arc(Vec3::Y, direction);
                }
            }
        }
    }
}

fn update_camera(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
    time: Res<Time>,
) {
    let mut camera_transform = camera_query.single_mut().expect("Camera not found");
    let rotation_speed = 1.0 * time.delta_secs();
    let zoom_speed = 5.0 * time.delta_secs();

    // Camera rotation
    if keyboard.pressed(KeyCode::KeyA) {
        let rotation = Quat::from_rotation_y(rotation_speed);
        camera_transform.rotate_around(Vec3::ZERO, rotation);
    }
    if keyboard.pressed(KeyCode::KeyD) {
        let rotation = Quat::from_rotation_y(-rotation_speed);
        camera_transform.rotate_around(Vec3::ZERO, rotation);
    }

    // Camera zoom
    if keyboard.pressed(KeyCode::KeyW) {
        let direction = (Vec3::ZERO - camera_transform.translation).normalize();
        camera_transform.translation += direction * zoom_speed;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        let direction = (Vec3::ZERO - camera_transform.translation).normalize();
        camera_transform.translation -= direction * zoom_speed;
    }

    // Keep looking at center
    camera_transform.look_at(Vec3::ZERO, Vec3::Y);
}
