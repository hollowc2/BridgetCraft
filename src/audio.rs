use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, SpatialListener, Volume};
use bevy::prelude::*;

use crate::block::BlockId;
use crate::world_gen::ProceduralTerrain;
use crate::voxel_config::BridgetWorld;
use bevy_voxel_world::prelude::*;

const UI_CLICK: &str = "kenney_ui-audio/Audio/click1.ogg";
const UI_ROLLOVER: &str = "kenney_ui-audio/Audio/rollover2.ogg";

const IMPACT_DIR: &str = "kenney_impact-sounds/Audio";

#[derive(Resource)]
pub struct GameAudio {
    ui_click: Handle<AudioSource>,
    ui_rollover: Handle<AudioSource>,
    footstep_grass: Vec<Handle<AudioSource>>,
    footstep_concrete: Vec<Handle<AudioSource>>,
    footstep_wood: Vec<Handle<AudioSource>>,
    footstep_snow: Vec<Handle<AudioSource>>,
    footstep_carpet: Vec<Handle<AudioSource>>,
    break_mining: Vec<Handle<AudioSource>>,
    break_wood: Vec<Handle<AudioSource>>,
    break_glass: Vec<Handle<AudioSource>>,
    break_soft: Vec<Handle<AudioSource>>,
    break_metal: Vec<Handle<AudioSource>>,
    place_generic: Vec<Handle<AudioSource>>,
    place_wood: Vec<Handle<AudioSource>>,
    place_soft: Vec<Handle<AudioSource>>,
    place_glass: Vec<Handle<AudioSource>>,
    place_metal: Vec<Handle<AudioSource>>,
    variant_cursor: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FootstepSurface {
    Grass,
    Concrete,
    Wood,
    Snow,
    Carpet,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockImpact {
    Mining,
    Wood,
    Glass,
    Soft,
    Metal,
}

pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_game_audio);
    }
}

fn load_game_audio(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(GameAudio {
        ui_click: asset_server.load(UI_CLICK),
        ui_rollover: asset_server.load(UI_ROLLOVER),
        footstep_grass: load_variants(&asset_server, "footstep_grass", 5),
        footstep_concrete: load_variants(&asset_server, "footstep_concrete", 5),
        footstep_wood: load_variants(&asset_server, "footstep_wood", 5),
        footstep_snow: load_variants(&asset_server, "footstep_snow", 5),
        footstep_carpet: load_variants(&asset_server, "footstep_carpet", 5),
        break_mining: load_variants(&asset_server, "impactMining", 5),
        break_wood: load_variants(&asset_server, "impactWood_medium", 5),
        break_glass: load_variants(&asset_server, "impactGlass_medium", 5),
        break_soft: load_variants(&asset_server, "impactSoft_medium", 5),
        break_metal: load_variants(&asset_server, "impactMetal_medium", 5),
        place_generic: load_variants(&asset_server, "impactGeneric_light", 5),
        place_wood: load_variants(&asset_server, "impactWood_light", 5),
        place_soft: load_variants(&asset_server, "impactSoft_medium", 5),
        place_glass: load_variants(&asset_server, "impactGlass_light", 5),
        place_metal: load_variants(&asset_server, "impactMetal_light", 5),
        variant_cursor: 0,
    });
}

fn load_variants(asset_server: &AssetServer, prefix: &str, count: usize) -> Vec<Handle<AudioSource>> {
    (0..count)
        .map(|index| asset_server.load(format!("{IMPACT_DIR}/{prefix}_{index:03}.ogg")))
        .collect()
}

impl GameAudio {
    pub fn play_ui_click(&mut self, commands: &mut Commands) {
        play_oneshot(commands, &self.ui_click, 0.55);
    }

    pub fn play_ui_rollover(&mut self, commands: &mut Commands) {
        play_oneshot(commands, &self.ui_rollover, 0.35);
    }

    pub fn play_footstep_at_feet(
        &mut self,
        commands: &mut Commands,
        voxel_world: &VoxelWorld<BridgetWorld>,
        terrain: &ProceduralTerrain,
        feet_position: Vec3,
    ) {
        let surface = footstep_surface_at_feet(voxel_world, terrain, feet_position);
        self.play_footstep(commands, surface);
    }

    fn play_footstep(&mut self, commands: &mut Commands, surface: FootstepSurface) {
        let len = match surface {
            FootstepSurface::Grass => self.footstep_grass.len(),
            FootstepSurface::Concrete => self.footstep_concrete.len(),
            FootstepSurface::Wood => self.footstep_wood.len(),
            FootstepSurface::Snow => self.footstep_snow.len(),
            FootstepSurface::Carpet => self.footstep_carpet.len(),
        };
        let index = self.next_index(len);
        let handle = match surface {
            FootstepSurface::Grass => self.footstep_grass.get(index),
            FootstepSurface::Concrete => self.footstep_concrete.get(index),
            FootstepSurface::Wood => self.footstep_wood.get(index),
            FootstepSurface::Snow => self.footstep_snow.get(index),
            FootstepSurface::Carpet => self.footstep_carpet.get(index),
        };
        if let Some(handle) = handle {
            play_oneshot(commands, handle, 0.42);
        }
    }

    pub fn play_block_break(&mut self, commands: &mut Commands, block: BlockId, position: IVec3) {
        let impact = block_impact(block);
        let len = match impact {
            BlockImpact::Mining => self.break_mining.len(),
            BlockImpact::Wood => self.break_wood.len(),
            BlockImpact::Glass => self.break_glass.len(),
            BlockImpact::Soft => self.break_soft.len(),
            BlockImpact::Metal => self.break_metal.len(),
        };
        let index = self.next_index(len);
        let handle = match impact {
            BlockImpact::Mining => self.break_mining.get(index),
            BlockImpact::Wood => self.break_wood.get(index),
            BlockImpact::Glass => self.break_glass.get(index),
            BlockImpact::Soft => self.break_soft.get(index),
            BlockImpact::Metal => self.break_metal.get(index),
        };
        if let Some(handle) = handle {
            play_spatial_oneshot(commands, handle, 0.72, position);
        }
    }

    pub fn play_block_place(&mut self, commands: &mut Commands, block: BlockId, position: IVec3) {
        let impact = block_impact(block);
        let len = match impact {
            BlockImpact::Mining => self.place_generic.len(),
            BlockImpact::Wood => self.place_wood.len(),
            BlockImpact::Glass => self.place_glass.len(),
            BlockImpact::Soft => self.place_soft.len(),
            BlockImpact::Metal => self.place_metal.len(),
        };
        let index = self.next_index(len);
        let handle = match impact {
            BlockImpact::Mining => self.place_generic.get(index),
            BlockImpact::Wood => self.place_wood.get(index),
            BlockImpact::Glass => self.place_glass.get(index),
            BlockImpact::Soft => self.place_soft.get(index),
            BlockImpact::Metal => self.place_metal.get(index),
        };
        if let Some(handle) = handle {
            play_spatial_oneshot(commands, handle, 0.58, position);
        }
    }

    fn next_index(&mut self, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        let index = self.variant_cursor as usize % len;
        self.variant_cursor = self.variant_cursor.wrapping_add(1);
        index
    }
}

pub fn voxel_block_at(
    voxel_world: &VoxelWorld<BridgetWorld>,
    terrain: &ProceduralTerrain,
    pos: IVec3,
) -> Option<BlockId> {
    let chunk_get_voxel = voxel_world.get_voxel_fn();
    let voxel = chunk_get_voxel(pos);
    let voxel = if voxel == WorldVoxel::Unset {
        terrain.voxel_at(pos)
    } else {
        voxel
    };

    match voxel {
        WorldVoxel::Solid(material) => BlockId::from_material(material),
        _ => None,
    }
}

fn footstep_surface_at_feet(
    voxel_world: &VoxelWorld<BridgetWorld>,
    terrain: &ProceduralTerrain,
    feet_position: Vec3,
) -> FootstepSurface {
    let below = (feet_position + Vec3::new(0.0, -0.1, 0.0)).floor().as_ivec3();
    voxel_block_at(voxel_world, terrain, below)
        .map(footstep_surface_for_block)
        .unwrap_or(FootstepSurface::Grass)
}

fn footstep_surface_for_block(block: BlockId) -> FootstepSurface {
    match block {
        BlockId::DirtGrass | BlockId::Dirt | BlockId::GrassDecor | BlockId::Leaves => {
            FootstepSurface::Grass
        }
        BlockId::Stone
        | BlockId::Cobble
        | BlockId::Gravel
        | BlockId::BrickRed
        | BlockId::BrickGrey
        | BlockId::Glowstone => FootstepSurface::Concrete,
        BlockId::Wood | BlockId::Planks | BlockId::Trunk | BlockId::TrunkWhite => {
            FootstepSurface::Wood
        }
        BlockId::Snow => FootstepSurface::Snow,
        BlockId::Sand | BlockId::Clay | BlockId::Wool | BlockId::Water | BlockId::Glass => {
            FootstepSurface::Carpet
        }
    }
}

fn block_impact(block: BlockId) -> BlockImpact {
    match block {
        BlockId::Stone
        | BlockId::Cobble
        | BlockId::Gravel
        | BlockId::BrickRed
        | BlockId::BrickGrey => BlockImpact::Mining,
        BlockId::Wood | BlockId::Planks | BlockId::Trunk | BlockId::TrunkWhite => BlockImpact::Wood,
        BlockId::Glass => BlockImpact::Glass,
        BlockId::Glowstone => BlockImpact::Metal,
        _ => BlockImpact::Soft,
    }
}

fn play_oneshot(commands: &mut Commands, source: &Handle<AudioSource>, volume: f32) {
    commands.spawn((
        AudioPlayer::new(source.clone()),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
    ));
}

fn play_spatial_oneshot(
    commands: &mut Commands,
    source: &Handle<AudioSource>,
    volume: f32,
    position: IVec3,
) {
    commands.spawn((
        AudioPlayer::new(source.clone()),
        PlaybackSettings::DESPAWN
            .with_volume(Volume::Linear(volume))
            .with_spatial(true),
        Transform::from_translation(position.as_vec3() + Vec3::splat(0.5)),
    ));
}

pub fn spatial_audio_listener() -> SpatialListener {
    SpatialListener::new(0.35)
}
