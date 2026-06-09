use bevy::prelude::*;
use bevy_replicon::prelude::{ClientTriggerExt, ServerTriggerExt, ToClients, SendTargets};
use bevy_voxel_world::prelude::*;

use crate::audio::{voxel_block_at, GameAudio};
use crate::block::HotbarSelection;
use crate::gamepad::select_primary;
use crate::net::replicate::BlockEditRequest;
use crate::net::NetworkRole;
use crate::save::{record_edit, WorldEdits};
use crate::voxel_config::BridgetWorld;
use crate::world_gen::WorldMetadata;

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

fn break_pressed(buttons: &ButtonInput<MouseButton>, gamepad: Option<&Gamepad>) -> bool {
    buttons.just_pressed(MouseButton::Left)
        || gamepad.is_some_and(|gamepad| {
            gamepad.just_pressed(GamepadButton::West)
                || gamepad.just_pressed(GamepadButton::RightTrigger2)
        })
}

fn place_pressed(buttons: &ButtonInput<MouseButton>, gamepad: Option<&Gamepad>) -> bool {
    buttons.just_pressed(MouseButton::Right)
        || gamepad.is_some_and(|gamepad| {
            gamepad.just_pressed(GamepadButton::East)
                || gamepad.just_pressed(GamepadButton::LeftTrigger2)
        })
}

pub fn handle_block_interaction(
    buttons: Res<ButtonInput<MouseButton>>,
    gamepads: Query<(&Name, &Gamepad)>,
    selection: Res<HotbarSelection>,
    target: Single<&BlockTarget>,
    mut voxel_world: VoxelWorld<BridgetWorld>,
    mut edits: ResMut<WorldEdits>,
    role: Res<NetworkRole>,
    metadata: Res<WorldMetadata>,
    mut audio: ResMut<GameAudio>,
    mut commands: Commands,
) {
    let gamepad = select_primary(gamepads.iter());

    if role.is_client() {
        if break_pressed(&buttons, gamepad) {
            if let Some(pos) = target.hit_pos {
                commands.client_trigger(BlockEditRequest {
                    pos,
                    voxel: SavedVoxel::Air,
                });
            }
        }
        if place_pressed(&buttons, gamepad) {
            if let Some(pos) = target.place_pos {
                commands.client_trigger(BlockEditRequest {
                    pos,
                    voxel: SavedVoxel::Solid(selection.selected_block().as_material()),
                });
            }
        }
        return;
    }

    if break_pressed(&buttons, gamepad) {
        if let Some(pos) = target.hit_pos {
            if let Some(block) = voxel_block_at(&voxel_world, &metadata, pos) {
                apply_block_edit(&mut voxel_world, &mut edits, pos, WorldVoxel::Air);
                audio.play_block_break(&mut commands, block, pos);
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
    }

    if place_pressed(&buttons, gamepad) {
        if let Some(pos) = target.place_pos {
            let block = selection.selected_block();
            let voxel = block.to_world_voxel();
            apply_block_edit(&mut voxel_world, &mut edits, pos, voxel);
            audio.play_block_place(&mut commands, block, pos);
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
