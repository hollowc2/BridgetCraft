use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy_voxel_world::prelude::WorldVoxel;
use futures_lite::future;
use serde::{Deserialize, Serialize};

use crate::block::SavedVoxel;
use crate::interaction::PendingBlockEdits;
use crate::world_gen::WorldMetadata;

const SAVE_INTERVAL_SECS: f32 = 30.0;

#[derive(Resource, Default, Clone)]
pub struct WorldEdits {
    edits: HashMap<IVec3, SavedVoxel>,
}

impl WorldEdits {
    pub fn record(&mut self, pos: IVec3, voxel: SavedVoxel) {
        self.edits.insert(pos, voxel);
    }

    pub fn iter(&self) -> impl Iterator<Item = (IVec3, SavedVoxel)> + '_ {
        self.edits.iter().map(|(pos, voxel)| (*pos, *voxel))
    }

    pub fn len(&self) -> usize {
        self.edits.len()
    }
}

#[derive(Serialize, Deserialize)]
struct WorldSaveFile {
    metadata: WorldMetadata,
    edits: Vec<(IVec3, SavedVoxel)>,
}

#[derive(Resource, Default)]
pub struct SaveStatus {
    pub last_error: Option<String>,
}

#[derive(Resource, Default)]
pub(crate) struct PendingSaveTask(Option<Task<std::io::Result<()>>>);

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingSaveTask>();
        app.init_resource::<SaveStatus>();
    }
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
    pending: &mut PendingBlockEdits,
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

    edits.edits = save.edits.into_iter().collect();
    for (pos, voxel) in edits.iter() {
        pending.queue(pos, voxel.to_world_voxel());
    }
    info!("loaded {} edits for world '{}'", edits.len(), metadata.name);
}

pub fn revert_to_world_base(
    metadata: &WorldMetadata,
    edits: &mut WorldEdits,
    pending: &mut PendingBlockEdits,
    persist: bool,
) -> std::io::Result<()> {
    let affected: Vec<IVec3> = edits.edits.keys().copied().collect();
    edits.edits.clear();

    let affected_count = affected.len();
    for pos in affected {
        pending.queue(pos, WorldVoxel::Unset);
    }

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
        edits: edits.iter().collect(),
    };
    let json = serde_json::to_string(&save)?;
    fs::write(dir.join("world.json"), json)
}

#[derive(Resource)]
pub struct SaveTimer(pub Timer);

impl Default for SaveTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(SAVE_INTERVAL_SECS, TimerMode::Repeating))
    }
}

fn poll_async_save(pending: &mut PendingSaveTask, status: &mut SaveStatus) {
    let Some(task) = pending.0.as_mut() else {
        return;
    };

    if let Some(result) = future::block_on(future::poll_once(task)) {
        pending.0 = None;
        match result {
            Ok(()) => status.last_error = None,
            Err(err) => {
                let message = err.to_string();
                warn!("save failed: {message}");
                status.last_error = Some(message);
            }
        }
    }
}

pub fn auto_save_system(
    time: Res<Time>,
    mut timer: ResMut<SaveTimer>,
    metadata: Res<WorldMetadata>,
    edits: Res<WorldEdits>,
    mut pending: ResMut<PendingSaveTask>,
    mut status: ResMut<SaveStatus>,
) {
    poll_async_save(&mut pending, &mut status);

    if pending.0.is_some() {
        return;
    }

    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    let metadata = metadata.clone();
    let edits = edits.clone();
    pending.0 = Some(AsyncComputeTaskPool::get().spawn(async move {
        save_world(&metadata, &edits)
    }));
}

pub fn save_on_exit(
    metadata: Res<WorldMetadata>,
    edits: Res<WorldEdits>,
    mut pending: ResMut<PendingSaveTask>,
    mut exit_events: MessageReader<AppExit>,
) {
    for _ in exit_events.read() {
        if let Some(task) = pending.0.take() {
            match future::block_on(task) {
                Ok(()) => {}
                Err(err) => warn!("exit save failed: {err}"),
            }
            continue;
        }

        if let Err(err) = save_world(&metadata, &edits) {
            warn!("exit save failed: {err}");
        }
    }
}

pub fn record_edit(edits: &mut WorldEdits, pos: IVec3, voxel: WorldVoxel<u8>) {
    edits.record(pos, SavedVoxel::from_world_voxel(voxel));
}
