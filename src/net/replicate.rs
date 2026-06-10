use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_voxel_world::prelude::*;
use serde::{Deserialize, Serialize};

use crate::audio::{voxel_block_at, GameAudio};
use crate::block::{BlockId, SavedVoxel};
use crate::interaction::{apply_block_edit, PendingBlockEdits};
use crate::player::Player;
use crate::save::{revert_to_world_base, WorldEdits};
use crate::voxel_config::BridgetWorld;
use crate::world_gen::{ProceduralTerrain, WorldMetadata};

#[derive(Component, Serialize, Deserialize, Clone)]
#[require(Replicated)]
pub struct NetworkPlayer {
    pub name: String,
    pub selected_block: u8,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Default)]
#[require(Replicated)]
pub struct NetworkTransform {
    pub translation: [f32; 3],
    pub yaw: f32,
}

#[derive(Component)]
pub struct RemotePlayerBody;

#[derive(Event, Serialize, Deserialize, Clone, Copy)]
pub struct BlockEditRequest {
    pub pos: IVec3,
    pub voxel: SavedVoxel,
}

#[derive(Event, Serialize, Deserialize, Clone, Copy)]
pub struct BlockEditBroadcast {
    pub pos: IVec3,
    pub voxel: SavedVoxel,
}

#[derive(Event, Serialize, Deserialize, Clone, Copy)]
pub struct WorldRevertBroadcast;

pub struct ReplicatePlugin;

impl Plugin for ReplicatePlugin {
    fn build(&self, app: &mut App) {
        app.replicate::<NetworkPlayer>()
            .replicate::<NetworkTransform>()
            .add_client_event::<BlockEditRequest>(Channel::Unordered)
            .add_server_event::<BlockEditBroadcast>(Channel::Unordered)
            .add_server_event::<WorldRevertBroadcast>(Channel::Unordered)
            .add_observer(apply_remote_block_edit)
            .add_observer(apply_block_edit_broadcast)
            .add_observer(apply_world_revert_broadcast)
            .add_observer(spawn_remote_player_visual)
            .add_systems(
                Update,
                (
                    sync_local_player_network_data,
                    sync_network_transforms,
                    tag_remote_players,
                ),
            );
    }
}

fn apply_remote_block_edit(
    request: On<FromClient<BlockEditRequest>>,
    mut pending: ResMut<PendingBlockEdits>,
    mut edits: ResMut<WorldEdits>,
    mut commands: Commands,
) {
    apply_block_edit(
        &mut pending,
        &mut edits,
        request.pos,
        request.voxel.to_world_voxel(),
    );
    commands.server_trigger(ToClients {
        targets: SendTargets::CLIENTS_ONLY,
        message: BlockEditBroadcast {
            pos: request.pos,
            voxel: request.voxel,
        },
    });
}

fn apply_block_edit_broadcast(
    broadcast: On<BlockEditBroadcast>,
    voxel_world: VoxelWorld<BridgetWorld>,
    mut pending: ResMut<PendingBlockEdits>,
    mut edits: ResMut<WorldEdits>,
    role: Res<crate::net::NetworkRole>,
    terrain: Res<ProceduralTerrain>,
    mut audio: ResMut<GameAudio>,
    mut commands: Commands,
) {
    if !role.is_client() {
        return;
    }

    match broadcast.voxel {
        SavedVoxel::Air => {
            if let Some(block) = voxel_block_at(&voxel_world, &terrain, broadcast.pos) {
                apply_block_edit(&mut pending, &mut edits, broadcast.pos, WorldVoxel::Air);
                audio.play_block_break(&mut commands, block, broadcast.pos);
            }
        }
        SavedVoxel::Solid(material) => {
            apply_block_edit(
                &mut pending,
                &mut edits,
                broadcast.pos,
                broadcast.voxel.to_world_voxel(),
            );
            if let Some(block) = BlockId::from_material(material) {
                audio.play_block_place(&mut commands, block, broadcast.pos);
            }
        }
    }
}

fn apply_world_revert_broadcast(
    _broadcast: On<WorldRevertBroadcast>,
    metadata: Res<WorldMetadata>,
    mut edits: ResMut<WorldEdits>,
    mut pending: ResMut<PendingBlockEdits>,
    role: Res<crate::net::NetworkRole>,
) {
    if !role.is_client() {
        return;
    }

    if let Err(err) = revert_to_world_base(&metadata, &mut edits, &mut pending, false) {
        warn!("failed to apply restored map from host: {err}");
    }
}

fn spawn_remote_player_visual(
    add: On<Add, RemotePlayerBody>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    players: Query<&NetworkPlayer>,
    local: Query<(), With<Player>>,
) {
    if local.get(add.entity).is_ok() {
        return;
    }

    let Ok(network_player) = players.get(add.entity) else {
        return;
    };

    let body_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.45, 0.32),
        ..default()
    });

    commands.entity(add.entity).with_children(|parent| {
        parent.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.7, 1.2, 0.45))),
            MeshMaterial3d(body_material),
            Transform::from_xyz(0.0, 0.6, 0.0),
        ));
        parent.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.45, 0.45, 0.45))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.8, 0.65),
                ..default()
            })),
            Transform::from_xyz(0.0, 1.45, 0.0),
        ));
        parent.spawn((
            Text2d::new(network_player.name.clone()),
            TextFont {
                font_size: 22.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Transform::from_xyz(0.0, 2.2, 0.0),
        ));
    });
}

fn sync_local_player_network_data(
    selection: Res<crate::block::HotbarSelection>,
    role: Res<crate::net::NetworkRole>,
    mut players: Query<
        (&Name, &Transform, &mut NetworkPlayer, &mut NetworkTransform),
        With<Player>,
    >,
) {
    if matches!(*role, crate::net::NetworkRole::None) {
        return;
    }

    let material = selection.selected_block().as_material();
    let selection_changed = selection.is_changed();

    for (name, transform, mut network_player, mut network_transform) in &mut players {
        let name_str = name.as_str();
        if network_player.name != name_str {
            network_player.name = name_str.to_string();
        }
        if selection_changed {
            network_player.selected_block = material;
        }

        let translation = transform.translation.to_array();
        let yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
        if network_transform.translation != translation {
            network_transform.translation = translation;
        }
        if (network_transform.yaw - yaw).abs() > f32::EPSILON {
            network_transform.yaw = yaw;
        }
    }
}

fn tag_remote_players(
    mut commands: Commands,
    players: Query<Entity, (With<NetworkPlayer>, Without<Player>, Without<RemotePlayerBody>)>,
) {
    for entity in &players {
        commands.entity(entity).insert(RemotePlayerBody);
    }
}

fn sync_network_transforms(
    mut remote: Query<
        (&NetworkTransform, &mut Transform),
        (With<RemotePlayerBody>, Without<Player>),
    >,
) {
    for (network_transform, mut transform) in &mut remote {
        transform.translation = Vec3::from_array(network_transform.translation);
        transform.rotation = Quat::from_rotation_y(network_transform.yaw);
    }
}
