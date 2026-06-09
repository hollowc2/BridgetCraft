use std::sync::{Arc, Mutex};

use bevy::platform::collections::HashMap;
use serde::{Deserialize, Serialize};
use bevy::prelude::*;
use bevy_voxel_world::prelude::*;
use noise::{HybridMulti, NoiseFn, Perlin};

use crate::block::BlockId;

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct WorldMetadata {
    pub name: String,
    pub seed: u32,
}

impl Default for WorldMetadata {
    fn default() -> Self {
        Self {
            name: "New World".to_string(),
            seed: 42_424,
        }
    }
}

pub fn terrain_lookup(seed: u32) -> VoxelLookupDelegate<u8> {
    let lookup = terrain_voxel_lookup(seed);
    Box::new(move |_chunk_pos, _lod, _previous| {
        let lookup = lookup.clone();
        Box::new(move |pos: IVec3, _previous| lookup(pos))
    })
}

fn height_noise(seed: u32) -> HybridMulti<Perlin> {
    let mut height_noise = HybridMulti::<Perlin>::new(seed);
    height_noise.octaves = 4;
    height_noise.frequency = 0.35;
    height_noise.lacunarity = 2.0;
    height_noise.persistence = 0.45;
    height_noise
}

pub fn terrain_surface_height(seed: u32, x: i32, z: i32) -> i32 {
    let height_noise = height_noise(seed);
    let sample = height_noise.get([x as f64 * 0.04, z as f64 * 0.04]);
    4 + (sample * 6.0).round() as i32
}

pub fn terrain_voxel_at(pos: IVec3, height: i32) -> WorldVoxel<u8> {
    if pos.y < 0 {
        return WorldVoxel::Solid(BlockId::Stone.as_material());
    }

    if pos.y > height {
        return WorldVoxel::Air;
    }

    if pos.y == height {
        if height <= 5 {
            return WorldVoxel::Solid(BlockId::Sand.as_material());
        }
        return WorldVoxel::Solid(BlockId::DirtGrass.as_material());
    }

    if pos.y >= height - 3 {
        return WorldVoxel::Solid(BlockId::Dirt.as_material());
    }

    WorldVoxel::Solid(BlockId::Stone.as_material())
}

pub fn terrain_voxel_lookup(seed: u32) -> Arc<dyn Fn(IVec3) -> WorldVoxel<u8> + Send + Sync> {
    let height_noise = height_noise(seed);
    let cache = Mutex::new(HashMap::<(i32, i32), i32>::new());

    Arc::new(move |pos: IVec3| {
        let height = {
            let mut cache = cache.lock().unwrap();
            match cache.get(&(pos.x, pos.z)) {
                Some(height) => *height,
                None => {
                    let sample = height_noise.get([pos.x as f64 * 0.04, pos.z as f64 * 0.04]);
                    let height = 4 + (sample * 6.0).round() as i32;
                    cache.insert((pos.x, pos.z), height);
                    height
                }
            }
        };

        terrain_voxel_at(pos, height)
    })
}

pub fn decorate_trees(seed: u32, center: IVec3, radius: i32) -> Vec<(IVec3, WorldVoxel<u8>)> {
    let tree_noise = Perlin::new(seed.wrapping_add(77_007));
    let mut edits = Vec::new();

    for x in (center.x - radius)..=(center.x + radius) {
        for z in (center.z - radius)..=(center.z + radius) {
            let density = tree_noise.get([x as f64 * 0.17, z as f64 * 0.17]);
            if density < 0.62 {
                continue;
            }

            let surface = terrain_surface_height(seed, x, z);

            for trunk_y in 1..=4 {
                edits.push((
                    IVec3::new(x, surface + trunk_y, z),
                    BlockId::Trunk.to_world_voxel(),
                ));
            }

            for dx in -2..=2 {
                for dy in 2..=5 {
                    for dz in -2..=2 {
                        if dx * dx + dy * dy + dz * dz > 8 {
                            continue;
                        }
                        edits.push((
                            IVec3::new(x + dx, surface + dy, z + dz),
                            BlockId::Leaves.to_world_voxel(),
                        ));
                    }
                }
            }
        }
    }

    edits
}
