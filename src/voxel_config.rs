use std::sync::Arc;

use bevy::prelude::*;
use bevy_voxel_world::prelude::*;

use crate::block::{BlockId, ATLAS_TEXTURE_COUNT};
use crate::world_gen::{terrain_lookup, ProceduralTerrain, WorldMetadata};

#[derive(Resource, Clone)]
pub struct BridgetWorld {
    pub seed: u32,
    pub spawning_distance: u32,
    terrain_lookup: Arc<dyn Fn(IVec3) -> WorldVoxel<u8> + Send + Sync>,
}

impl Default for BridgetWorld {
    fn default() -> Self {
        let terrain = ProceduralTerrain::new(42_424);
        Self {
            seed: 42_424,
            spawning_distance: 6,
            terrain_lookup: terrain.lookup(),
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

    fn voxel_lookup_delegate(&self) -> VoxelLookupDelegate<Self::MaterialIndex> {
        terrain_lookup(self.terrain_lookup.clone())
    }

    fn texture_index_mapper(
        &self,
    ) -> Arc<dyn Fn(u8) -> [u32; 3] + Send + Sync> {
        Arc::new(|material| {
            BlockId::from_material(material)
                .map(BlockId::texture_indices)
                .unwrap_or([0, 0, 0])
        })
    }

    fn voxel_texture(&self) -> Option<(String, u32)> {
        Some(("textures/voxel_atlas.png".into(), ATLAS_TEXTURE_COUNT))
    }
}

pub struct VoxelConfigPlugin;

impl Plugin for VoxelConfigPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BridgetWorld>();
        app.add_plugins(VoxelWorldPlugin::with_config(BridgetWorld::default()));
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
        config.terrain_lookup = terrain.lookup();
    }

    config.seed = metadata.seed;
    config.spawning_distance = settings.render_distance;
}
