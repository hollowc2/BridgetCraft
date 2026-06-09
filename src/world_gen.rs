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

#[derive(Clone, Copy)]
struct LandmarkOrigins {
    pyramid: IVec3,
    tower: IVec3,
    lighthouse: IVec3,
    bridge: IVec3,
    observatory: IVec3,
}

fn landmark_origins(height_noise: &HybridMulti<Perlin>) -> LandmarkOrigins {
    let surface = |x: i32, z: i32| terrain_surface_height_with(height_noise, x, z);
    LandmarkOrigins {
        pyramid: IVec3::new(34, surface(34, -22) + 1, -22),
        tower: IVec3::new(-32, surface(-32, 26) + 1, 26),
        lighthouse: IVec3::new(24, surface(24, 30) + 1, 30),
        bridge: IVec3::new(0, surface(0, -34) + 1, -34),
        observatory: IVec3::new(-22, surface(-22, -22) + 1, -22),
    }
}

const TREE_RADIUS: i32 = 48;
const TREE_LEAF_CLEARANCE: i32 = 6;

/// Cached procedural terrain data. Rebuilt only when the world seed changes.
#[derive(Resource)]
pub struct ProceduralTerrain {
    pub seed: u32,
    height_noise: Arc<HybridMulti<Perlin>>,
    height_cache: Arc<Mutex<HashMap<(i32, i32), i32>>>,
    lookup: Arc<dyn Fn(IVec3) -> WorldVoxel<u8> + Send + Sync>,
}

fn build_tree_columns(
    height_noise: &HybridMulti<Perlin>,
    tree_noise: &Perlin,
) -> HashMap<(i32, i32), i32> {
    let mut columns = HashMap::new();
    for x in -TREE_RADIUS..=TREE_RADIUS {
        for z in -TREE_RADIUS..=TREE_RADIUS {
            if x * x + z * z > TREE_RADIUS * TREE_RADIUS {
                continue;
            }

            let density = tree_noise.get([x as f64 * 0.17, z as f64 * 0.17]);
            if density < 0.62 {
                continue;
            }

            columns.insert((x, z), terrain_surface_height_with(height_noise, x, z));
        }
    }
    columns
}

fn near_landmark(pos: IVec3, origins: LandmarkOrigins) -> bool {
    let checks = [
        (origins.pyramid, 12, 10),
        (origins.tower, 86, 5),
        (origins.lighthouse, 29, 4),
        (origins.bridge, 13, 14),
        (origins.observatory, 22, 9),
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

fn in_tree_region(pos: IVec3) -> bool {
    pos.x * pos.x + pos.z * pos.z <= (TREE_RADIUS + 2) * (TREE_RADIUS + 2)
}

impl Default for ProceduralTerrain {
    fn default() -> Self {
        Self::new(42_424)
    }
}

impl ProceduralTerrain {
    pub fn new(seed: u32) -> Self {
        let height_noise = Arc::new(build_height_noise(seed));
        let height_cache = Arc::new(Mutex::new(HashMap::new()));
        let tree_noise = Perlin::new(seed.wrapping_add(77_007));
        let tree_columns = build_tree_columns(&height_noise, &tree_noise);
        let landmarks = landmark_origins(&height_noise);
        let noise = height_noise.clone();
        let cache = height_cache.clone();
        let voxel_cache = Arc::new(Mutex::new(HashMap::<IVec3, WorldVoxel<u8>>::new()));
        let memo = voxel_cache.clone();
        let lookup = Arc::new(move |pos: IVec3| {
            if let Some(voxel) = memo.lock().unwrap().get(&pos) {
                return *voxel;
            }

            let height = {
                let mut cache = cache.lock().unwrap();
                match cache.get(&(pos.x, pos.z)) {
                    Some(height) => *height,
                    None => {
                        let height = terrain_surface_height_with(&noise, pos.x, pos.z);
                        cache.insert((pos.x, pos.z), height);
                        height
                    }
                }
            };

            let voxel = if pos.y > height + TREE_LEAF_CLEARANCE {
                WorldVoxel::Air
            } else if pos.y < 0 {
                WorldVoxel::Solid(BlockId::Stone.as_material())
            } else if pos.y <= height + TREE_LEAF_CLEARANCE
                && (in_tree_region(pos) || near_landmark(pos, landmarks))
            {
                decoration_voxel_at(landmarks, &tree_columns, pos, height)
                    .unwrap_or_else(|| terrain_voxel_at(pos, height))
            } else {
                terrain_voxel_at(pos, height)
            };

            memo.lock().unwrap().insert(pos, voxel);
            voxel
        });

        Self {
            seed,
            height_noise,
            height_cache,
            lookup,
        }
    }

    pub fn surface_height(&self, x: i32, z: i32) -> i32 {
        let mut cache = self.height_cache.lock().unwrap();
        *cache.entry((x, z)).or_insert_with(|| {
            terrain_surface_height_with(&self.height_noise, x, z)
        })
    }

    pub fn voxel_at(&self, pos: IVec3) -> WorldVoxel<u8> {
        (self.lookup)(pos)
    }

    pub fn lookup(&self) -> Arc<dyn Fn(IVec3) -> WorldVoxel<u8> + Send + Sync> {
        self.lookup.clone()
    }
}

pub fn terrain_lookup(lookup: Arc<dyn Fn(IVec3) -> WorldVoxel<u8> + Send + Sync>) -> VoxelLookupDelegate<u8> {
    Box::new(move |_chunk_pos, _lod, _previous| {
        let lookup = lookup.clone();
        Box::new(move |pos: IVec3, _previous| lookup(pos))
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
    tree_columns: &HashMap<(i32, i32), i32>,
    pos: IVec3,
    surface_height: i32,
) -> Option<WorldVoxel<u8>> {
    if near_landmark(pos, landmarks) {
        if let Some(block) = landmark_voxel_at(landmarks, pos) {
            return Some(block.to_world_voxel());
        }
    }

    if pos.y <= surface_height + TREE_LEAF_CLEARANCE {
        tree_voxel_at(tree_columns, pos).map(|block| block.to_world_voxel())
    } else {
        None
    }
}

fn tree_voxel_at(tree_columns: &HashMap<(i32, i32), i32>, pos: IVec3) -> Option<BlockId> {
    if !in_tree_region(pos) {
        return None;
    }

    if let Some(&surface) = tree_columns.get(&(pos.x, pos.z)) {
        let trunk_y = pos.y - surface;
        if (1..=4).contains(&trunk_y) {
            return Some(BlockId::Trunk);
        }
    }

    for x in (pos.x - 2)..=(pos.x + 2) {
        for z in (pos.z - 2)..=(pos.z + 2) {
            let Some(&surface) = tree_columns.get(&(x, z)) else {
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

    None
}
