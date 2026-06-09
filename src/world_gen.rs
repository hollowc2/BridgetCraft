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
            name: "Whispering Brickshire".to_string(),
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

const MEADOW_HEIGHT: i32 = 5;
const MEADOW_RADIUS: i32 = 40;

fn height_noise(seed: u32) -> HybridMulti<Perlin> {
    let mut height_noise = HybridMulti::<Perlin>::new(seed);
    height_noise.octaves = 3;
    height_noise.frequency = 0.35;
    height_noise.lacunarity = 2.0;
    height_noise.persistence = 0.4;
    height_noise
}

fn raw_terrain_height(seed: u32, x: i32, z: i32) -> i32 {
    let height_noise = height_noise(seed);
    let sample = height_noise.get([x as f64 * 0.035, z as f64 * 0.035]);
    MEADOW_HEIGHT + (sample * 2.0).round() as i32
}

pub fn terrain_surface_height(seed: u32, x: i32, z: i32) -> i32 {
    let height = raw_terrain_height(seed, x, z);
    let dist_sq = x * x + z * z;
    if dist_sq > MEADOW_RADIUS * MEADOW_RADIUS {
        return height;
    }

    let blend = dist_sq as f32 / (MEADOW_RADIUS * MEADOW_RADIUS) as f32;
    ((MEADOW_HEIGHT as f32 * (1.0 - blend) + height as f32 * blend).round()) as i32
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
    let cache = Mutex::new(HashMap::<(i32, i32), i32>::new());

    Arc::new(move |pos: IVec3| {
        let height = {
            let mut cache = cache.lock().unwrap();
            match cache.get(&(pos.x, pos.z)) {
                Some(height) => *height,
                None => {
                    let height = terrain_surface_height(seed, pos.x, pos.z);
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

fn hollow_glass_box(
    origin: IVec3,
    half_x: i32,
    half_z: i32,
    height: i32,
    edits: &mut Vec<(IVec3, WorldVoxel<u8>)>,
) {
    for y in 0..height {
        for x in -half_x..=half_x {
            for z in -half_z..=half_z {
                let on_wall = x.abs() == half_x || z.abs() == half_z;
                let on_floor = y == 0;
                if on_wall || on_floor {
                    edits.push((
                        origin + IVec3::new(x, y, z),
                        BlockId::Glass.to_world_voxel(),
                    ));
                }
            }
        }
    }
}

fn push_block(edits: &mut Vec<(IVec3, WorldVoxel<u8>)>, pos: IVec3, block: BlockId) {
    edits.push((pos, block.to_world_voxel()));
}

fn in_disk(x: i32, z: i32, radius: i32) -> bool {
    x * x + z * z <= radius * radius
}

fn on_disk_edge(x: i32, z: i32, radius: i32) -> bool {
    in_disk(x, z, radius) && !in_disk(x, z, radius - 1)
}

fn glass_lighthouse(origin: IVec3) -> Vec<(IVec3, WorldVoxel<u8>)> {
    let mut edits = Vec::new();

    for x in -4..=4 {
        for z in -4..=4 {
            if in_disk(x, z, 4) {
                push_block(&mut edits, origin + IVec3::new(x, 0, z), BlockId::Cobble);
            }
        }
    }

    let tiers = [(3, 0, 15), (2, 15, 8), (1, 23, 4)];
    for (radius, base_y, height) in tiers {
        for y in 0..height {
            for x in -radius..=radius {
                for z in -radius..=radius {
                    if !on_disk_edge(x, z, radius) {
                        continue;
                    }
                    let level_y = base_y + y;
                    let band = level_y > 0 && level_y % 5 == 0;
                    push_block(
                        &mut edits,
                        origin + IVec3::new(x, level_y, z),
                        if band {
                            BlockId::BrickRed
                        } else {
                            BlockId::Glass
                        },
                    );
                }
            }
        }
    }

    let lantern_y = 27;
    for x in -2..=2 {
        for z in -2..=2 {
            if on_disk_edge(x, z, 2) || (x == 0 && z == 0) {
                push_block(
                    &mut edits,
                    origin + IVec3::new(x, lantern_y, z),
                    BlockId::Glass,
                );
            }
        }
    }
    for x in -1..=1 {
        for z in -1..=1 {
            push_block(
                &mut edits,
                origin + IVec3::new(x, lantern_y + 1, z),
                BlockId::Glowstone,
            );
        }
    }
    push_block(&mut edits, origin + IVec3::new(0, lantern_y + 2, 0), BlockId::Glass);

    edits
}

fn glass_arch_bridge(origin: IVec3) -> Vec<(IVec3, WorldVoxel<u8>)> {
    let mut edits = Vec::new();
    let deck_y = 10;

    for x in -14..=14 {
        for z in -1..=1 {
            push_block(
                &mut edits,
                origin + IVec3::new(x, deck_y, z),
                BlockId::Planks,
            );
            push_block(
                &mut edits,
                origin + IVec3::new(x, deck_y + 1, z),
                BlockId::Glass,
            );
        }
    }

    for pylon_x in [-13, 13] {
        for y in 0..deck_y {
            for dx in -2i32..=2 {
                for dz in -2i32..=2 {
                    let on_face = dx.abs() == 2 || dz.abs() == 2;
                    if on_face {
                        push_block(
                            &mut edits,
                            origin + IVec3::new(pylon_x + dx, y, dz),
                            BlockId::BrickGrey,
                        );
                    }
                }
            }
        }
        for dz in -1..=1 {
            push_block(
                &mut edits,
                origin + IVec3::new(pylon_x, deck_y + 2, dz),
                BlockId::Glowstone,
            );
        }
    }

    for x in -14..=14 {
        let t = x as f32 / 14.0;
        let arch_height = ((1.0 - t * t).max(0.0) * 9.0).round() as i32;
        for y in 0..=arch_height {
            push_block(
                &mut edits,
                origin + IVec3::new(x, y, 0),
                BlockId::Glass,
            );
        }
    }

    for side in [-1, 1] {
        for step in 0..=12 {
            let x = side * (13 - step);
            let y = deck_y + 2 - step / 2;
            push_block(
                &mut edits,
                origin + IVec3::new(x, y, side * 2),
                BlockId::Wool,
            );
        }
    }

    for x in -12..=12 {
        for z in -2..=2 {
            for y in -1..=0 {
                push_block(
                    &mut edits,
                    origin + IVec3::new(x, y, z),
                    BlockId::Water,
                );
            }
        }
    }

    edits
}

fn glass_observatory(origin: IVec3) -> Vec<(IVec3, WorldVoxel<u8>)> {
    let mut edits = Vec::new();
    let platform_half = 9;

    for x in -platform_half..=platform_half {
        for z in -platform_half..=platform_half {
            push_block(&mut edits, origin + IVec3::new(x, 0, z), BlockId::Stone);
            if x.abs() == platform_half || z.abs() == platform_half {
                push_block(
                    &mut edits,
                    origin + IVec3::new(x, 1, z),
                    BlockId::BrickGrey,
                );
            }
        }
    }

    let tower_positions = [(-7, -7), (7, -7), (-7, 7), (7, 7)];
    for (tx, tz) in tower_positions {
        for y in 1..=18 {
            for dx in -1i32..=1 {
                for dz in -1i32..=1 {
                    let on_wall = dx.abs() == 1 || dz.abs() == 1;
                    if on_wall {
                        push_block(
                            &mut edits,
                            origin + IVec3::new(tx + dx, y, tz + dz),
                            BlockId::Glass,
                        );
                    }
                }
            }
        }
        for dx in -1..=1 {
            for dz in -1..=1 {
                push_block(
                    &mut edits,
                    origin + IVec3::new(tx + dx, 19, tz + dz),
                    BlockId::BrickRed,
                );
            }
        }
        push_block(
            &mut edits,
            origin + IVec3::new(tx, 20, tz),
            BlockId::Glowstone,
        );
    }

    let ring_y = 14;
    for x in -7..=7 {
        push_block(
            &mut edits,
            origin + IVec3::new(x, ring_y, -7),
            BlockId::Glass,
        );
        push_block(
            &mut edits,
            origin + IVec3::new(x, ring_y, 7),
            BlockId::Glass,
        );
    }
    for z in -6..=6 {
        push_block(
            &mut edits,
            origin + IVec3::new(-7, ring_y, z),
            BlockId::Glass,
        );
        push_block(
            &mut edits,
            origin + IVec3::new(7, ring_y, z),
            BlockId::Glass,
        );
    }

    for (tx, tz) in tower_positions {
        for step in 1..=6 {
            let px = tx.signum() * (tx.abs() - step);
            let pz = tz.signum() * (tz.abs() - step);
            push_block(
                &mut edits,
                origin + IVec3::new(px, 2 + step, pz),
                BlockId::Cobble,
            );
        }
    }

    let dome_radius = 6;
    let dome_base_y = 2;
    for y in 0..=dome_radius {
        let slice_radius = {
            let t = y as f32 / dome_radius as f32;
            ((1.0 - t * t).max(0.0).sqrt() * dome_radius as f32).round() as i32
        };
        for x in -slice_radius..=slice_radius {
            for z in -slice_radius..=slice_radius {
                if !on_disk_edge(x, z, slice_radius) {
                    continue;
                }
                let pos = origin + IVec3::new(x, dome_base_y + y, z);
                let meridian = x == 0 || z == 0 || x.abs() == z.abs();
                push_block(
                    &mut edits,
                    pos,
                    if meridian && y % 2 == 0 {
                        BlockId::Glowstone
                    } else {
                        BlockId::Glass
                    },
                );
            }
        }
    }

    for y in 1..=4 {
        push_block(
            &mut edits,
            origin + IVec3::new(0, y, 0),
            BlockId::Glowstone,
        );
    }
    push_block(&mut edits, origin + IVec3::new(0, dome_base_y + dome_radius + 1, 0), BlockId::Glass);

    edits
}

fn glass_pyramid(origin: IVec3, half_base: i32) -> Vec<(IVec3, WorldVoxel<u8>)> {
    let mut edits = Vec::new();
    let mut half = half_base;
    let mut y = 0;

    while half >= 0 {
        for x in -half..=half {
            for z in -half..=half {
                if x.abs() == half || z.abs() == half || y == 0 {
                    edits.push((
                        origin + IVec3::new(x, y, z),
                        BlockId::Glass.to_world_voxel(),
                    ));
                }
            }
        }
        half -= 1;
        y += 1;
    }

    edits.push((
        origin + IVec3::new(0, y, 0),
        BlockId::Glowstone.to_world_voxel(),
    ));

    edits
}

fn glass_empire_tower(origin: IVec3) -> Vec<(IVec3, WorldVoxel<u8>)> {
    let mut edits = Vec::new();
    let sections = [(5, 22), (4, 18), (3, 14), (2, 12), (1, 16)];

    let mut y = 0;
    for (half, section_height) in sections {
        hollow_glass_box(origin + IVec3::new(0, y, 0), half, half, section_height, &mut edits);
        y += section_height;
    }

    for spire_y in 0..4 {
        edits.push((
            origin + IVec3::new(0, y + spire_y, 0),
            BlockId::Glass.to_world_voxel(),
        ));
    }
    edits.push((
        origin + IVec3::new(0, y + 4, 0),
        BlockId::Glowstone.to_world_voxel(),
    ));

    edits
}

pub fn decorate_landmarks(seed: u32) -> Vec<(IVec3, WorldVoxel<u8>)> {
    let pyramid_center = IVec3::new(34, terrain_surface_height(seed, 34, -22) + 1, -22);
    let tower_center = IVec3::new(-32, terrain_surface_height(seed, -32, 26) + 1, 26);
    let lighthouse_center = IVec3::new(24, terrain_surface_height(seed, 24, 30) + 1, 30);
    let bridge_center = IVec3::new(0, terrain_surface_height(seed, 0, -34) + 1, -34);
    let observatory_center = IVec3::new(-22, terrain_surface_height(seed, -22, -22) + 1, -22);

    let mut edits = glass_pyramid(pyramid_center, 10);
    edits.extend(glass_empire_tower(tower_center));
    edits.extend(glass_lighthouse(lighthouse_center));
    edits.extend(glass_arch_bridge(bridge_center));
    edits.extend(glass_observatory(observatory_center));
    edits
}

pub fn world_base_voxels(seed: u32) -> Vec<(IVec3, WorldVoxel<u8>)> {
    let mut voxels = decorate_trees(seed, IVec3::ZERO, 48);
    voxels.extend(decorate_landmarks(seed));
    voxels
}
