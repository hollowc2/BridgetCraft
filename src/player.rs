use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use bevy_voxel_world::prelude::*;

use crate::block::BlockId;
use crate::voxel_config::BridgetWorld;

pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_RADIUS: f32 = 0.35;
pub const WALK_SPEED: f32 = 6.0;
pub const FLY_SPEED: f32 = 10.0;
pub const JUMP_SPEED: f32 = 8.0;
pub const GRAVITY: f32 = 22.0;
pub const MOUSE_SENSITIVITY: f32 = 0.002;

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
}

#[derive(Resource)]
pub struct PlayerSettings {
    pub mouse_sensitivity: f32,
    pub render_distance: u32,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: MOUSE_SENSITIVITY,
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
            VoxelWorldCamera::<BridgetWorld>::default(),
            Transform::from_xyz(0.0, PLAYER_HEIGHT - 0.2, 0.0),
        ))
        .id();

    commands
        .spawn((
            Player,
            PlayerController::default(),
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
    mut motion: MessageReader<MouseMotion>,
    settings: Res<PlayerSettings>,
    mut players: Query<(&mut PlayerController, &Children), With<Player>>,
    mut cameras: Query<&mut Transform, With<PlayerCamera>>,
) {
    let mut delta = Vec2::ZERO;
    for event in motion.read() {
        delta += event.delta;
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
    mut players: Query<(&mut Transform, &mut PlayerController), With<Player>>,
    voxel_world: VoxelWorld<BridgetWorld>,
) {
    let Ok((mut transform, mut controller)) = players.single_mut() else {
        return;
    };

    controller.fly_toggle_cooldown = (controller.fly_toggle_cooldown - time.delta_secs()).max(0.0);

    if controller.fly_toggle_cooldown <= 0.0 && keys.just_pressed(KeyCode::Space) {
        let now = time.elapsed_secs();
        // Double-tap detection via cooldown trick: if space pressed while still grounded recently
        if keys.pressed(KeyCode::Space) && controller.grounded {
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

    if controller.flying {
        if keys.pressed(KeyCode::Space) {
            wish_dir.y += 1.0;
        }
        if keys.pressed(KeyCode::ShiftLeft) {
            wish_dir.y -= 1.0;
        }
        if wish_dir != Vec3::ZERO {
            transform.translation += wish_dir.normalize() * FLY_SPEED * time.delta_secs();
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

    if controller.grounded && keys.just_pressed(KeyCode::Space) {
        controller.velocity.y = JUMP_SPEED;
        controller.grounded = false;
    }

    controller.velocity.y -= GRAVITY * time.delta_secs();
    let get_voxel = voxel_world.get_voxel_fn();
    move_with_collision(&mut transform, &mut controller, get_voxel, time.delta_secs());
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

pub fn find_spawn_position(voxel_world: &VoxelWorld<BridgetWorld>) -> Vec3 {
    let get_voxel = voxel_world.get_voxel_fn();
    for z in -4..=4 {
        for x in -4..=4 {
            for y in (1..40).rev() {
                let pos = IVec3::new(x, y, z);
                let below = get_voxel(pos);
                let above = get_voxel(pos + IVec3::Y);
                if is_solid_voxel(below) && !is_solid_voxel(above) {
                    return pos.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
                }
            }
        }
    }
    Vec3::new(0.5, 12.0, 0.5)
}
