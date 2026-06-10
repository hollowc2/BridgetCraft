use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use bevy::window::{CursorGrabMode, CursorOptions, PresentMode};
use bevy_voxel_world::prelude::*;

use crate::audio::{spatial_audio_listener, GameAudio};
use crate::block::BlockId;
use crate::gamepad::select_primary;
use crate::interaction::PendingBlockEdits;
use crate::save::WorldEdits;
use crate::voxel_config::BridgetWorld;
use crate::world_gen::ProceduralTerrain;

pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_RADIUS: f32 = 0.35;
pub const WALK_SPEED: f32 = 6.0;
pub const FLY_SPEED: f32 = 10.0;
pub const JUMP_SPEED: f32 = 8.0;
pub const GRAVITY: f32 = 22.0;
pub const LOW_GRAVITY_MULTIPLIER: f32 = 0.35;
pub const DOUBLE_TAP_JUMP_WINDOW: f32 = 0.35;
pub const MOUSE_SENSITIVITY: f32 = 0.002;
pub const GAMEPAD_LOOK_SENSITIVITY: f32 = 2.5;
pub const GAMEPAD_MOVE_DEADZONE: f32 = 0.15;
pub const INITIAL_LOOK_PITCH: f32 = -0.35;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Component)]
pub struct ChunkSpawnAnchor;

#[derive(Component, Default)]
pub struct PlayerController {
    pub yaw: f32,
    pub pitch: f32,
    pub velocity: Vec3,
    pub grounded: bool,
    pub flying: bool,
    pub last_jump_press_time: f32,
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

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum GravityMode {
    #[default]
    Normal,
    Low,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ShadowQuality {
    Off,
    Low,
    #[default]
    High,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum FlyActivation {
    #[default]
    Off,
    Always,
    DoubleTap,
}

impl GravityMode {
    pub fn multiplier(self) -> f32 {
        match self {
            Self::Normal => 1.0,
            Self::Low => LOW_GRAVITY_MULTIPLIER,
        }
    }

    pub const ALL: [Self; 2] = [Self::Normal, Self::Low];

    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Low => "Low",
        }
    }
}

impl ShadowQuality {
    pub const ALL: [Self; 3] = [Self::Off, Self::Low, Self::High];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Low => "Low",
            Self::High => "High",
        }
    }
}

impl FlyActivation {
    pub const ALL: [Self; 3] = [Self::Off, Self::Always, Self::DoubleTap];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Always => "Always on",
            Self::DoubleTap => "Double-tap jump",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum VsyncMode {
    #[default]
    On,
    Off,
}

impl VsyncMode {
    pub const ALL: [Self; 2] = [Self::On, Self::Off];

    pub fn label(self) -> &'static str {
        match self {
            Self::On => "On",
            Self::Off => "Off (benchmark)",
        }
    }

    pub fn present_mode(self) -> PresentMode {
        match self {
            Self::On => PresentMode::AutoVsync,
            Self::Off => PresentMode::AutoNoVsync,
        }
    }
}

#[derive(Resource)]
pub struct PlayerSettings {
    pub mouse_sensitivity: f32,
    pub gamepad_look_sensitivity: f32,
    pub render_distance: u32,
    pub shadow_quality: ShadowQuality,
    pub show_diagnostics: bool,
    pub gravity_mode: GravityMode,
    pub fly_activation: FlyActivation,
    pub msaa: Msaa,
    pub vsync_mode: VsyncMode,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: MOUSE_SENSITIVITY,
            gamepad_look_sensitivity: GAMEPAD_LOOK_SENSITIVITY,
            render_distance: 4,
            shadow_quality: ShadowQuality::Low,
            show_diagnostics: false,
            gravity_mode: GravityMode::Normal,
            fly_activation: FlyActivation::Off,
            msaa: Msaa::Sample4,
            vsync_mode: VsyncMode::On,
        }
    }
}

fn effective_msaa(msaa: Msaa) -> Msaa {
    match msaa {
        Msaa::Off => Msaa::Sample4,
        other => other,
    }
}

pub fn spawn_player(
    commands: &mut Commands,
    name: &str,
    position: Vec3,
    settings: &PlayerSettings,
) -> (Entity, Entity) {
    let camera_rotation = Quat::from_rotation_x(INITIAL_LOOK_PITCH);
    let eye_position = position + Vec3::new(0.0, PLAYER_HEIGHT - 0.2, 0.0);
    let camera_transform =
        Transform::from_translation(eye_position).with_rotation(camera_rotation);

    // Root-level camera so we can write GlobalTransform directly. bevy_voxel_world casts chunk
    // rays from GlobalTransform in PreUpdate, before Bevy's transform propagation runs.
    let camera = commands
        .spawn((
            Camera3d::default(),
            Camera {
                order: 0,
                clear_color: ClearColorConfig::Custom(Color::srgb(0.53, 0.75, 0.92).into()),
                ..default()
            },
            // MSAA Off breaks bevy_voxel_world chunk rendering; always spawn with 2x/4x MSAA.
            effective_msaa(settings.msaa),
            PlayerCamera,
            spatial_audio_listener(),
            camera_transform,
            GlobalTransform::from(camera_transform),
        ))
        .id();

    let player = commands
        .spawn((
            Player,
            PlayerController {
                pitch: INITIAL_LOOK_PITCH,
                ..PlayerController::new()
            },
            Transform::from_translation(position),
            Visibility::default(),
            Name::new(name.to_string()),
        ))
        .id();

    (player, camera)
}

pub fn spawn_chunk_anchor(commands: &mut Commands, position: Vec3) -> Entity {
    let rotation = Quat::from_rotation_x(INITIAL_LOOK_PITCH);
    let eye = position + Vec3::new(0.0, PLAYER_HEIGHT - 0.2, 0.0);
    let forward = Vec3::new(0.0, 0.0, -1.0);
    let transform = Transform::from_translation(eye + forward * 6.0).with_rotation(rotation);

    commands
        .spawn((
            ChunkSpawnAnchor,
            // bevy_voxel_world only considers entities with both VoxelWorldCamera and Camera.
            Camera3d::default(),
            Camera {
                is_active: false,
                ..default()
            },
            VoxelWorldCamera::<BridgetWorld>::default(),
            transform,
            GlobalTransform::from(transform),
        ))
        .id()
}

pub fn lead_chunk_spawn_anchor(
    players: Query<(&Transform, &PlayerController), With<Player>>,
    mut anchors: Query<
        (&mut Transform, &mut GlobalTransform),
        (With<ChunkSpawnAnchor>, Without<Player>),
    >,
) {
    let Ok((player, controller)) = players.single() else {
        return;
    };
    let Ok((mut transform, mut global)) = anchors.single_mut() else {
        return;
    };

    let forward = Vec3::new(-controller.yaw.sin(), 0.0, -controller.yaw.cos());
    let velocity = Vec3::new(controller.velocity.x, 0.0, controller.velocity.z);
    let speed = velocity.length();
    let lead_distance = if speed > 0.5 {
        (speed * 0.75).clamp(4.0, 16.0)
    } else {
        0.0
    };
    let lead = if lead_distance > 0.0 {
        velocity.normalize_or_zero() * lead_distance
    } else {
        forward * 6.0
    };

    let eye = player.translation + Vec3::new(0.0, PLAYER_HEIGHT - 0.2, 0.0);
    transform.translation = eye + lead;
    transform.rotation =
        Quat::from_rotation_y(controller.yaw) * Quat::from_rotation_x(controller.pitch);
    *global = GlobalTransform::from(*transform);
}

pub fn sync_player_camera(
    players: Query<(&Transform, &PlayerController), With<Player>>,
    mut cameras: Query<
        (&mut Transform, &mut GlobalTransform),
        (With<PlayerCamera>, Without<Player>),
    >,
) {
    let Ok((player, controller)) = players.single() else {
        return;
    };
    let Ok((mut transform, mut global)) = cameras.single_mut() else {
        return;
    };

    transform.translation = player.translation + Vec3::new(0.0, PLAYER_HEIGHT - 0.2, 0.0);
    transform.rotation =
        Quat::from_rotation_y(controller.yaw) * Quat::from_rotation_x(controller.pitch);
    *global = GlobalTransform::from(*transform);
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
    mut players: Query<&mut PlayerController, With<Player>>,
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

    let Ok(mut controller) = players.single_mut() else {
        return;
    };

    controller.yaw -= delta.x * settings.mouse_sensitivity;
    controller.pitch = (controller.pitch - delta.y * settings.mouse_sensitivity)
        .clamp(-1.54, 1.54);
}

pub fn player_movement(
    time: Res<Time<Fixed>>,
    settings: Res<PlayerSettings>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(&Name, &Gamepad)>,
    terrain: Res<ProceduralTerrain>,
    edits: Res<WorldEdits>,
    pending: Res<PendingBlockEdits>,
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

    let mut double_tap_fly = false;
    if jump_pressed {
        let now = time.elapsed_secs();
        if settings.fly_activation == FlyActivation::DoubleTap
            && now - controller.last_jump_press_time < DOUBLE_TAP_JUMP_WINDOW
        {
            controller.flying = true;
            controller.velocity.y = 0.0;
            double_tap_fly = true;
        }
        controller.last_jump_press_time = now;
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
    let resolve = |pos: IVec3| {
        resolve_voxel(
            chunk_get_voxel.as_ref(),
            &terrain,
            &edits,
            &pending,
            pos,
        )
    };

    let fly_active =
        settings.fly_activation == FlyActivation::Always || controller.flying;

    if fly_active {
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
            move_by_delta(&mut transform.translation, delta, &resolve);
        }
        recover_if_below_surface(&mut transform, &terrain, &resolve);

        if settings.fly_activation == FlyActivation::DoubleTap
            && is_grounded(transform.translation, &resolve)
        {
            controller.flying = false;
            controller.grounded = true;
            controller.velocity = Vec3::ZERO;
        } else {
            controller.grounded = false;
            controller.velocity = Vec3::ZERO;
        }
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

    if controller.grounded && jump_pressed && !double_tap_fly {
        controller.velocity.y = JUMP_SPEED;
        controller.grounded = false;
    }

    let gravity = GRAVITY * settings.gravity_mode.multiplier();
    controller.velocity.y -= gravity * time.delta_secs();
    move_with_collision(
        &mut transform,
        &mut controller,
        &resolve,
        time.delta_secs(),
    );
    recover_if_below_surface(&mut transform, &terrain, &resolve);

    let moving = Vec2::new(controller.velocity.x, controller.velocity.z).length() > 0.5;
    if controller.grounded && moving {
        controller.footstep_timer.tick(time.delta());
        if controller.footstep_timer.just_finished() {
            audio.play_footstep_at_feet(
                &mut commands,
                &voxel_world,
                &terrain,
                transform.translation,
            );
        }
    } else {
        controller.footstep_timer.reset();
    }
}

#[inline]
fn resolve_voxel(
    chunk_get_voxel: &dyn Fn(IVec3) -> WorldVoxel<u8>,
    terrain: &ProceduralTerrain,
    edits: &WorldEdits,
    pending: &PendingBlockEdits,
    pos: IVec3,
) -> WorldVoxel<u8> {
    if let Some(voxel) = pending.get_voxel(pos) {
        return voxel;
    }
    if let Some(voxel) = edits.get_voxel(pos) {
        return voxel;
    }

    let voxel = chunk_get_voxel(pos);
    if voxel == WorldVoxel::Unset {
        terrain.voxel_at(pos)
    } else {
        voxel
    }
}

fn move_by_delta(
    position: &mut Vec3,
    delta: Vec3,
    resolve: &dyn Fn(IVec3) -> WorldVoxel<u8>,
) {
    let mut new_pos = *position;

    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        let movement = axis * delta.dot(axis);
        if movement.length_squared() == 0.0 {
            continue;
        }
        let candidate = new_pos + movement;
        if !collides(candidate, resolve) {
            new_pos = candidate;
        }
    }

    *position = new_pos;
}

fn recover_if_below_surface(
    transform: &mut Transform,
    terrain: &ProceduralTerrain,
    resolve: &dyn Fn(IVec3) -> WorldVoxel<u8>,
) {
    let x = transform.translation.x.floor() as i32;
    let z = transform.translation.z.floor() as i32;
    let min_feet_y = terrain.surface_height(x, z) as f32 + 1.0;

    if transform.translation.y >= min_feet_y && !collides(transform.translation, resolve) {
        return;
    }

    transform.translation.y = min_feet_y;
    for offset in 0..=6 {
        let candidate = transform.translation + Vec3::new(0.0, offset as f32, 0.0);
        if !collides(candidate, resolve) {
            transform.translation = candidate;
            return;
        }
    }
}

fn move_with_collision(
    transform: &mut Transform,
    controller: &mut PlayerController,
    resolve: &dyn Fn(IVec3) -> WorldVoxel<u8>,
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
        if !collides(candidate, resolve) {
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
    controller.grounded = controller.grounded || is_grounded(new_pos, resolve);
}

fn collides(position: Vec3, resolve: &dyn Fn(IVec3) -> WorldVoxel<u8>) -> bool {
    let min = position + Vec3::new(-PLAYER_RADIUS, 0.0, -PLAYER_RADIUS);
    let max = position + Vec3::new(PLAYER_RADIUS, PLAYER_HEIGHT, PLAYER_RADIUS);

    for x in (min.x.floor() as i32)..=(max.x.floor() as i32) {
        for y in (min.y.floor() as i32)..=(max.y.floor() as i32) {
            for z in (min.z.floor() as i32)..=(max.z.floor() as i32) {
                if is_solid_voxel(resolve(IVec3::new(x, y, z))) {
                    return true;
                }
            }
        }
    }
    false
}

fn is_grounded(position: Vec3, resolve: &dyn Fn(IVec3) -> WorldVoxel<u8>) -> bool {
    let foot = position + Vec3::new(0.0, -0.05, 0.0);
    is_solid_voxel(resolve(foot.floor().as_ivec3()))
}

fn is_solid_voxel(voxel: WorldVoxel<u8>) -> bool {
    match voxel {
        WorldVoxel::Solid(material) => BlockId::from_material(material)
            .map(BlockId::is_solid)
            .unwrap_or(true),
        _ => false,
    }
}

pub fn find_spawn_position(terrain: &ProceduralTerrain) -> Vec3 {
    let height = terrain.surface_height(0, 0);
    Vec3::new(0.5, height as f32 + 1.0, 0.5)
}

pub fn apply_render_settings_on_enter(
    settings: Res<PlayerSettings>,
    mut windows: Query<&mut Window>,
    mut cameras: Query<&mut Msaa, With<PlayerCamera>>,
) {
    let present_mode = settings.vsync_mode.present_mode();
    for mut window in &mut windows {
        window.present_mode = present_mode;
    }

    let msaa = effective_msaa(settings.msaa);
    for mut camera_msaa in &mut cameras {
        *camera_msaa = msaa;
    }
}

pub fn apply_render_settings(
    settings: Res<PlayerSettings>,
    windows: Query<&mut Window>,
    cameras: Query<&mut Msaa, With<PlayerCamera>>,
) {
    if !settings.is_changed() {
        return;
    }

    let present_mode = settings.vsync_mode.present_mode();
    for mut window in windows {
        window.present_mode = present_mode;
    }

    let msaa = effective_msaa(settings.msaa);
    for mut camera_msaa in cameras {
        *camera_msaa = msaa;
    }
}
