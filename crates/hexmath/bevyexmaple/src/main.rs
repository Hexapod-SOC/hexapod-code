mod camera;
mod gait;

use bevy::prelude::*;
use bevy::input::gamepad::{GamepadConnection, GamepadEvent};
use bevy_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use camera::{CameraPlugin, CameraSettingsPlugin, ScenePlugin};
use bevy_rapier3d::prelude::*;
use hexmath::{compute_leg_joints, step_hexapod, GaitConfig, InputState, WalkState};
use hexmath::hexapod::{Hexapod, Leg};
use gait::{GaitDisplayInfo, gait_input_system};

const SCALE: f32 = 1.0 / 100.0; // scale from mm to bevy units (meters)

#[derive(Resource)]
struct BodyEntity(Entity);

#[derive(Resource, Default, Clone, Debug)]
struct InputStateRes(InputState);

#[derive(Resource, Clone, Debug)]
struct WalkStateRes(WalkState);

impl Default for WalkStateRes {
    fn default() -> Self {
        Self(WalkState::default())
    }
}

#[derive(Resource, Clone, Debug)]
struct GaitConfigRes(GaitConfig);

impl Default for GaitConfigRes {
    fn default() -> Self {
        Self(GaitConfig::default())
    }
}

#[derive(Resource)]
struct HexapodState(Hexapod);

#[derive(Resource)]
struct SimulationSpeed {
    value: f32,
    min: f32,
    max: f32,
}

/// Marker component for the camera that follows the hexapod
#[derive(Component)]
struct FollowCamera {
    offset: Vec3,  // Offset from target (x, y, z) where y is height
}




fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(RapierDebugRenderPlugin::default())
        // Example code plugins
        .add_plugins((CameraPlugin, CameraSettingsPlugin, ScenePlugin))
        .init_resource::<InputStateRes>()
        .init_resource::<GaitConfigRes>()
        .init_resource::<GaitDisplayInfo>()
        .insert_resource(SimulationSpeed { value: 1.0, min: 0.1, max: 2.0 })
        .add_systems(Startup, setup)
        .add_systems(PostStartup, (setup_follow_camera, setup_gait_ui, setup_angles_ui, setup_speed_ui))
        .add_systems(Update, (
            gait_input_system,
            handle_gait_tuning_input,
            handle_input_system,
            walk_and_render_system,
            camera_follow_system,
            update_gait_ui,
            update_angles_ui,
            update_speed_ui,
            handle_speed_slider_input,
            handle_speed_keybinds,
        ).chain())
        .run();
}

/// Setup the camera to follow the hexapod
fn setup_follow_camera(
    mut commands: Commands,
    camera_query: Query<Entity, With<Camera3d>>,
) {
    if let Ok(camera_entity) = camera_query.single() {
        commands.entity(camera_entity).insert(FollowCamera {
            offset: Vec3::new(-2.5, 4.5, 9.0), // Camera offset from hexapod
        });
    }
}

/// System to make camera follow the hexapod
fn camera_follow_system(
    walk: Res<WalkStateRes>,
    mut camera_query: Query<(&mut Transform, &FollowCamera), With<Camera3d>>,
) {
    if let Ok((mut camera_transform, follow)) = camera_query.single_mut() {
        // Follow hexapod's X and Z position, keep camera's own height offset
        let target_pos = Vec3::new(
            walk.0.body_pos.x + follow.offset.x,
            follow.offset.y,  // Keep fixed height
            walk.0.body_pos.z + follow.offset.z,
        );
        
        // Smoothly interpolate camera position
        camera_transform.translation = camera_transform.translation.lerp(target_pos, 0.1);
        
        // Look at the hexapod
        camera_transform.look_at(walk.0.body_pos, Vec3::Y);
    }
}

/// Marker component for gait UI text
#[derive(Component)]
struct GaitUiText;

/// Marker component for joint angles UI text
#[derive(Component)]
struct JointAnglesUiText;

#[derive(Component)]
struct SpeedSliderTrack;

#[derive(Component)]
struct SpeedSliderHandle;

#[derive(Component)]
struct SpeedSliderText;

/// Setup the gait display UI
fn setup_gait_ui(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        GaitUiText,
    ));
}

/// Setup the joint angles display UI
fn setup_angles_ui(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.9, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            ..default()
        },
        JointAnglesUiText,
    ));
}

fn setup_speed_ui(mut commands: Commands) {
    const TRACK_WIDTH: f32 = 220.0;
    const TRACK_HEIGHT: f32 = 18.0;
    const HANDLE_WIDTH: f32 = 10.0;
    const HANDLE_HEIGHT: f32 = 22.0;

    commands.spawn((
        Text::new("Sim Speed: 1.00x"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(16.0),
            left: Val::Px(10.0),
            ..default()
        },
        SpeedSliderText,
    ));

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(40.0),
            left: Val::Px(10.0),
            width: Val::Px(TRACK_WIDTH),
            height: Val::Px(TRACK_HEIGHT),
            ..default()
        },
        BackgroundColor(Color::srgb(0.2, 0.2, 0.25)),
        BorderRadius::all(Val::Px(6.0)),
        Interaction::default(),
        SpeedSliderTrack,
    )).with_children(|parent| {
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(-(HANDLE_HEIGHT - TRACK_HEIGHT) * 0.5),
                width: Val::Px(HANDLE_WIDTH),
                height: Val::Px(HANDLE_HEIGHT),
                ..default()
            },
            BackgroundColor(Color::srgb(0.9, 0.7, 0.3)),
            BorderRadius::all(Val::Px(4.0)),
            SpeedSliderHandle,
        ));
    });
}

/// Update the gait UI display
fn update_gait_ui(
    gait_config: Res<GaitConfigRes>,
    display_info: Res<GaitDisplayInfo>,
    mut query: Query<&mut Text, With<GaitUiText>>,
) {
    if let Ok(mut text) = query.single_mut() {
        **text = format!(
            "Gait: {} (1/2/3/4 or Tab to switch)\n\
             Legs: {}/6 enabled\n\
             Disabled: {}\n\
             Gait Speed: {:.1}x\n\
             Step L: {:.0}  Step H: {:.0}\n\
             Base H: {:.0}  Duty: {:.2}\n\
             Push: {:.2}\n\
             \n\
             Controls:\n\
             1=Tripod 2=Tetrapod 3=Wave 4=Ripple\n\
             Z/X StepL  C/V StepH  B/N BaseH\n\
             G/H Duty   J/K Push   ,/. GaitSpeed\n\
             WASD - Move | QE - Turn\n\
             F1-F6 - Toggle legs",
            display_info.current_gait_name,
            display_info.enabled_legs,
            display_info.disabled_legs_list,
            gait_config.0.speed,
            gait_config.0.step_length,
            gait_config.0.step_height,
            gait_config.0.base_height,
            gait_config.0.duty_factor,
            gait_config.0.body_push_gain,
        );
    }
}

fn handle_gait_tuning_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut gait_config: ResMut<GaitConfigRes>,
) {
    const STEP_LEN_STEP: f32 = 5.0;
    const STEP_H_STEP: f32 = 2.0;
    const BASE_H_STEP: f32 = 2.0;
    const DUTY_STEP: f32 = 0.02;
    const PUSH_STEP: f32 = 0.1;
    const GAIT_SPEED_STEP: f32 = 0.1;

    if keyboard.just_pressed(KeyCode::KeyZ) {
        gait_config.0.step_length = (gait_config.0.step_length - STEP_LEN_STEP).max(5.0);
    }
    if keyboard.just_pressed(KeyCode::KeyX) {
        gait_config.0.step_length = (gait_config.0.step_length + STEP_LEN_STEP).min(120.0);
    }

    if keyboard.just_pressed(KeyCode::KeyC) {
        gait_config.0.step_height = (gait_config.0.step_height - STEP_H_STEP).max(1.0);
    }
    if keyboard.just_pressed(KeyCode::KeyV) {
        gait_config.0.step_height = (gait_config.0.step_height + STEP_H_STEP).min(80.0);
    }

    if keyboard.just_pressed(KeyCode::KeyB) {
        gait_config.0.base_height = (gait_config.0.base_height - BASE_H_STEP).max(-120.0);
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        gait_config.0.base_height = (gait_config.0.base_height + BASE_H_STEP).min(-10.0);
    }

    if keyboard.just_pressed(KeyCode::KeyG) {
        gait_config.0.duty_factor = (gait_config.0.duty_factor - DUTY_STEP).clamp(0.2, 0.95);
    }
    if keyboard.just_pressed(KeyCode::KeyH) {
        gait_config.0.duty_factor = (gait_config.0.duty_factor + DUTY_STEP).clamp(0.2, 0.95);
    }

    if keyboard.just_pressed(KeyCode::KeyJ) {
        gait_config.0.body_push_gain = (gait_config.0.body_push_gain - PUSH_STEP).clamp(0.1, 10.0);
    }
    if keyboard.just_pressed(KeyCode::KeyK) {
        gait_config.0.body_push_gain = (gait_config.0.body_push_gain + PUSH_STEP).clamp(0.1, 10.0);
    }

    if keyboard.just_pressed(KeyCode::Comma) {
        gait_config.0.speed = (gait_config.0.speed - GAIT_SPEED_STEP).clamp(0.1, 5.0);
    }
    if keyboard.just_pressed(KeyCode::Period) {
        gait_config.0.speed = (gait_config.0.speed + GAIT_SPEED_STEP).clamp(0.1, 5.0);
    }
}

/// Update the joint angles display UI
fn update_angles_ui(
    hexapod: Res<HexapodState>,
    mut query: Query<&mut Text, With<JointAnglesUiText>>,
) {
    if let Ok(mut text) = query.single_mut() {
        let legs = [
            ("LF", &hexapod.0.legs.left_front),
            ("LM", &hexapod.0.legs.left_middle),
            ("LB", &hexapod.0.legs.left_back),
            ("RF", &hexapod.0.legs.right_front),
            ("RM", &hexapod.0.legs.right_middle),
            ("RB", &hexapod.0.legs.right_back),
        ];

        let mut lines = String::from("Joint Angles\n");
        for (label, leg) in legs {
            lines.push_str(&format!(
                "{}  C:{:>5.1} F:{:>5.1} T:{:>5.1}\n",
                label,
                leg.target_coxa_angle,
                leg.target_femur_angle,
                leg.target_tibia_angle,
            ));
        }

        **text = lines;
    }
}

fn update_speed_ui(
    sim_speed: Res<SimulationSpeed>,
    mut text_query: Query<&mut Text, With<SpeedSliderText>>,
    mut handle_query: Query<&mut Node, With<SpeedSliderHandle>>,
) {
    const TRACK_WIDTH: f32 = 220.0;
    const HANDLE_WIDTH: f32 = 10.0;

    if let Ok(mut text) = text_query.single_mut() {
        **text = format!("Sim Speed: {:.2}x", sim_speed.value);
    }

    let t = ((sim_speed.value - sim_speed.min) / (sim_speed.max - sim_speed.min)).clamp(0.0, 1.0);
    let handle_x = t * (TRACK_WIDTH - HANDLE_WIDTH);

    if let Ok(mut handle_node) = handle_query.single_mut() {
        handle_node.left = Val::Px(handle_x);
    }
}

fn handle_speed_slider_input(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    track_query: Query<&GlobalTransform, With<SpeedSliderTrack>>,
    mut sim_speed: ResMut<SimulationSpeed>,
) {
    const TRACK_WIDTH: f32 = 220.0;
    const TRACK_HEIGHT: f32 = 18.0;

    if !mouse.pressed(MouseButton::Left) {
        return;
    }

    let window = match windows.single() {
        Ok(w) => w,
        Err(_) => return,
    };

    let cursor = match window.cursor_position() {
        Some(pos) => pos,
        None => return,
    };

    for transform in track_query.iter() {
        let track_center = transform.translation().truncate();
        let left = track_center.x - TRACK_WIDTH * 0.5;
        let right = track_center.x + TRACK_WIDTH * 0.5;
        let top = track_center.y + TRACK_HEIGHT * 0.5;
        let bottom = track_center.y - TRACK_HEIGHT * 0.5;

        if cursor.x < left || cursor.x > right || cursor.y < bottom || cursor.y > top {
            continue;
        }

        let t = ((cursor.x - left) / TRACK_WIDTH).clamp(0.0, 1.0);
        sim_speed.value = sim_speed.min + t * (sim_speed.max - sim_speed.min);
    }
}

fn handle_speed_keybinds(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut sim_speed: ResMut<SimulationSpeed>,
) {
    let step = 0.1;

    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        sim_speed.value = (sim_speed.value - step).clamp(sim_speed.min, sim_speed.max);
    }

    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        sim_speed.value = (sim_speed.value + step).clamp(sim_speed.min, sim_speed.max);
    }
}

/// Handle gamepad and keyboard input for controlling the hexapod
fn handle_input_system(
    time: Res<Time>,
    gamepads: Query<&Gamepad>,
    keyboard: Res<ButtonInput<KeyCode>>,
    sim_speed: Res<SimulationSpeed>,
    mut input_state: ResMut<InputStateRes>,
) {
    let dt = time.delta_secs() * sim_speed.value;
    
    // Reset input
    let mut forward = 0.0;
    let mut strafe = 0.0;
    let mut turn = 0.0;
    
    // Try to get gamepad input first
    for gamepad in gamepads.iter() {
        // Left stick for movement
        if let Some(left_stick_y) = gamepad.get(GamepadAxis::LeftStickY) {
            forward = left_stick_y;
        }
        if let Some(left_stick_x) = gamepad.get(GamepadAxis::LeftStickX) {
            strafe = -left_stick_x;
        }
        
        // Right stick X for turning
        if let Some(right_stick_x) = gamepad.get(GamepadAxis::RightStickX) {
            turn = right_stick_x;
        }
        
        // Triggers can also be used for turning
        let left_trigger = gamepad.get(GamepadAxis::LeftZ).unwrap_or(0.0);
        let right_trigger = gamepad.get(GamepadAxis::RightZ).unwrap_or(0.0);
        if turn.abs() < 0.1 {
            turn = right_trigger - left_trigger;
        }
    }
    
    // Keyboard fallback (WASD + QE for turning)
    if forward.abs() < 0.1 && strafe.abs() < 0.1 && turn.abs() < 0.1 {
        if keyboard.pressed(KeyCode::KeyW) {
            forward = 1.0;
        }
        if keyboard.pressed(KeyCode::KeyS) {
            forward = -1.0;
        }
        if keyboard.pressed(KeyCode::KeyA) {
            strafe = -1.0;
        }
        if keyboard.pressed(KeyCode::KeyD) {
            strafe = 1.0;
        }
        if keyboard.pressed(KeyCode::KeyQ) {
            turn = -1.0;
        }
        if keyboard.pressed(KeyCode::KeyE) {
            turn = 1.0;
        }
    }
    
    // Apply deadzone
    if forward.abs() < 0.1 { forward = 0.0; }
    if strafe.abs() < 0.1 { strafe = 0.0; }
    if turn.abs() < 0.15 { turn = 0.0; }
    
    input_state.0.move_forward = forward;
    input_state.0.move_strafe = strafe;
    input_state.0.turn = turn;
    
    // Update body yaw based on turn input
    let turn_speed = 0.5; // radians per second
    input_state.0.body_yaw += turn * turn_speed * dt;
}

/// this component indicates what entities should rotate
#[derive(Component)]
struct Rotator;

/// rotates the parent, which will result in the child also rotating
fn rotator_system(time: Res<Time>, mut query: Query<&mut Transform, With<Rotator>>) {
    for mut transform in &mut query {
        transform.rotate_x(3.0 * time.delta_secs());
    }
}

/// set up a simple scene with a "parent" cube and a "child" cube
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let cube_handle = meshes.add(Cuboid::new(2.0, 2.0, 2.0));
    let cube_material_handle = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.7, 0.6),
        ..default()
    });

    // platform
    let platform_radius = 14.0;
    let platform_thickness = 0.1;
    let checker = create_checkerboard(&mut images, 8, 8, 64, Color::srgb(0.9, 0.9, 0.9), Color::srgb(0.2, 0.2, 0.2));
    let platform_material = materials.add(StandardMaterial {
        base_color_texture: Some(checker),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(
            platform_radius * 2.0,
            platform_thickness,
            platform_radius * 2.0,
        ))),
        MeshMaterial3d(platform_material),
        Transform::from_xyz(0.0, -platform_thickness * 0.5, 0.0),
        RigidBody::Fixed,
        Collider::cuboid(platform_radius, platform_thickness * 0.5, platform_radius),
        Friction::coefficient(1.0),
        Restitution::coefficient(0.0),
    ));



    let hexapod = Hexapod::new();

    let base_height = -70.0;
    let body_height = (-base_height * SCALE).max(0.2);

    let body = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(
                hexapod.dimensions.body_width * SCALE,
                hexapod.dimensions.body_height * SCALE,
                hexapod.dimensions.body_length * SCALE,
            ))),
            MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
            Transform::from_xyz(0.0, body_height, 0.0),
        ))
        .id();

    render_legs(body, &hexapod, &mut commands, &mut meshes, &mut materials);

    commands.insert_resource(BodyEntity(body));
    commands.insert_resource(WalkStateRes(WalkState {
        time: 0.0,
        phase: 0.0,
        body_pos: Vec3::new(0.0, body_height, 0.0),
        body_velocity: Vec3::ZERO,
    }));
    commands.insert_resource(HexapodState(hexapod));
}

fn create_checkerboard(
    images: &mut Assets<Image>,
    squares_x: u32,
    squares_y: u32,
    pixels_per_square: u32,
    light: Color,
    dark: Color,
) -> Handle<Image> {
    let width = squares_x * pixels_per_square;
    let height = squares_y * pixels_per_square;
    let mut data = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            let sx = x / pixels_per_square;
            let sy = y / pixels_per_square;
            let use_light = (sx + sy) % 2 == 0;
            let color = if use_light { light } else { dark };
            let [r, g, b, a] = color.to_srgba().to_u8_array();
            data.extend_from_slice(&[r, g, b, a]);
        }
    }

    let image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );

    images.add(image)
}

fn walk_and_render_system(
    time: Res<Time>,
    input_state: Res<InputStateRes>,
    gait_config: Res<GaitConfigRes>,
    sim_speed: Res<SimulationSpeed>,
    mut walk: ResMut<WalkStateRes>,
    mut hexapod: ResMut<HexapodState>,
    body: Res<BodyEntity>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    children_query: Query<&Children>,
    mut body_query: Query<&mut Transform>,
) {
    let dt = time.delta_secs() * sim_speed.value;

    let _ = step_hexapod(
        &mut hexapod.0,
        &mut walk.0,
        &input_state.0,
        &gait_config.0,
        dt,
        SCALE,
        0.0,
        0.03,
    );

    if let Ok(mut transform) = body_query.get_mut(body.0) {
        transform.translation = walk.0.body_pos;
        transform.rotation = Quat::from_rotation_y(input_state.0.body_yaw);
    }

    clear_body_children(body.0, &mut commands, &children_query);
    render_legs(body.0, &hexapod.0, &mut commands, &mut meshes, &mut materials);
}

fn render_legs(
    body: Entity,
    hexapod: &Hexapod,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(body).with_children(|parent| {
        let joint_radius = 0.05;
        let capsule_radius = 0.035;

        let joint_color = Color::srgb_u8(250, 230, 160);
        let capsule_color = Color::srgb_u8(200, 200, 220);
        let joint_mesh_handle = meshes.add(Sphere::new(joint_radius));

        let mut spawn_leg_from_angles = |leg: &Leg| {
            let all_joints = compute_leg_joints(leg, SCALE);
            let anchor = all_joints[0];
            let coxa_end = all_joints[1];
            let femur_end = all_joints[2];
            let tibia_end = all_joints[3];
            for joint in all_joints.iter() {
                parent.spawn((
                    Mesh3d(joint_mesh_handle.clone()),
                    MeshMaterial3d(materials.add(joint_color)),
                    Transform::from_translation(*joint),
                ));
            }

            let segments = [
                (anchor, coxa_end),
                (coxa_end, femur_end),
                (femur_end, tibia_end),
            ];

            for (start, end) in segments {
                let direction = end - start;
                let length = direction.length();
                if length <= f32::EPSILON {
                    continue;
                }

                let midpoint = start + direction * 0.5;
                let rotation = Quat::from_rotation_arc(Vec3::Y, direction.normalize());

                parent.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(capsule_radius, length))),
                    MeshMaterial3d(materials.add(capsule_color)),
                    Transform::from_translation(midpoint).with_rotation(rotation),
                ));
            }
        };

        let legs = [
            &hexapod.legs.left_front,
            &hexapod.legs.left_middle,
            &hexapod.legs.left_back,
            &hexapod.legs.right_front,
            &hexapod.legs.right_middle,
            &hexapod.legs.right_back,
        ];

        for leg in legs {
            spawn_leg_from_angles(leg);
        }
    });
}



fn clear_body_children(body: Entity, commands: &mut Commands, children_query: &Query<&Children>) {
    if let Ok(children) = children_query.get(body) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
}
