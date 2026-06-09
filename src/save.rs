use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use bevy::prelude::*;
use bevy_voxel_world::prelude::*;
use serde::{Deserialize, Serialize};

use crate::block::SavedVoxel;
use crate::voxel_config::BridgetWorld;
use crate::world_gen::{world_base_voxels, WorldMetadata};

const SAVE_INTERVAL_SECS: f32 = 30.0;

#[derive(Resource, Default, Clone, Serialize, Deserialize)]
pub struct WorldEdits {
    pub edits: Vec<(IVec3, SavedVoxel)>,
}

impl WorldEdits {
    pub fn record(&mut self, pos: IVec3, voxel: SavedVoxel) {
        if let Some(existing) = self.edits.iter_mut().find(|(p, _)| *p == pos) {
            existing.1 = voxel;
        } else {
            self.edits.push((pos, voxel));
        }
    }
}

#[derive(Serialize, Deserialize)]
struct WorldSaveFile {
    metadata: WorldMetadata,
    edits: Vec<(IVec3, SavedVoxel)>,
}

pub fn save_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("BridgetCraft")
        .join("worlds")
}

pub fn world_path(name: &str) -> PathBuf {
    save_dir().join(sanitize_world_name(name))
}

fn sanitize_world_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

pub fn load_world_edits(
    metadata: &WorldMetadata,
    edits: &mut WorldEdits,
    voxel_world: &mut VoxelWorld<crate::voxel_config::BridgetWorld>,
) {
    edits.edits.clear();

    let path = world_path(&metadata.name).join("world.json");
    if !path.exists() {
        return;
    }

    let contents = fs::read_to_string(&path).unwrap_or_default();
    let Ok(save) = serde_json::from_str::<WorldSaveFile>(&contents) else {
        warn!("failed to parse save file at {}", path.display());
        return;
    };

    edits.edits = save.edits.clone();
    for (pos, voxel) in &save.edits {
        voxel_world.set_voxel(*pos, voxel.to_world_voxel());
    }
    info!("loaded {} edits for world '{}'", save.edits.len(), metadata.name);
}

pub fn apply_world_base(
    seed: u32,
    voxel_world: &mut VoxelWorld<BridgetWorld>,
) {
    for (pos, voxel) in world_base_voxels(seed) {
        voxel_world.set_voxel(pos, voxel);
    }
}

pub fn revert_to_world_base(
    metadata: &WorldMetadata,
    edits: &mut WorldEdits,
    voxel_world: &mut VoxelWorld<BridgetWorld>,
    persist: bool,
) -> std::io::Result<()> {
    let base = world_base_voxels(metadata.seed);
    let mut affected = HashSet::new();
    for (pos, _) in &edits.edits {
        affected.insert(*pos);
    }
    for (pos, _) in &base {
        affected.insert(*pos);
    }

    edits.edits.clear();

    let affected_count = affected.len();
    for pos in affected {
        voxel_world.set_voxel(pos, WorldVoxel::Unset);
    }

    apply_world_base(metadata.seed, voxel_world);

    if persist {
        save_world(metadata, edits)?;
    }

    info!(
        "restored original base map for world '{}' ({} affected voxels)",
        metadata.name,
        affected_count
    );
    Ok(())
}

pub fn save_world(metadata: &WorldMetadata, edits: &WorldEdits) -> std::io::Result<()> {
    let dir = world_path(&metadata.name);
    fs::create_dir_all(&dir)?;
    let save = WorldSaveFile {
        metadata: metadata.clone(),
        edits: edits.edits.clone(),
    };
    let json = serde_json::to_string_pretty(&save)?;
    fs::write(dir.join("world.json"), json)
}

#[derive(Resource)]
pub struct SaveTimer(pub Timer);

impl Default for SaveTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(SAVE_INTERVAL_SECS, TimerMode::Repeating))
    }
}

pub fn auto_save_system(
    time: Res<Time>,
    mut timer: ResMut<SaveTimer>,
    metadata: Res<WorldMetadata>,
    edits: Res<WorldEdits>,
) {
    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        if let Err(err) = save_world(&metadata, &edits) {
            warn!("auto-save failed: {err}");
        }
    }
}

pub fn save_on_exit(
    metadata: Res<WorldMetadata>,
    edits: Res<WorldEdits>,
    mut exit_events: MessageReader<AppExit>,
) {
    for _ in exit_events.read() {
        if let Err(err) = save_world(&metadata, &edits) {
            warn!("exit save failed: {err}");
        }
    }
}

pub fn record_edit(edits: &mut WorldEdits, pos: IVec3, voxel: WorldVoxel<u8>) {
    edits.record(pos, SavedVoxel::from_world_voxel(voxel));
}
