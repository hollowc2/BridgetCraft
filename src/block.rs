use bevy::prelude::*;
use bevy_voxel_world::prelude::WorldVoxel;
use serde::{Deserialize, Serialize};

pub const HOTBAR_SIZE: usize = 9;

/// Texture atlas indices (128px tiles stacked vertically in `assets/textures/voxel_atlas.png`).
pub mod atlas {
    pub const GRASS_TOP: u32 = 0;
    pub const DIRT_GRASS_SIDE: u32 = 1;
    pub const DIRT: u32 = 2;
    pub const STONE: u32 = 3;
    pub const SAND: u32 = 4;
    pub const WOOD: u32 = 5;
    pub const BRICK_RED: u32 = 6;
    pub const BRICK_GREY: u32 = 7;
    pub const GLASS: u32 = 8;
    pub const GRAVEL: u32 = 9;
    pub const CLAY: u32 = 10;
    pub const SNOW: u32 = 11;
    pub const LEAVES: u32 = 12;
    pub const TRUNK_TOP: u32 = 13;
    pub const TRUNK_SIDE: u32 = 14;
    pub const TRUNK_WHITE_TOP: u32 = 15;
    pub const TRUNK_WHITE_SIDE: u32 = 16;
    pub const WATER: u32 = 17;
    pub const COBBLE: u32 = 18;
    pub const PLANKS: u32 = 19;
    pub const WOOL: u32 = 20;
    pub const GLOWSTONE: u32 = 21;
    pub const GRASS_DECOR: u32 = 22;
}

pub const ATLAS_TEXTURE_COUNT: u32 = 23;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlockId {
    DirtGrass = 0,
    Dirt = 1,
    Stone = 2,
    Sand = 3,
    Wood = 4,
    BrickRed = 5,
    BrickGrey = 6,
    Glass = 7,
    Gravel = 8,
    Clay = 9,
    Snow = 10,
    Leaves = 11,
    Trunk = 12,
    TrunkWhite = 13,
    Water = 14,
    Cobble = 15,
    Planks = 16,
    Wool = 17,
    Glowstone = 18,
    GrassDecor = 19,
}

impl BlockId {
    pub const ALL: [BlockId; 20] = [
        BlockId::DirtGrass,
        BlockId::Dirt,
        BlockId::Stone,
        BlockId::Sand,
        BlockId::Wood,
        BlockId::BrickRed,
        BlockId::BrickGrey,
        BlockId::Glass,
        BlockId::Gravel,
        BlockId::Clay,
        BlockId::Snow,
        BlockId::Leaves,
        BlockId::Trunk,
        BlockId::TrunkWhite,
        BlockId::Water,
        BlockId::Cobble,
        BlockId::Planks,
        BlockId::Wool,
        BlockId::Glowstone,
        BlockId::GrassDecor,
    ];

    pub const HOTBAR: [BlockId; HOTBAR_SIZE] = [
        BlockId::DirtGrass,
        BlockId::Dirt,
        BlockId::Stone,
        BlockId::Wood,
        BlockId::BrickRed,
        BlockId::Glass,
        BlockId::Sand,
        BlockId::Leaves,
        BlockId::Glowstone,
    ];

    pub fn as_material(self) -> u8 {
        self as u8
    }

    pub fn from_material(value: u8) -> Option<Self> {
        BlockId::ALL.iter().copied().find(|block| block.as_material() == value)
    }

    pub fn label(self) -> &'static str {
        match self {
            BlockId::DirtGrass => "Grass",
            BlockId::Dirt => "Dirt",
            BlockId::Stone => "Stone",
            BlockId::Sand => "Sand",
            BlockId::Wood => "Wood",
            BlockId::BrickRed => "Red Brick",
            BlockId::BrickGrey => "Grey Brick",
            BlockId::Glass => "Glass",
            BlockId::Gravel => "Gravel",
            BlockId::Clay => "Clay",
            BlockId::Snow => "Snow",
            BlockId::Leaves => "Leaves",
            BlockId::Trunk => "Trunk",
            BlockId::TrunkWhite => "Birch Trunk",
            BlockId::Water => "Water",
            BlockId::Cobble => "Cobble",
            BlockId::Planks => "Planks",
            BlockId::Wool => "Wool",
            BlockId::Glowstone => "Glowstone",
            BlockId::GrassDecor => "Grass",
        }
    }

    pub fn is_solid(self) -> bool {
        !matches!(self, BlockId::Water | BlockId::Glass | BlockId::Leaves)
    }

    pub fn is_breakable(self) -> bool {
        true
    }

    pub fn texture_indices(self) -> [u32; 3] {
        use atlas::*;
        match self {
            BlockId::DirtGrass => [GRASS_TOP, DIRT_GRASS_SIDE, DIRT],
            BlockId::Dirt => [DIRT, DIRT, DIRT],
            BlockId::Stone => [STONE, STONE, STONE],
            BlockId::Sand => [SAND, SAND, SAND],
            BlockId::Wood => [WOOD, WOOD, WOOD],
            BlockId::BrickRed => [BRICK_RED, BRICK_RED, BRICK_RED],
            BlockId::BrickGrey => [BRICK_GREY, BRICK_GREY, BRICK_GREY],
            BlockId::Glass => [GLASS, GLASS, GLASS],
            BlockId::Gravel => [GRAVEL, GRAVEL, GRAVEL],
            BlockId::Clay => [CLAY, CLAY, CLAY],
            BlockId::Snow => [SNOW, SNOW, SNOW],
            BlockId::Leaves => [LEAVES, LEAVES, LEAVES],
            BlockId::Trunk => [TRUNK_TOP, TRUNK_SIDE, TRUNK_TOP],
            BlockId::TrunkWhite => [TRUNK_WHITE_TOP, TRUNK_WHITE_SIDE, TRUNK_WHITE_TOP],
            BlockId::Water => [WATER, WATER, WATER],
            BlockId::Cobble => [COBBLE, COBBLE, COBBLE],
            BlockId::Planks => [PLANKS, PLANKS, PLANKS],
            BlockId::Wool => [WOOL, WOOL, WOOL],
            BlockId::Glowstone => [GLOWSTONE, GLOWSTONE, GLOWSTONE],
            BlockId::GrassDecor => [GRASS_DECOR, GRASS_DECOR, GRASS_TOP],
        }
    }

    pub fn to_world_voxel(self) -> WorldVoxel<u8> {
        WorldVoxel::Solid(self.as_material())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SavedVoxel {
    Air,
    Solid(u8),
}

impl SavedVoxel {
    pub fn from_world_voxel(voxel: WorldVoxel<u8>) -> Self {
        match voxel {
            WorldVoxel::Air => SavedVoxel::Air,
            WorldVoxel::Solid(material) => SavedVoxel::Solid(material),
            WorldVoxel::Unset => SavedVoxel::Air,
        }
    }

    pub fn to_world_voxel(self) -> WorldVoxel<u8> {
        match self {
            SavedVoxel::Air => WorldVoxel::Air,
            SavedVoxel::Solid(material) => WorldVoxel::Solid(material),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct HotbarSelection {
    pub index: usize,
}

impl Default for HotbarSelection {
    fn default() -> Self {
        Self { index: 0 }
    }
}

impl HotbarSelection {
    pub fn selected_block(&self) -> BlockId {
        BlockId::HOTBAR[self.index % HOTBAR_SIZE]
    }
}
