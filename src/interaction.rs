use bevy::prelude::*;
use bevy_replicon::prelude::{ClientTriggerExt, ServerTriggerExt, ToClients, SendTargets};
use bevy_voxel_world::prelude::*;

use crate::block::{BlockId, HotbarSelection};
use crate::net::replicate::BlockEditRequest;
use crate::net::NetworkRole;
use crate::save::{record_edit, WorldEdits};
use crate::voxel_config::BridgetWorld;

#[derive(Component, Default)]
pub struct BlockTarget {
    pub hit_pos: Option<IVec3>,
    pub place_pos: Option<IVec3>,
}

pub fn update_block_target(
    buttons: Res<ButtonInput<MouseButton>>,
    voxel_world: VoxelWorld<BridgetWorld>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::player::PlayerCamera>>,
    mut targets: Query<&mut BlockTarget>,
) {
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };

    let center = camera.logical_viewport_size().map(|size| size / 2.0).unwrap_or(Vec2::ZERO);
    let Ok(ray) = camera.viewport_to_world(camera_transform, center) else {
        return;
    };

    let Ok(mut target) = targets.single_mut() else {
        return;
    };

    target.hit_pos = None;
    target.place_pos = None;

    if let Some(result) = voxel_world.raycast(ray, &|(_pos, voxel)| voxel.is_solid()) {
        let hit_pos = result.voxel_pos();
        target.hit_pos = Some(hit_pos);
        if let Some(normal) = result.voxel_normal() {
            target.place_pos = Some(hit_pos + normal);
        }
    }

    let _ = buttons;
}

pub fn handle_block_interaction(
    buttons: Res<ButtonInput<MouseButton>>,
    selection: Res<HotbarSelection>,
    target: Single<&BlockTarget>,
    mut voxel_world: VoxelWorld<BridgetWorld>,
    mut edits: ResMut<WorldEdits>,
    role: Res<NetworkRole>,
    mut commands: Commands,
) {
    if role.is_client() {
        if buttons.just_pressed(MouseButton::Left) {
            if let Some(pos) = target.hit_pos {
                commands.client_trigger(BlockEditRequest {
                    pos,
                    voxel: SavedVoxel::Air,
                });
            }
        }
        if buttons.just_pressed(MouseButton::Right) {
            if let Some(pos) = target.place_pos {
                commands.client_trigger(BlockEditRequest {
                    pos,
                    voxel: SavedVoxel::Solid(selection.selected_block().as_material()),
                });
            }
        }
        return;
    }

    if buttons.just_pressed(MouseButton::Left) {
        if let Some(pos) = target.hit_pos {
            apply_block_edit(&mut voxel_world, &mut edits, pos, WorldVoxel::Air);
            if role.is_host() {
                commands.server_trigger(ToClients {
                    targets: SendTargets::CLIENTS_ONLY,
                    message: BlockEditBroadcast {
                        pos,
                        voxel: SavedVoxel::Air,
                    },
                });
            }
        }
    }

    if buttons.just_pressed(MouseButton::Right) {
        if let Some(pos) = target.place_pos {
            let voxel = selection.selected_block().to_world_voxel();
            apply_block_edit(&mut voxel_world, &mut edits, pos, voxel);
            if role.is_host() {
                commands.server_trigger(ToClients {
                    targets: SendTargets::CLIENTS_ONLY,
                    message: BlockEditBroadcast {
                        pos,
                        voxel: SavedVoxel::from_world_voxel(voxel),
                    },
                });
            }
        }
    }
}

use crate::net::replicate::BlockEditBroadcast;

pub fn apply_block_edit(
    voxel_world: &mut VoxelWorld<BridgetWorld>,
    edits: &mut WorldEdits,
    pos: IVec3,
    voxel: WorldVoxel<u8>,
) {
    voxel_world.set_voxel(pos, voxel);
    record_edit(edits, pos, voxel);
}

use crate::block::SavedVoxel;
