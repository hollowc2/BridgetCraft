use std::sync::Arc;

use bevy::prelude::*;
use bevy_voxel_world::prelude::*;

use crate::block::texture_index_table;
use crate::world_gen::{terrain_lookup, ProceduralTerrain, WorldMetadata};

#[derive(Resource, Clone)]
pub struct BridgetWorld {
    pub seed: u32,
    pub spawning_distance: u32,
    pub max_spawn_per_frame: usize,
    terrain: Arc<ProceduralTerrain>,
}

impl Default for BridgetWorld {
    fn default() -> Self {
        let terrain = Arc::new(ProceduralTerrain::new(42_424));
        Self {
            seed: 42_424,
            spawning_distance: 6,
            max_spawn_per_frame: 24,
            terrain,
        }
    }
}

impl VoxelWorldConfig for BridgetWorld {
    type MaterialIndex = u8;
    type ChunkUserBundle = ();

    fn spawning_distance(&self) -> u32 {
        self.spawning_distance
    }

    fn min_despawn_distance(&self) -> u32 {
        2
    }

    fn chunk_spawn_strategy(&self) -> ChunkSpawnStrategy {
        ChunkSpawnStrategy::Close
    }

    fn chunk_despawn_strategy(&self) -> ChunkDespawnStrategy {
        ChunkDespawnStrategy::FarAway
    }

    fn attach_chunks_to_root(&self) -> bool {
        false
    }

    fn max_active_chunk_threads(&self) -> usize {
        std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(4)
    }

    fn voxel_lookup_delegate(&self) -> VoxelLookupDelegate<Self::MaterialIndex> {
        terrain_lookup(self.terrain.clone())
    }

    fn texture_index_mapper(
        &self,
    ) -> Arc<dyn Fn(u8) -> [u32; 3] + Send + Sync> {
        let table = texture_index_table();
        Arc::new(move |material| table[material as usize])
    }

    fn voxel_texture(&self) -> Option<(String, u32)> {
        Some(("textures/voxel_atlas.png".into(), crate::block::ATLAS_TEXTURE_COUNT))
    }

    fn spawning_rays(&self) -> usize {
        24
    }

    fn max_spawn_per_frame(&self) -> usize {
        self.max_spawn_per_frame
    }
}

pub struct VoxelConfigPlugin;

impl Plugin for VoxelConfigPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BridgetWorld>();
        app.init_resource::<FrameBudget>();
        app.add_plugins(VoxelWorldPlugin::with_config(BridgetWorld::default()));
        app.add_systems(Update, tune_chunk_spawn_budget);
    }
}

pub fn sync_world_seed(
    mut config: ResMut<BridgetWorld>,
    mut terrain: ResMut<ProceduralTerrain>,
    metadata: Res<WorldMetadata>,
    settings: Res<crate::player::PlayerSettings>,
) {
    if terrain.seed != metadata.seed {
        *terrain = ProceduralTerrain::new(metadata.seed);
    }

    if config.terrain.generation != terrain.generation {
        config.terrain = Arc::new(terrain.clone());
    }

    config.seed = metadata.seed;
    config.spawning_distance = settings.render_distance;
}

#[derive(Resource)]
struct FrameBudget {
    ema_ms: f32,
}

impl Default for FrameBudget {
    fn default() -> Self {
        Self { ema_ms: 16.0 }
    }
}

fn tune_chunk_spawn_budget(
    time: Res<Time>,
    mut budget: ResMut<FrameBudget>,
    mut config: ResMut<BridgetWorld>,
) {
    // Hold the initial spawn budget during startup meshing; aggressive throttling to 8
    // left most chunks without meshes for a long time.
    if time.elapsed_secs() < 12.0 {
        return;
    }

    let frame_ms = time.delta_secs() * 1000.0;
    budget.ema_ms = budget.ema_ms * 0.92 + frame_ms * 0.08;

    let target_ms = 16.0;
    let current = config.max_spawn_per_frame;

    if budget.ema_ms > target_ms * 1.25 && current > 16 {
        config.max_spawn_per_frame = current.saturating_sub(4);
    } else if budget.ema_ms < target_ms * 0.75 && current < 48 {
        config.max_spawn_per_frame = (current + 2).min(48);
    }
}
