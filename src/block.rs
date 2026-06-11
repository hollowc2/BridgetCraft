use bevy::prelude::Color;
use bevy_voxel_world::prelude::WorldVoxel;
use serde::{Deserialize, Serialize};

use crate::item::BlockBreakCategory;

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

    pub fn as_material(self) -> u8 {
        self as u8
    }

    pub fn from_material(value: u8) -> Option<Self> {
        match value {
            0 => Some(BlockId::DirtGrass),
            1 => Some(BlockId::Dirt),
            2 => Some(BlockId::Stone),
            3 => Some(BlockId::Sand),
            4 => Some(BlockId::Wood),
            5 => Some(BlockId::BrickRed),
            6 => Some(BlockId::BrickGrey),
            7 => Some(BlockId::Glass),
            8 => Some(BlockId::Gravel),
            9 => Some(BlockId::Clay),
            10 => Some(BlockId::Snow),
            11 => Some(BlockId::Leaves),
            12 => Some(BlockId::Trunk),
            13 => Some(BlockId::TrunkWhite),
            14 => Some(BlockId::Water),
            15 => Some(BlockId::Cobble),
            16 => Some(BlockId::Planks),
            17 => Some(BlockId::Wool),
            18 => Some(BlockId::Glowstone),
            19 => Some(BlockId::GrassDecor),
            _ => None,
        }
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
        !matches!(self, BlockId::Water)
    }

    pub fn break_category(self) -> BlockBreakCategory {
        match self {
            BlockId::DirtGrass
            | BlockId::Dirt
            | BlockId::Sand
            | BlockId::Gravel
            | BlockId::Clay
            | BlockId::Snow
            | BlockId::GrassDecor => BlockBreakCategory::Soil,
            BlockId::Stone
            | BlockId::Cobble
            | BlockId::BrickRed
            | BlockId::BrickGrey
            | BlockId::Glowstone => BlockBreakCategory::Stone,
            BlockId::Wood
            | BlockId::Planks
            | BlockId::Trunk
            | BlockId::TrunkWhite
            | BlockId::Leaves => BlockBreakCategory::Wood,
            BlockId::Glass | BlockId::Water | BlockId::Wool => BlockBreakCategory::Soft,
        }
    }

    pub fn base_break_seconds(self) -> f32 {
        match self.break_category() {
            BlockBreakCategory::Soft => 0.2,
            BlockBreakCategory::Soil => 0.45,
            BlockBreakCategory::Wood => 0.55,
            BlockBreakCategory::Stone => 1.4,
        }
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

    /// Approximate tint for held-block previews on player avatars.
    pub fn preview_color(self) -> Color {
        match self {
            BlockId::DirtGrass => Color::srgb(0.34, 0.62, 0.24),
            BlockId::Dirt => Color::srgb(0.55, 0.38, 0.22),
            BlockId::Stone => Color::srgb(0.55, 0.55, 0.58),
            BlockId::Sand => Color::srgb(0.86, 0.78, 0.52),
            BlockId::Wood => Color::srgb(0.58, 0.4, 0.22),
            BlockId::BrickRed => Color::srgb(0.72, 0.32, 0.24),
            BlockId::BrickGrey => Color::srgb(0.55, 0.55, 0.55),
            BlockId::Glass => Color::srgba(0.72, 0.88, 0.95, 0.65),
            BlockId::Gravel => Color::srgb(0.62, 0.6, 0.58),
            BlockId::Clay => Color::srgb(0.62, 0.48, 0.42),
            BlockId::Snow => Color::srgb(0.92, 0.94, 0.98),
            BlockId::Leaves => Color::srgb(0.28, 0.58, 0.22),
            BlockId::Trunk => Color::srgb(0.42, 0.28, 0.16),
            BlockId::TrunkWhite => Color::srgb(0.82, 0.78, 0.72),
            BlockId::Water => Color::srgba(0.22, 0.45, 0.82, 0.75),
            BlockId::Cobble => Color::srgb(0.48, 0.48, 0.5),
            BlockId::Planks => Color::srgb(0.72, 0.55, 0.32),
            BlockId::Wool => Color::srgb(0.92, 0.92, 0.92),
            BlockId::Glowstone => Color::srgb(0.95, 0.88, 0.45),
            BlockId::GrassDecor => Color::srgb(0.4, 0.68, 0.28),
        }
    }
}

/// Precomputed texture atlas indices per material id for O(1) meshing lookups.
pub fn texture_index_table() -> &'static [[u32; 3]; 256] {
    static TABLE: std::sync::OnceLock<[[u32; 3]; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [[0u32; 3]; 256];
        for block in BlockId::ALL {
            table[block.as_material() as usize] = block.texture_indices();
        }
        table
    })
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

