use std::sync::Arc;

use bevy::prelude::*;
use bevy_voxel_world::custom_meshing::{CHUNK_SIZE_F, CHUNK_SIZE_U};
use bevy_voxel_world::prelude::*;

use crate::block::texture_index_table;
use crate::world_gen::{terrain_lookup, ProceduralTerrain, WorldMetadata};

const LOD_DISTANCE_FRACTION_NEAR: f32 = 0.45;
const LOD_DISTANCE_FRACTION_MID: f32 = 0.75;
const LOD_HYSTERESIS: f32 = 0.08;
const MIN_CHUNK_INTERIOR: u32 = 8;

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

    fn chunk_lod(
        &self,
        chunk_position: IVec3,
        previous_lod: Option<LodLevel>,
        camera_position: Vec3,
    ) -> LodLevel {
        let chunk_center =
            chunk_position.as_vec3() * CHUNK_SIZE_F + Vec3::splat(CHUNK_SIZE_F * 0.5);
        let distance = chunk_center.distance(camera_position);
        let render_extent = self.spawning_distance as f32 * CHUNK_SIZE_F;
        let near_cutoff = render_extent * LOD_DISTANCE_FRACTION_NEAR;
        let mid_cutoff = render_extent * LOD_DISTANCE_FRACTION_MID;

        let target = if distance >= mid_cutoff {
            2
        } else if distance >= near_cutoff {
            1
        } else {
            0
        };

        match previous_lod {
            Some(previous) if previous > target => {
                let threshold = match target {
                    0 => near_cutoff * (1.0 - LOD_HYSTERESIS),
                    1 => mid_cutoff * (1.0 - LOD_HYSTERESIS),
                    _ => f32::MAX,
                };
                if distance < threshold {
                    previous
                } else {
                    target
                }
            }
            Some(previous) if previous < target => {
                let threshold = match target {
                    1 => near_cutoff * (1.0 + LOD_HYSTERESIS),
                    2 => mid_cutoff * (1.0 + LOD_HYSTERESIS),
                    _ => 0.0,
                };
                if distance > threshold {
                    previous
                } else {
                    target
                }
            }
            _ => target,
        }
    }

    fn chunk_data_shape(&self, lod_level: LodLevel) -> UVec3 {
        let shift = lod_level.min(2);
        let interior = (CHUNK_SIZE_U >> shift).max(MIN_CHUNK_INTERIOR);
        padded_chunk_shape_uniform(interior)
    }

    fn chunk_meshing_shape(&self, lod_level: LodLevel) -> UVec3 {
        self.chunk_data_shape(lod_level)
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
    let frame_ms = time.delta_secs() * 1000.0;
    budget.ema_ms = budget.ema_ms * 0.92 + frame_ms * 0.08;

    let target_ms = 16.0;
    let current = config.max_spawn_per_frame;

    if budget.ema_ms > target_ms * 1.25 && current > 8 {
        config.max_spawn_per_frame = current.saturating_sub(4);
    } else if budget.ema_ms < target_ms * 0.75 && current < 48 {
        config.max_spawn_per_frame = (current + 2).min(48);
    }
}
