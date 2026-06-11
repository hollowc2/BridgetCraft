use bevy::prelude::*;

use crate::block::BlockId;

pub const HOTBAR_SIZE: usize = 9;

/// Hotbar icon atlas: indices 0–19 are block materials; 20–22 are tools.
pub mod icon_atlas {
    pub const TOOL_PICK: u32 = 20;
    pub const TOOL_SHOVEL: u32 = 21;
    pub const TOOL_AXE: u32 = 22;
    pub const ICON_COUNT: u32 = 23;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolId {
    Shovel,
    Pick,
    Axe,
}

impl ToolId {
    pub fn break_category(self) -> BlockBreakCategory {
        match self {
            ToolId::Shovel => BlockBreakCategory::Soil,
            ToolId::Pick => BlockBreakCategory::Stone,
            ToolId::Axe => BlockBreakCategory::Wood,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ToolId::Shovel => "Shovel",
            ToolId::Pick => "Pick",
            ToolId::Axe => "Axe",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockBreakCategory {
    Soil,
    Stone,
    Wood,
    Soft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotbarSlot {
    Block(BlockId),
    Tool(ToolId),
}

impl HotbarSlot {
    pub fn label(self) -> &'static str {
        match self {
            HotbarSlot::Block(block) => block.label(),
            HotbarSlot::Tool(tool) => tool.label(),
        }
    }

    pub fn icon_index(self) -> u32 {
        match self {
            HotbarSlot::Block(block) => block.as_material() as u32,
            HotbarSlot::Tool(ToolId::Pick) => icon_atlas::TOOL_PICK,
            HotbarSlot::Tool(ToolId::Shovel) => icon_atlas::TOOL_SHOVEL,
            HotbarSlot::Tool(ToolId::Axe) => icon_atlas::TOOL_AXE,
        }
    }

    pub fn placement_block(self) -> Option<BlockId> {
        match self {
            HotbarSlot::Block(block) => Some(block),
            HotbarSlot::Tool(_) => None,
        }
    }

    /// Wrong tool still breaks blocks, just far slower than hand or the right tool.
    pub fn break_multiplier(self, category: BlockBreakCategory) -> f32 {
        const RIGHT_TOOL: f32 = 45.0;
        const HAND: f32 = 1.0;
        const WRONG_TOOL: f32 = 0.04;

        match self {
            HotbarSlot::Tool(tool) if tool.break_category() == category => RIGHT_TOOL,
            HotbarSlot::Tool(_) => WRONG_TOOL,
            HotbarSlot::Block(_) => HAND,
        }
    }
}

pub const HOTBAR: [HotbarSlot; HOTBAR_SIZE] = [
    HotbarSlot::Block(BlockId::DirtGrass),
    HotbarSlot::Tool(ToolId::Shovel),
    HotbarSlot::Block(BlockId::Stone),
    HotbarSlot::Tool(ToolId::Pick),
    HotbarSlot::Block(BlockId::Wood),
    HotbarSlot::Tool(ToolId::Axe),
    HotbarSlot::Block(BlockId::Sand),
    HotbarSlot::Block(BlockId::Leaves),
    HotbarSlot::Block(BlockId::Glowstone),
];

pub fn block_break_seconds(block: BlockId, slot: HotbarSlot) -> f32 {
    let base = block.base_break_seconds();
    let multiplier = slot.break_multiplier(block.break_category());
    base / multiplier
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
    pub fn selected(&self) -> HotbarSlot {
        HOTBAR[self.index % HOTBAR_SIZE]
    }

    pub fn selected_block(&self) -> Option<BlockId> {
        self.selected().placement_block()
    }
}

#[derive(Resource)]
pub struct HotbarAssets {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}
