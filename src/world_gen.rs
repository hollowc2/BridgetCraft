use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static TERRAIN_GENERATION: AtomicU64 = AtomicU64::new(1);

use bevy::platform::collections::HashMap;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use bevy::prelude::*;
use bevy_voxel_world::custom_meshing::CHUNK_SIZE_I;
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

#[derive(Clone, Copy)]
struct LandmarkOrigins {
    pyramid: IVec3,
    tower: IVec3,
    lighthouse: IVec3,
    bridge: IVec3,
    observatory: IVec3,
    city: IVec3,
    colosseum: IVec3,
    clock_tower: IVec3,
    windmill: IVec3,
    castle: IVec3,
}

fn landmark_origins(height_noise: &HybridMulti<Perlin>) -> LandmarkOrigins {
    let surface = |x: i32, z: i32| terrain_surface_height_with(height_noise, x, z);
    LandmarkOrigins {
        pyramid: IVec3::new(34, surface(34, -22) + 1, -22),
        tower: IVec3::new(-32, surface(-32, 26) + 1, 26),
        lighthouse: IVec3::new(24, surface(24, 30) + 1, 30),
        bridge: IVec3::new(0, surface(0, -34) + 1, -34),
        observatory: IVec3::new(-22, surface(-22, -22) + 1, -22),
        city: IVec3::new(75, surface(75, 0) + 1, 0),
        colosseum: IVec3::new(0, surface(0, -70) + 1, -70),
        clock_tower: IVec3::new(-65, surface(-65, 30) + 1, 30),
        windmill: IVec3::new(-30, surface(-30, -65) + 1, -65),
        castle: IVec3::new(60, surface(60, 60) + 1, 60),
    }
}

const TREE_LEAF_CLEARANCE: i32 = 6;
const TREE_DENSITY_THRESHOLD: f64 = 0.62;

/// Cached procedural terrain data. Rebuilt only when the world seed changes.
#[derive(Resource, Clone)]
pub struct ProceduralTerrain {
    pub generation: u64,
    pub seed: u32,
    height_noise: Arc<HybridMulti<Perlin>>,
    height_cache: Arc<DashMap<(i32, i32), i32>>,
    tree_noise: Arc<Perlin>,
    landmarks: LandmarkOrigins,
}

fn near_landmark(pos: IVec3, origins: LandmarkOrigins) -> bool {
    let checks = [
        (origins.pyramid, 12, 10),
        (origins.tower, 86, 5),
        (origins.lighthouse, 29, 4),
        (origins.bridge, 13, 14),
        (origins.observatory, 22, 9),
        (origins.city, 28, 28),
        (origins.colosseum, 18, 16),
        (origins.clock_tower, 48, 5),
        (origins.windmill, 22, 7),
        (origins.castle, 22, 14),
    ];

    for (origin, max_y, horizontal) in checks {
        let local = pos - origin;
        if local.y >= -1 && local.y <= max_y && local.x.abs() <= horizontal && local.z.abs() <= horizontal
        {
            return true;
        }
    }

    false
}

impl Default for ProceduralTerrain {
    fn default() -> Self {
        Self::new(42_424)
    }
}

impl ProceduralTerrain {
    pub fn new(seed: u32) -> Self {
        let height_noise = Arc::new(build_height_noise(seed));
        // Sharded concurrent caches: chunk meshing runs on the Bevy compute task pool, and a
        // single global Mutex here serialized every thread, stalling startup. DashMap shards the
        // locks so parallel voxel lookups no longer contend on one lock.
        let height_cache: Arc<DashMap<(i32, i32), i32>> = Arc::new(DashMap::new());
        let tree_noise = Arc::new(Perlin::new(seed.wrapping_add(77_007)));
        let landmarks = landmark_origins(&height_noise);

        Self {
            generation: TERRAIN_GENERATION.fetch_add(1, Ordering::Relaxed),
            seed,
            height_noise,
            height_cache,
            tree_noise,
            landmarks,
        }
    }

    pub fn surface_height(&self, x: i32, z: i32) -> i32 {
        *self
            .height_cache
            .entry((x, z))
            .or_insert_with(|| terrain_surface_height_with(&self.height_noise, x, z))
    }

    pub fn voxel_at(&self, pos: IVec3) -> WorldVoxel<u8> {
        let height = self.surface_height(pos.x, pos.z);
        procedural_voxel_at(
            pos,
            height,
            self.landmarks,
            &self.height_noise,
            &self.tree_noise,
        )
    }
}

fn procedural_voxel_at(
    pos: IVec3,
    height: i32,
    landmarks: LandmarkOrigins,
    height_noise: &HybridMulti<Perlin>,
    tree_noise: &Perlin,
) -> WorldVoxel<u8> {
    if pos.y > height + TREE_LEAF_CLEARANCE {
        WorldVoxel::Air
    } else if pos.y < 0 {
        WorldVoxel::Solid(BlockId::Stone.as_material())
    } else if pos.y <= height + TREE_LEAF_CLEARANCE {
        decoration_voxel_at(landmarks, height_noise, tree_noise, pos, height)
            .unwrap_or_else(|| terrain_voxel_at(pos, height))
    } else {
        terrain_voxel_at(pos, height)
    }
}

fn lod_sample_step(lod: LodLevel) -> i32 {
    1i32 << lod.min(2)
}

fn lod_snap(pos: IVec3, step: i32) -> IVec3 {
    if step <= 1 {
        return pos;
    }

    IVec3::new(
        pos.x.div_euclid(step) * step,
        pos.y.div_euclid(step) * step,
        pos.z.div_euclid(step) * step,
    )
}

pub fn terrain_lookup(terrain: Arc<ProceduralTerrain>) -> VoxelLookupDelegate<u8> {
    Box::new(move |chunk_pos, lod, _previous| {
        let terrain = terrain.clone();
        let step = lod_sample_step(lod);
        let landmarks = terrain.landmarks;
        let height_noise = terrain.height_noise.clone();
        let tree_noise = terrain.tree_noise.clone();

        let origin = chunk_pos * CHUNK_SIZE_I;
        let min_x = origin.x - 1;
        let max_x = origin.x + CHUNK_SIZE_I as i32 + 1;
        let min_z = origin.z - 1;
        let max_z = origin.z + CHUNK_SIZE_I as i32 + 1;
        let column_count = ((max_x - min_x + 1) * (max_z - min_z + 1)) as usize;

        let mut local_heights = HashMap::with_capacity(column_count);
        for x in min_x..=max_x {
            for z in min_z..=max_z {
                local_heights.insert((x, z), terrain.surface_height(x, z));
            }
        }

        Box::new(move |pos: IVec3, _previous| {
            let query = lod_snap(pos, step);
            let height = local_heights
                .get(&(query.x, query.z))
                .copied()
                .unwrap_or_else(|| terrain.surface_height(query.x, query.z));
            procedural_voxel_at(query, height, landmarks, &height_noise, &tree_noise)
        })
    })
}

const MEADOW_HEIGHT: i32 = 5;
const MEADOW_RADIUS: i32 = 40;

fn build_height_noise(seed: u32) -> HybridMulti<Perlin> {
    let mut height_noise = HybridMulti::<Perlin>::new(seed);
    height_noise.octaves = 3;
    height_noise.frequency = 0.35;
    height_noise.lacunarity = 2.0;
    height_noise.persistence = 0.4;
    height_noise
}

fn raw_terrain_height(noise: &HybridMulti<Perlin>, x: i32, z: i32) -> i32 {
    let sample = noise.get([x as f64 * 0.035, z as f64 * 0.035]);
    MEADOW_HEIGHT + (sample * 2.0).round() as i32
}

fn terrain_surface_height_with(noise: &HybridMulti<Perlin>, x: i32, z: i32) -> i32 {
    let height = raw_terrain_height(noise, x, z);
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

fn decoration_voxel_at(
    landmarks: LandmarkOrigins,
    height_noise: &HybridMulti<Perlin>,
    tree_noise: &Perlin,
    pos: IVec3,
    surface_height: i32,
) -> Option<WorldVoxel<u8>> {
    if near_landmark(pos, landmarks) {
        if let Some(block) = landmark_voxel_at(landmarks, pos) {
            return Some(block.to_world_voxel());
        }
    }

    if pos.y <= surface_height + TREE_LEAF_CLEARANCE {
        tree_voxel_at(height_noise, tree_noise, pos).map(|block| block.to_world_voxel())
    } else {
        None
    }
}

fn tree_surface_at(
    height_noise: &HybridMulti<Perlin>,
    tree_noise: &Perlin,
    x: i32,
    z: i32,
) -> Option<i32> {
    let density = tree_noise.get([x as f64 * 0.17, z as f64 * 0.17]);
    if density < TREE_DENSITY_THRESHOLD {
        return None;
    }

    Some(terrain_surface_height_with(height_noise, x, z))
}

fn tree_voxel_at(
    height_noise: &HybridMulti<Perlin>,
    tree_noise: &Perlin,
    pos: IVec3,
) -> Option<BlockId> {
    if let Some(surface) = tree_surface_at(height_noise, tree_noise, pos.x, pos.z) {
        let trunk_y = pos.y - surface;
        if (1..=4).contains(&trunk_y) {
            return Some(BlockId::Trunk);
        }
    }

    for x in (pos.x - 2)..=(pos.x + 2) {
        for z in (pos.z - 2)..=(pos.z + 2) {
            let Some(surface) = tree_surface_at(height_noise, tree_noise, x, z) else {
                continue;
            };

            let dx = pos.x - x;
            let dy = pos.y - surface;
            let dz = pos.z - z;
            if (2..=5).contains(&dy) && dx * dx + dy * dy + dz * dz <= 8 {
                return Some(BlockId::Leaves);
            }
        }
    }

    None
}

fn hollow_glass_box_local(
    local: IVec3,
    y_offset: i32,
    half_x: i32,
    half_z: i32,
    height: i32,
) -> Option<BlockId> {
    let y = local.y - y_offset;
    if y < 0 || y >= height || local.x.abs() > half_x || local.z.abs() > half_z {
        return None;
    }

    let on_wall = local.x.abs() == half_x || local.z.abs() == half_z;
    let on_floor = y == 0;
    if on_wall || on_floor {
        Some(BlockId::Glass)
    } else {
        None
    }
}

fn in_disk(x: i32, z: i32, radius: i32) -> bool {
    x * x + z * z <= radius * radius
}

fn on_disk_edge(x: i32, z: i32, radius: i32) -> bool {
    in_disk(x, z, radius) && !in_disk(x, z, radius - 1)
}

fn pyramid_voxel_at(origin: IVec3, half_base: i32, pos: IVec3) -> Option<BlockId> {
    let local = pos - origin;
    if local.y < 0 || local.x.abs() > half_base || local.z.abs() > half_base {
        return None;
    }

    let mut half = half_base;
    let mut y = 0;
    while half >= 0 {
        if local.y == y {
            if local.x.abs() <= half && local.z.abs() <= half {
                if local.x.abs() == half || local.z.abs() == half || y == 0 {
                    return Some(BlockId::Glass);
                }
            }
            return None;
        }
        half -= 1;
        y += 1;
    }

    if local.x == 0 && local.z == 0 && local.y == y {
        Some(BlockId::Glowstone)
    } else {
        None
    }
}

fn lighthouse_voxel_at(origin: IVec3, pos: IVec3) -> Option<BlockId> {
    let local = pos - origin;

    if local.y == 0 && in_disk(local.x, local.z, 4) {
        return Some(BlockId::Cobble);
    }

    let tiers = [(3, 0, 15), (2, 15, 8), (1, 23, 4)];
    for (radius, base_y, height) in tiers {
        if local.y < base_y || local.y >= base_y + height {
            continue;
        }
        if local.x.abs() > radius || local.z.abs() > radius {
            continue;
        }
        if !on_disk_edge(local.x, local.z, radius) {
            continue;
        }
        let band = local.y > 0 && local.y % 5 == 0;
        return Some(if band {
            BlockId::BrickRed
        } else {
            BlockId::Glass
        });
    }

    let lantern_y = 27;
    if local.y == lantern_y
        && (on_disk_edge(local.x, local.z, 2) || (local.x == 0 && local.z == 0))
    {
        return Some(BlockId::Glass);
    }
    if local.y == lantern_y + 1 && local.x.abs() <= 1 && local.z.abs() <= 1 {
        return Some(BlockId::Glowstone);
    }
    if local.y == lantern_y + 2 && local.x == 0 && local.z == 0 {
        return Some(BlockId::Glass);
    }

    None
}

fn bridge_voxel_at(origin: IVec3, pos: IVec3) -> Option<BlockId> {
    let local = pos - origin;
    let deck_y = 10;

    if local.y == deck_y && local.x.abs() <= 14 && local.z.abs() <= 1 {
        return Some(BlockId::Planks);
    }
    if local.y == deck_y + 1 && local.x.abs() <= 14 && local.z.abs() <= 1 {
        return Some(BlockId::Glass);
    }

    for pylon_x in [-13, 13] {
        if local.x >= pylon_x - 2 && local.x <= pylon_x + 2 && local.z.abs() <= 2 {
            if local.y < deck_y {
                let dx = local.x - pylon_x;
                let on_face = dx.abs() == 2 || local.z.abs() == 2;
                if on_face {
                    return Some(BlockId::BrickGrey);
                }
            }
            if local.y == deck_y + 2 && local.z.abs() <= 1 {
                return Some(BlockId::Glowstone);
            }
        }
    }

    if local.z == 0 && local.x.abs() <= 14 && local.y >= 0 {
        let t = local.x as f32 / 14.0;
        let arch_height = ((1.0 - t * t).max(0.0) * 9.0).round() as i32;
        if local.y <= arch_height {
            return Some(BlockId::Glass);
        }
    }

    for side in [-1, 1] {
        for step in 0..=12 {
            let x = side * (13 - step);
            let y = deck_y + 2 - step / 2;
            if local.x == x && local.y == y && local.z == side * 2 {
                return Some(BlockId::Wool);
            }
        }
    }

    if local.y >= -1
        && local.y <= 0
        && local.x.abs() <= 12
        && local.z.abs() <= 2
    {
        return Some(BlockId::Water);
    }

    None
}

fn observatory_voxel_at(origin: IVec3, pos: IVec3) -> Option<BlockId> {
    let local = pos - origin;
    let platform_half = 9;

    if local.y == 0 && local.x.abs() <= platform_half && local.z.abs() <= platform_half {
        return Some(BlockId::Stone);
    }
    if local.y == 1
        && local.x.abs() <= platform_half
        && local.z.abs() <= platform_half
        && (local.x.abs() == platform_half || local.z.abs() == platform_half)
    {
        return Some(BlockId::BrickGrey);
    }

    let tower_positions = [(-7, -7), (7, -7), (-7, 7), (7, 7)];
    for (tx, tz) in tower_positions {
        let dx = local.x - tx;
        let dz = local.z - tz;
        if local.y >= 1 && local.y <= 18 && dx.abs() <= 1 && dz.abs() <= 1 {
            if dx.abs() == 1 || dz.abs() == 1 {
                return Some(BlockId::Glass);
            }
        }
        if local.y == 19 && dx.abs() <= 1 && dz.abs() <= 1 {
            return Some(BlockId::BrickRed);
        }
        if local.y == 20 && dx == 0 && dz == 0 {
            return Some(BlockId::Glowstone);
        }
    }

    let ring_y = 14;
    if local.y == ring_y {
        if local.x.abs() <= 7 && local.z == -7 {
            return Some(BlockId::Glass);
        }
        if local.x.abs() <= 7 && local.z == 7 {
            return Some(BlockId::Glass);
        }
        if local.z.abs() <= 6 && local.x == -7 {
            return Some(BlockId::Glass);
        }
        if local.z.abs() <= 6 && local.x == 7 {
            return Some(BlockId::Glass);
        }
    }

    for (tx, tz) in tower_positions {
        for step in 1..=6 {
            let px = tx.signum() * (tx.abs() - step);
            let pz = tz.signum() * (tz.abs() - step);
            if local.x == px && local.z == pz && local.y == 2 + step {
                return Some(BlockId::Cobble);
            }
        }
    }

    let dome_radius = 6;
    let dome_base_y = 2;
    if local.y >= dome_base_y && local.y <= dome_base_y + dome_radius {
        let y = local.y - dome_base_y;
        let t = y as f32 / dome_radius as f32;
        let slice_radius = ((1.0 - t * t).max(0.0).sqrt() * dome_radius as f32).round() as i32;
        if local.x.abs() <= slice_radius
            && local.z.abs() <= slice_radius
            && on_disk_edge(local.x, local.z, slice_radius)
        {
            let meridian = local.x == 0 || local.z == 0 || local.x.abs() == local.z.abs();
            return Some(if meridian && y % 2 == 0 {
                BlockId::Glowstone
            } else {
                BlockId::Glass
            });
        }
    }

    if local.x == 0 && local.z == 0 && (1..=4).contains(&local.y) {
        return Some(BlockId::Glowstone);
    }
    if local.x == 0
        && local.z == 0
        && local.y == dome_base_y + dome_radius + 1
    {
        return Some(BlockId::Glass);
    }

    None
}

fn pos_hash(x: i32, z: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(3_748_279);
    h = h.wrapping_add((z as u32).wrapping_mul(7_507_201));
    h ^ (h >> 13)
}

fn hollow_box_local(
    local: IVec3,
    y_offset: i32,
    half_x: i32,
    half_z: i32,
    height: i32,
    wall: BlockId,
    floor: BlockId,
) -> Option<BlockId> {
    let y = local.y - y_offset;
    if y < 0 || y >= height || local.x.abs() > half_x || local.z.abs() > half_z {
        return None;
    }

    let on_wall = local.x.abs() == half_x || local.z.abs() == half_z;
    let on_floor = y == 0;
    if on_wall {
        Some(wall)
    } else if on_floor {
        Some(floor)
    } else {
        None
    }
}

fn city_building_voxel_at(local: IVec3, bx: i32, bz: i32) -> Option<BlockId> {
    let rel = IVec3::new(local.x - bx, local.y, local.z - bz);
    let hash = pos_hash(bx, bz);
    let height = 6 + (hash % 14) as i32;
    let half_x = 2 + (hash % 3) as i32;
    let half_z = 2 + ((hash >> 4) % 3) as i32;
    let brick = if hash % 2 == 0 {
        BlockId::BrickRed
    } else {
        BlockId::BrickGrey
    };

    if rel.y == 0 && rel.x.abs() <= half_x && rel.z.abs() <= half_z {
        return Some(BlockId::Cobble);
    }

    if rel.y >= 1 && rel.y <= height {
        if rel.x.abs() <= half_x && rel.z.abs() <= half_z {
            let on_face = rel.x.abs() == half_x || rel.z.abs() == half_z;
            let on_roof = rel.y == height;
            if on_face || on_roof {
                return Some(brick);
            }
            if rel.y % 4 == 0 && rel.x.abs() <= half_x - 1 && rel.z.abs() <= half_z - 1 {
                return Some(BlockId::Glass);
            }
        }
    }

    if rel.y == height + 1 && rel.x == 0 && rel.z == 0 {
        return Some(BlockId::Glowstone);
    }

    None
}

fn city_voxel_at(origin: IVec3, pos: IVec3) -> Option<BlockId> {
    let local = pos - origin;
    let half = 28;

    if local.y == 0 && local.x.abs() <= half && local.z.abs() <= half {
        let on_street_x = local.x % 8 == 0;
        let on_street_z = local.z % 8 == 0;
        if on_street_x || on_street_z {
            return Some(BlockId::Cobble);
        }
        if local.x.abs() <= 4 && local.z.abs() <= 4 {
            return Some(BlockId::Water);
        }
        return Some(BlockId::GrassDecor);
    }

    if local.y == 1 && local.x.abs() <= 3 && local.z.abs() <= 3 {
        return Some(BlockId::Stone);
    }
    if local.y == 2 && local.x == 0 && local.z == 0 {
        return Some(BlockId::Glowstone);
    }

    if (local.x % 8 == 4) && (local.z % 8 == 4) && local.y >= 1 && local.y <= 3 {
        if local.y == 3 && local.x.abs() <= half && local.z.abs() <= half {
            return Some(BlockId::Glowstone);
        }
        if local.y < 3 {
            return Some(BlockId::TrunkWhite);
        }
    }

    for bx in (-24i32..=24).step_by(8) {
        for bz in (-24i32..=24).step_by(8) {
            if bx.abs() <= 3 && bz.abs() <= 3 {
                continue;
            }
            if let Some(block) = city_building_voxel_at(local, bx, bz) {
                return Some(block);
            }
        }
    }

    None
}

fn colosseum_voxel_at(origin: IVec3, pos: IVec3) -> Option<BlockId> {
    let local = pos - origin;
    let outer_rx = 14;
    let outer_rz = 10;
    let inner_rx = 9;
    let inner_rz = 6;

    let in_outer = (local.x * local.x) * inner_rz * inner_rz
        + (local.z * local.z) * outer_rx * outer_rx
        <= outer_rx * outer_rx * inner_rz * inner_rz;
    let in_inner = (local.x * local.x) * inner_rz * inner_rz
        + (local.z * local.z) * inner_rx * inner_rx
        <= inner_rx * inner_rx * inner_rz * inner_rz;

    if local.y == 0 && in_outer {
        return Some(if in_inner {
            BlockId::Sand
        } else {
            BlockId::Cobble
        });
    }

    for tier in 0..4 {
        let base_y = 1 + tier * 4;
        let tier_rx = outer_rx - tier;
        let tier_rz = outer_rz - tier / 2;
        let tier_inner_rx = inner_rx - tier;
        let tier_inner_rz = inner_rz - tier / 2;

        let in_tier_outer = (local.x * local.x) * tier_inner_rz * tier_inner_rz
            + (local.z * local.z) * tier_rx * tier_rx
            <= tier_rx * tier_rx * tier_inner_rz * tier_inner_rz;
        let in_tier_inner = (local.x * local.x) * tier_inner_rz * tier_inner_rz
            + (local.z * local.z) * tier_inner_rx * tier_inner_rx
            <= tier_inner_rx * tier_inner_rx * tier_inner_rz * tier_inner_rz;

        if local.y >= base_y && local.y < base_y + 4 {
            if in_tier_outer && !in_tier_inner {
                let on_face = local.x.abs() >= tier_rx - 1 || local.z.abs() >= tier_rz - 1;
                if on_face {
                    let brick = if tier % 2 == 0 {
                        BlockId::BrickRed
                    } else {
                        BlockId::BrickGrey
                    };
                    return Some(brick);
                }
                if local.y == base_y + 3 {
                    return Some(BlockId::Cobble);
                }
            }
        }
    }

    if local.y >= 1 && local.y <= 14 {
        for arch_x in [-12, -6, 0, 6, 12] {
            if local.x == arch_x && local.z.abs() <= 1 {
                let arch_top = 8 - (arch_x.abs() / 3);
                if local.y <= arch_top {
                    return Some(BlockId::BrickGrey);
                }
            }
        }
    }

    if local.y == 17 && in_outer && !in_inner {
        return Some(BlockId::Glowstone);
    }

    None
}

fn clock_tower_voxel_at(origin: IVec3, pos: IVec3) -> Option<BlockId> {
    let local = pos - origin;

    if local.y == 0 && in_disk(local.x, local.z, 4) {
        return Some(BlockId::Cobble);
    }

    if local.y >= 1 && local.y <= 32 {
        if on_disk_edge(local.x, local.z, 3) || (local.x == 0 && local.z == 0) {
            let band = local.y % 6 == 0;
            return Some(if band {
                BlockId::BrickGrey
            } else {
                BlockId::BrickRed
            });
        }
    }

    if local.y >= 33 && local.y <= 38 {
        if on_disk_edge(local.x, local.z, 4) {
            return Some(BlockId::BrickGrey);
        }
        if local.y == 33 && local.x.abs() <= 3 && local.z.abs() <= 3 {
            return Some(BlockId::Planks);
        }
    }

    if local.y == 36 {
        for (cx, cz) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
            if local.x >= cx - 1
                && local.x <= cx + 1
                && local.z >= cz - 1
                && local.z <= cz + 1
            {
                let on_ring = local.x == cx - 1
                    || local.x == cx + 1
                    || local.z == cz - 1
                    || local.z == cz + 1;
                if on_ring {
                    return Some(BlockId::Wool);
                }
                if local.x == cx && local.z == cz {
                    return Some(BlockId::Glowstone);
                }
            }
        }
    }

    for spire_y in 39..=46 {
        if local.y == spire_y && local.x == 0 && local.z == 0 {
            return Some(if spire_y == 46 {
                BlockId::Glowstone
            } else {
                BlockId::BrickGrey
            });
        }
    }

    None
}

fn windmill_voxel_at(origin: IVec3, pos: IVec3) -> Option<BlockId> {
    let local = pos - origin;

    if local.y == 0 && in_disk(local.x, local.z, 5) {
        return Some(BlockId::Cobble);
    }

    if local.y >= 1 && local.y <= 12 {
        if on_disk_edge(local.x, local.z, 3) {
            return Some(BlockId::BrickRed);
        }
        if local.x == 0 && local.z == 0 {
            return Some(BlockId::Planks);
        }
    }

    if local.y == 13 && on_disk_edge(local.x, local.z, 3) {
        return Some(BlockId::BrickGrey);
    }

    if local.y == 14 && local.x.abs() <= 1 && local.z.abs() <= 1 {
        return Some(BlockId::Wood);
    }

    if local.y == 15 {
        for blade in 0..4 {
            let angle = blade as f32 * std::f32::consts::FRAC_PI_2;
            let dir_x = angle.cos().round() as i32;
            let dir_z = angle.sin().round() as i32;
            for dist in 1..=6 {
                if local.x == dir_x * dist && local.z == dir_z * dist {
                    return Some(if dist % 2 == 0 {
                        BlockId::Wool
                    } else {
                        BlockId::Planks
                    });
                }
            }
        }
        if local.x == 0 && local.z == 0 {
            return Some(BlockId::Trunk);
        }
    }

    None
}

fn castle_voxel_at(origin: IVec3, pos: IVec3) -> Option<BlockId> {
    let local = pos - origin;
    let wall_half = 12;

    if local.y == 0 && local.x.abs() <= wall_half && local.z.abs() <= wall_half {
        return Some(BlockId::Cobble);
    }

    if local.y >= 1 && local.y <= 8 {
        let on_wall = local.x.abs() == wall_half
            || local.z.abs() == wall_half
            || (local.x.abs() <= 1 && local.z == -wall_half);
        if on_wall && local.x.abs() <= wall_half && local.z.abs() <= wall_half {
            let crenel = local.y % 2 == 1 && local.y >= 6;
            if !crenel || local.x % 2 == 0 || local.z % 2 == 0 {
                return Some(BlockId::Stone);
            }
        }
    }

    let towers = [
        (-wall_half, -wall_half),
        (wall_half, -wall_half),
        (-wall_half, wall_half),
        (wall_half, wall_half),
    ];
    for (tx, tz) in towers {
        let dx = local.x - tx;
        let dz = local.z - tz;
        if dx.abs() <= 2 && dz.abs() <= 2 {
            if local.y >= 1 && local.y <= 16 {
                if dx.abs() == 2 || dz.abs() == 2 {
                    return Some(BlockId::BrickGrey);
                }
            }
            if local.y == 17 && (dx.abs() == 2 || dz.abs() == 2) {
                return Some(BlockId::BrickRed);
            }
            if local.y == 18 && dx == 0 && dz == 0 {
                return Some(BlockId::Glowstone);
            }
        }
    }

    if local.y >= 1 && local.y <= 10 && local.x.abs() <= 4 && local.z.abs() <= 6 {
        if let Some(block) = hollow_box_local(
            IVec3::new(local.x, local.y, local.z),
            1,
            4,
            6,
            10,
            BlockId::BrickRed,
            BlockId::Planks,
        ) {
            return Some(block);
        }
    }

    if local.y == 11 && local.x == 0 && local.z == 0 {
        return Some(BlockId::Glowstone);
    }

    None
}

fn empire_tower_voxel_at(origin: IVec3, pos: IVec3) -> Option<BlockId> {
    let local = pos - origin;
    let sections = [(5, 22), (4, 18), (3, 14), (2, 12), (1, 16)];

    let mut y_offset = 0;
    for (half, section_height) in sections {
        if let Some(block) =
            hollow_glass_box_local(local, y_offset, half, half, section_height)
        {
            return Some(block);
        }
        y_offset += section_height;
    }

    for spire_y in 0..4 {
        if local.x == 0 && local.z == 0 && local.y == y_offset + spire_y {
            return Some(BlockId::Glass);
        }
    }
    if local.x == 0 && local.z == 0 && local.y == y_offset + 4 {
        return Some(BlockId::Glowstone);
    }

    None
}

fn landmark_voxel_at(origins: LandmarkOrigins, pos: IVec3) -> Option<BlockId> {
    let pyramid_local = pos - origins.pyramid;
    if pyramid_local.y >= 0
        && pyramid_local.y <= 12
        && pyramid_local.x.abs() <= 10
        && pyramid_local.z.abs() <= 10
    {
        if let Some(block) = pyramid_voxel_at(origins.pyramid, 10, pos) {
            return Some(block);
        }
    }

    let tower_local = pos - origins.tower;
    if tower_local.y >= 0
        && tower_local.y <= 86
        && tower_local.x.abs() <= 5
        && tower_local.z.abs() <= 5
    {
        if let Some(block) = empire_tower_voxel_at(origins.tower, pos) {
            return Some(block);
        }
    }

    let lighthouse_local = pos - origins.lighthouse;
    if lighthouse_local.y >= 0
        && lighthouse_local.y <= 29
        && in_disk(lighthouse_local.x, lighthouse_local.z, 4)
    {
        if let Some(block) = lighthouse_voxel_at(origins.lighthouse, pos) {
            return Some(block);
        }
    }

    let bridge_local = pos - origins.bridge;
    if bridge_local.y >= -1
        && bridge_local.y <= 13
        && bridge_local.x.abs() <= 14
        && bridge_local.z.abs() <= 2
    {
        if let Some(block) = bridge_voxel_at(origins.bridge, pos) {
            return Some(block);
        }
    }

    let observatory_local = pos - origins.observatory;
    if observatory_local.y >= 0
        && observatory_local.y <= 22
        && observatory_local.x.abs() <= 9
        && observatory_local.z.abs() <= 9
    {
        if let Some(block) = observatory_voxel_at(origins.observatory, pos) {
            return Some(block);
        }
    }

    let city_local = pos - origins.city;
    if city_local.y >= 0
        && city_local.y <= 28
        && city_local.x.abs() <= 28
        && city_local.z.abs() <= 28
    {
        if let Some(block) = city_voxel_at(origins.city, pos) {
            return Some(block);
        }
    }

    let colosseum_local = pos - origins.colosseum;
    if colosseum_local.y >= 0
        && colosseum_local.y <= 18
        && colosseum_local.x.abs() <= 16
        && colosseum_local.z.abs() <= 12
    {
        if let Some(block) = colosseum_voxel_at(origins.colosseum, pos) {
            return Some(block);
        }
    }

    let clock_local = pos - origins.clock_tower;
    if clock_local.y >= 0
        && clock_local.y <= 48
        && in_disk(clock_local.x, clock_local.z, 5)
    {
        if let Some(block) = clock_tower_voxel_at(origins.clock_tower, pos) {
            return Some(block);
        }
    }

    let windmill_local = pos - origins.windmill;
    if windmill_local.y >= 0
        && windmill_local.y <= 22
        && windmill_local.x.abs() <= 7
        && windmill_local.z.abs() <= 7
    {
        if let Some(block) = windmill_voxel_at(origins.windmill, pos) {
            return Some(block);
        }
    }

    let castle_local = pos - origins.castle;
    if castle_local.y >= 0
        && castle_local.y <= 22
        && castle_local.x.abs() <= 14
        && castle_local.z.abs() <= 14
    {
        if let Some(block) = castle_voxel_at(origins.castle, pos) {
            return Some(block);
        }
    }

    None
}
