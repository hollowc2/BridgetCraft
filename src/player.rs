use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use bevy_voxel_world::prelude::*;

use crate::audio::{spatial_audio_listener, GameAudio};
use crate::block::BlockId;
use crate::gamepad::select_primary;
use crate::voxel_config::BridgetWorld;
use crate::world_gen::{terrain_surface_height, terrain_voxel_lookup};

pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_RADIUS: f32 = 0.35;
pub const WALK_SPEED: f32 = 6.0;
pub const FLY_SPEED: f32 = 10.0;
pub const JUMP_SPEED: f32 = 8.0;
pub const GRAVITY: f32 = 22.0;
pub const MOUSE_SENSITIVITY: f32 = 0.002;
pub const GAMEPAD_LOOK_SENSITIVITY: f32 = 2.5;
pub const GAMEPAD_MOVE_DEADZONE: f32 = 0.15;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Component, Default)]
pub struct PlayerController {
    pub yaw: f32,
    pub pitch: f32,
    pub velocity: Vec3,
    pub grounded: bool,
    pub flying: bool,
    pub fly_toggle_cooldown: f32,
    pub footstep_timer: Timer,
}

impl PlayerController {
    fn new() -> Self {
        Self {
            footstep_timer: Timer::from_seconds(0.42, TimerMode::Repeating),
            ..Default::default()
        }
    }
}

#[derive(Resource)]
pub struct PlayerSettings {
    pub mouse_sensitivity: f32,
    pub gamepad_look_sensitivity: f32,
    pub render_distance: u32,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: MOUSE_SENSITIVITY,
            gamepad_look_sensitivity: GAMEPAD_LOOK_SENSITIVITY,
            render_distance: 6,
        }
    }
}

pub fn spawn_player(commands: &mut Commands, name: &str, position: Vec3) -> Entity {
    let camera = commands
        .spawn((
            Camera3d::default(),
            Camera {
                order: 0,
                ..default()
            },
            PlayerCamera,
            spatial_audio_listener(),
            VoxelWorldCamera::<BridgetWorld>::default(),
            Transform::from_xyz(0.0, PLAYER_HEIGHT - 0.2, 0.0),
        ))
        .id();

    commands
        .spawn((
            Player,
            PlayerController::new(),
            Transform::from_translation(position),
            Visibility::default(),
            Name::new(name.to_string()),
        ))
        .add_child(camera)
        .id()
}

pub fn grab_cursor(mut cursor: Single<&mut CursorOptions>) {
    cursor.visible = false;
    cursor.grab_mode = CursorGrabMode::Locked;
}

pub fn release_cursor(mut cursor: Query<&mut CursorOptions>) {
    for mut options in &mut cursor {
        options.visible = true;
        options.grab_mode = CursorGrabMode::None;
    }
}

pub fn mouse_look(
    time: Res<Time>,
    mut motion: MessageReader<MouseMotion>,
    settings: Res<PlayerSettings>,
    gamepads: Query<(&Name, &Gamepad)>,
    mut players: Query<(&mut PlayerController, &Children), With<Player>>,
    mut cameras: Query<&mut Transform, With<PlayerCamera>>,
) {
    let mut delta = Vec2::ZERO;
    for event in motion.read() {
        delta += event.delta;
    }

    if let Some(gamepad) = select_primary(gamepads.iter()) {
        let stick = gamepad.right_stick();
        if stick.length() > GAMEPAD_MOVE_DEADZONE {
            let look = stick * settings.gamepad_look_sensitivity * time.delta_secs();
            delta.x += look.x * 60.0;
            delta.y -= look.y * 60.0;
        }
    }

    if delta == Vec2::ZERO {
        return;
    }

    let Ok((mut controller, children)) = players.single_mut() else {
        return;
    };

    controller.yaw -= delta.x * settings.mouse_sensitivity;
    controller.pitch = (controller.pitch - delta.y * settings.mouse_sensitivity)
        .clamp(-1.54, 1.54);

    for child in children.iter() {
        if let Ok(mut camera_transform) = cameras.get_mut(child) {
            camera_transform.rotation =
                Quat::from_rotation_y(controller.yaw) * Quat::from_rotation_x(controller.pitch);
        }
    }
}

pub fn player_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(&Name, &Gamepad)>,
    metadata: Res<crate::world_gen::WorldMetadata>,
    mut players: Query<(&mut Transform, &mut PlayerController), With<Player>>,
    voxel_world: VoxelWorld<BridgetWorld>,
    mut audio: ResMut<GameAudio>,
    mut commands: Commands,
) {
    let Ok((mut transform, mut controller)) = players.single_mut() else {
        return;
    };

    let gamepad = select_primary(gamepads.iter());
    let jump_pressed = keys.just_pressed(KeyCode::Space)
        || gamepad.is_some_and(|gamepad| gamepad.just_pressed(GamepadButton::South));

    controller.fly_toggle_cooldown = (controller.fly_toggle_cooldown - time.delta_secs()).max(0.0);

    if controller.fly_toggle_cooldown <= 0.0 && jump_pressed {
        let now = time.elapsed_secs();
        // Double-tap detection via cooldown trick: if space pressed while still grounded recently
        let jump_held = keys.pressed(KeyCode::Space)
            || gamepad.is_some_and(|gamepad| gamepad.pressed(GamepadButton::South));
        if jump_held && controller.grounded {
            controller.flying = !controller.flying;
            controller.fly_toggle_cooldown = 0.35;
            controller.velocity.y = 0.0;
            let _ = now;
        }
    }

    let forward = Vec3::new(-controller.yaw.sin(), 0.0, -controller.yaw.cos());
    let right = Vec3::new(controller.yaw.cos(), 0.0, -controller.yaw.sin());
    let mut wish_dir = Vec3::ZERO;

    if keys.pressed(KeyCode::KeyW) {
        wish_dir += forward;
    }
    if keys.pressed(KeyCode::KeyS) {
        wish_dir -= forward;
    }
    if keys.pressed(KeyCode::KeyA) {
        wish_dir -= right;
    }
    if keys.pressed(KeyCode::KeyD) {
        wish_dir += right;
    }

    if let Some(gamepad) = gamepad {
        let stick = gamepad.left_stick();
        if stick.length() > GAMEPAD_MOVE_DEADZONE {
            wish_dir += forward * -stick.y + right * stick.x;
        }
    }

    let chunk_get_voxel = voxel_world.get_voxel_fn();
    let procedural_get_voxel = terrain_voxel_lookup(metadata.seed);
    let get_voxel = std::sync::Arc::new(move |pos: IVec3| {
        let voxel = chunk_get_voxel(pos);
        if voxel == WorldVoxel::Unset {
            procedural_get_voxel(pos)
        } else {
            voxel
        }
    });

    if controller.flying {
        if keys.pressed(KeyCode::Space)
            || gamepad.is_some_and(|gamepad| gamepad.pressed(GamepadButton::South))
        {
            wish_dir.y += 1.0;
        }
        if keys.pressed(KeyCode::ShiftLeft)
            || gamepad.is_some_and(|gamepad| {
                gamepad.pressed(GamepadButton::LeftTrigger)
                    || gamepad.pressed(GamepadButton::North)
            })
        {
            wish_dir.y -= 1.0;
        }
        if wish_dir != Vec3::ZERO {
            let delta = wish_dir.normalize() * FLY_SPEED * time.delta_secs();
            move_by_delta(&mut transform.translation, delta, &*get_voxel);
        }
        recover_if_below_surface(&mut transform, metadata.seed, &*get_voxel);
        controller.grounded = false;
        controller.velocity = Vec3::ZERO;
        return;
    }

    if wish_dir != Vec3::ZERO {
        wish_dir = wish_dir.normalize();
        controller.velocity.x = wish_dir.x * WALK_SPEED;
        controller.velocity.z = wish_dir.z * WALK_SPEED;
    } else {
        controller.velocity.x = 0.0;
        controller.velocity.z = 0.0;
    }

    if controller.grounded && jump_pressed {
        controller.velocity.y = JUMP_SPEED;
        controller.grounded = false;
    }

    controller.velocity.y -= GRAVITY * time.delta_secs();
    move_with_collision(&mut transform, &mut controller, get_voxel.clone(), time.delta_secs());
    recover_if_below_surface(&mut transform, metadata.seed, &*get_voxel);

    let moving = Vec2::new(controller.velocity.x, controller.velocity.z).length() > 0.5;
    if controller.grounded && moving {
        controller.footstep_timer.tick(time.delta());
        if controller.footstep_timer.just_finished() {
            audio.play_footstep_at_feet(
                &mut commands,
                &voxel_world,
                &metadata,
                transform.translation,
            );
        }
    } else {
        controller.footstep_timer.reset();
    }
}

fn move_by_delta(
    position: &mut Vec3,
    delta: Vec3,
    get_voxel: &(impl Fn(IVec3) -> WorldVoxel<u8> + ?Sized),
) {
    let mut new_pos = *position;

    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        let movement = axis * delta.dot(axis);
        if movement.length_squared() == 0.0 {
            continue;
        }
        let candidate = new_pos + movement;
        if !collides(candidate, get_voxel) {
            new_pos = candidate;
        }
    }

    *position = new_pos;
}

fn recover_if_below_surface(
    transform: &mut Transform,
    seed: u32,
    get_voxel: &(impl Fn(IVec3) -> WorldVoxel<u8> + ?Sized),
) {
    let x = transform.translation.x.floor() as i32;
    let z = transform.translation.z.floor() as i32;
    let min_feet_y = terrain_surface_height(seed, x, z) as f32 + 1.0;

    if transform.translation.y >= min_feet_y && !collides(transform.translation, get_voxel) {
        return;
    }

    transform.translation.y = min_feet_y;
    for offset in 0..=6 {
        let candidate = transform.translation + Vec3::new(0.0, offset as f32, 0.0);
        if !collides(candidate, get_voxel) {
            transform.translation = candidate;
            return;
        }
    }
}

fn move_with_collision(
    transform: &mut Transform,
    controller: &mut PlayerController,
    get_voxel: std::sync::Arc<dyn Fn(IVec3) -> WorldVoxel<u8> + Send + Sync>,
    dt: f32,
) {
    let velocity = controller.velocity * dt;
    let mut new_pos = transform.translation;

    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        let movement = axis * velocity.dot(axis);
        if movement.length_squared() == 0.0 {
            continue;
        }
        let candidate = new_pos + movement;
        if !collides(candidate, &*get_voxel) {
            new_pos = candidate;
        } else if axis == Vec3::Y && movement.y < 0.0 {
            controller.grounded = true;
            controller.velocity.y = 0.0;
        } else if axis == Vec3::Y && movement.y > 0.0 {
            controller.velocity.y = 0.0;
        } else if axis != Vec3::Y {
            controller.velocity.x = 0.0;
            controller.velocity.z = 0.0;
        }
    }

    transform.translation = new_pos;
    controller.grounded = controller.grounded || is_grounded(new_pos, &*get_voxel);
}

fn collides(position: Vec3, get_voxel: &(impl Fn(IVec3) -> WorldVoxel<u8> + ?Sized)) -> bool {
    let min = position + Vec3::new(-PLAYER_RADIUS, 0.0, -PLAYER_RADIUS);
    let max = position + Vec3::new(PLAYER_RADIUS, PLAYER_HEIGHT, PLAYER_RADIUS);

    for x in (min.x.floor() as i32)..=(max.x.floor() as i32) {
        for y in (min.y.floor() as i32)..=(max.y.floor() as i32) {
            for z in (min.z.floor() as i32)..=(max.z.floor() as i32) {
                if is_solid_voxel(get_voxel(IVec3::new(x, y, z))) {
                    return true;
                }
            }
        }
    }
    false
}

fn is_grounded(position: Vec3, get_voxel: &(impl Fn(IVec3) -> WorldVoxel<u8> + ?Sized)) -> bool {
    let foot = position + Vec3::new(0.0, -0.05, 0.0);
    is_solid_voxel(get_voxel(foot.floor().as_ivec3()))
}

fn is_solid_voxel(voxel: WorldVoxel<u8>) -> bool {
    match voxel {
        WorldVoxel::Solid(material) => BlockId::from_material(material)
            .map(BlockId::is_solid)
            .unwrap_or(true),
        _ => false,
    }
}

pub fn find_spawn_position(seed: u32) -> Vec3 {
    let mut best = (0, 0, terrain_surface_height(seed, 0, 0));
    for z in -8..=8 {
        for x in -8..=8 {
            let height = terrain_surface_height(seed, x, z);
            if height >= best.2 {
                best = (x, z, height);
            }
        }
    }

    Vec3::new(
        best.0 as f32 + 0.5,
        best.2 as f32 + 1.0,
        best.1 as f32 + 0.5,
    )
}
