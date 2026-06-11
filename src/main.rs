mod audio;
mod bench;
mod block;
mod game_settings;
mod gamepad;
mod interaction;
mod item;
mod net;
mod player;
mod save;
mod sky;
mod ui;
mod voxel_config;
mod world_gen;

use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FPS_OVERLAY_ZINDEX};
use bevy::prelude::*;
use bevy::window::PresentMode;
use bevy_egui::{EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext};
use bevy_voxel_world::prelude::*;

use audio::GameAudioPlugin;
use bench::BenchPlugin;
use item::{HotbarAssets, HotbarSelection};
use game_settings::{
    apply_game_speed, reset_game_settings, sync_game_settings_to_network,
    sync_network_game_settings, GameSettings,
};
use interaction::{
    apply_pending_to_world, draw_block_target_outline, flush_pending_block_edits,
    handle_block_interaction, update_block_break_progress, update_block_target,
    BlockBreakState, BlockTarget, PendingBlockEdits,
};
use net::host::show_host_message;
use net::{NetworkPlugin, NetworkRole};
use player::{
    apply_render_settings, apply_render_settings_on_enter, find_spawn_position, grab_cursor,
    lead_chunk_spawn_anchor, mouse_look, player_movement, release_cursor, spawn_chunk_anchor,
    spawn_player, sync_player_camera, PlayerSettings,
};
use save::{auto_save_system, load_world_edits, save_on_exit, SavePlugin, SaveTimer, WorldEdits};
use net::replicate::{ChatInput, ChatLog};
use ui::hud::{
    chat_input_system, hotbar_scroll, spawn_hud, update_chat_hud, update_hotbar_hud,
    update_network_info,
};
use ui::menu::{
    cleanup_menu, menu_button_interaction, menu_input_display, menu_input_focus,
    menu_input_keyboard, menu_input_unfocus, menu_player_name, spawn_main_menu, MenuFocus,
    MenuSettings,
};
use ui::game_menu::{
    cleanup_world, game_menu_button_interaction, game_menu_settings_open, menu_closed,
    retire_world_chunks, settings_ui, sync_game_menu_content_visibility, toggle_game_menu,
    GameMenuOpen, GameMenuPanelState,
};
use ui::loading::{
    begin_world_load, cleanup_loading_overlay, spawn_loading_overlay, update_loading_progress,
    WorldLoadState,
};
use ui::menu_splash::{
    cleanup_menu_splash, rotate_menu_splash, spawn_menu_splash,
};
use voxel_config::{sync_world_seed, BridgetWorld, VoxelConfigPlugin};
use sky::{
    apply_shadow_settings, follow_sky_to_player, spawn_sky, spawn_sun_and_ambient,
    update_celestial_bodies, update_day_night, DayNightCycle,
};
use world_gen::{ProceduralTerrain, WorldMetadata};

#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
pub enum AppState {
    #[default]
    MainMenu,
    InGame,
}

fn main() {
    // When the binary is launched directly (not via `cargo run`), Bevy otherwise resolves
    // assets next to the executable (`target/debug/assets/`) instead of the project folder.
    if std::env::var("BEVY_ASSET_ROOT").is_err() {
        let root = std::env::var("CARGO_MANIFEST_DIR")
            .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_string());
        std::env::set_var("BEVY_ASSET_ROOT", root);
    }

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BridgetCraft".into(),
                resolution: (1280, 720).into(),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }),
    )
    .insert_resource(EguiGlobalSettings {
        auto_create_primary_context: false,
        ..default()
    })
    .add_plugins(EguiPlugin {
        // Single-pass mode avoids duplicate EguiPrimaryContextPass panics when multiple
        // cameras exist (menu splash Camera3d + UI Camera2d).
        #[allow(deprecated)]
        enable_multipass_for_primary_context: false,
        ..default()
    })
    .add_plugins(FpsOverlayPlugin {
        config: FpsOverlayConfig {
            enabled: false,
            frame_time_graph_config: bevy::dev_tools::fps_overlay::FrameTimeGraphConfig {
                enabled: false,
                ..default()
            },
            ..default()
        },
    })
    .add_plugins(BenchPlugin)
    .add_plugins(GameAudioPlugin)
    .add_plugins(SavePlugin)
    .add_systems(
        PreUpdate,
        (
            sync_player_camera,
            lead_chunk_spawn_anchor,
        )
            .chain()
            .run_if(in_state(AppState::InGame)),
    )
    .add_plugins(VoxelConfigPlugin)
    .add_plugins(NetworkPlugin)
    .init_state::<AppState>()
    .init_resource::<WorldMetadata>()
    .init_resource::<ProceduralTerrain>()
    .init_resource::<WorldEdits>()
    .init_resource::<PendingBlockEdits>()
    .init_resource::<HotbarSelection>()
    .init_resource::<BlockBreakState>()
    .init_resource::<NetworkRole>()
    .init_resource::<MenuSettings>()
    .init_resource::<MenuFocus>()
    .init_resource::<PlayerSettings>()
    .init_resource::<GameSettings>()
    .init_resource::<SaveTimer>()
    .init_resource::<DayNightCycle>()
    .init_resource::<GameMenuOpen>()
    .init_resource::<GameMenuPanelState>()
    .init_resource::<WorldLoadState>()
    .init_resource::<ChatLog>()
    .init_resource::<ChatInput>()
    .insert_resource(Time::<Fixed>::from_hz(60.0))
    .add_systems(Startup, setup_ui_camera)
    .add_systems(
        OnEnter(AppState::MainMenu),
        (release_cursor, spawn_menu_splash, spawn_main_menu),
    )
    .add_systems(OnExit(AppState::MainMenu), (cleanup_menu, cleanup_menu_splash))
    .add_systems(
        Update,
        (
            menu_input_focus,
            menu_input_keyboard,
            menu_input_unfocus,
            menu_input_display,
            menu_button_interaction,
        )
            .chain()
            .run_if(in_state(AppState::MainMenu)),
    )
    .add_systems(
        Update,
        rotate_menu_splash.run_if(in_state(AppState::MainMenu)),
    )
    .add_systems(
        OnEnter(AppState::InGame),
        (
            grab_cursor,
            sync_world_seed,
            begin_world_load,
            setup_world,
            apply_render_settings_on_enter,
            show_host_message,
            spawn_loading_overlay,
        )
            .chain(),
    )
    .add_systems(
        OnExit(AppState::InGame),
        (
            flush_pending_block_edits,
            retire_world_chunks,
            cleanup_world,
            cleanup_loading_overlay,
            reset_game_settings,
            release_cursor,
        )
            .chain(),
    )
    .add_systems(Startup, position_fps_overlay)
    .add_systems(Update, (toggle_performance_overlay, warn_if_voxel_atlas_failed))
    .add_systems(
        Update,
        (
            toggle_game_menu,
            game_menu_button_interaction,
            sync_game_menu_content_visibility,
        )
            .run_if(in_state(AppState::InGame)),
    )
    .add_systems(
        Update,
        (
            sync_world_seed,
            mouse_look,
            sync_player_camera.after(mouse_look),
            follow_sky_to_player,
            update_celestial_bodies.after(follow_sky_to_player),
            update_block_target.after(sync_player_camera),
            draw_block_target_outline.after(update_block_target),
            update_block_break_progress.after(update_block_target),
            handle_block_interaction.after(update_block_break_progress),
        )
            .run_if(in_state(AppState::InGame).and(menu_closed).and(not_loading)),
    )
    .add_systems(
        Update,
        (
            update_hotbar_hud,
            hotbar_scroll,
            update_network_info,
            update_chat_hud,
            chat_input_system,
            auto_save_system,
            save_on_exit,
            apply_shadow_settings,
            apply_render_settings,
            update_day_night,
            sync_diagnostics_overlay,
            apply_game_speed,
            sync_game_settings_to_network.after(apply_game_speed),
            sync_network_game_settings.after(sync_game_settings_to_network),
        )
            .run_if(in_state(AppState::InGame).and(menu_closed).and(not_loading)),
    )
    .add_systems(
        EguiPrimaryContextPass,
        settings_ui.run_if(in_state(AppState::InGame).and(game_menu_settings_open).and(not_loading)),
    )
    .add_systems(
        Update,
        update_loading_progress.run_if(in_state(AppState::InGame)),
    )
    .add_systems(
        FixedUpdate,
        player_movement.run_if(in_state(AppState::InGame).and(menu_closed).and(not_loading)),
    )
    .add_systems(
        PostUpdate,
        flush_pending_block_edits.run_if(in_state(AppState::InGame)),
    );

    #[cfg(feature = "trace_tracy")]
    {
        app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default());
    }

    app.run();
}

#[derive(Resource)]
struct VoxelAtlasHandle(Handle<Image>);

#[derive(Component)]
struct UiCamera;

fn setup_ui_camera(mut commands: Commands) {
    commands.spawn((
        UiCamera,
        Camera2d,
        IsDefaultUiCamera,
        PrimaryEguiContext,
        Camera {
            order: 10,
            clear_color: ClearColorConfig::None,
            ..default()
        },
    ));
}

fn setup_world(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    metadata: Res<WorldMetadata>,
    terrain: Res<ProceduralTerrain>,
    menu_settings: Res<MenuSettings>,
    settings: Res<PlayerSettings>,
    mut edits: ResMut<WorldEdits>,
    mut pending: ResMut<PendingBlockEdits>,
    role: Res<NetworkRole>,
    mut voxel_world: VoxelWorld<BridgetWorld>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_sun_and_ambient(&mut commands, &settings);
    spawn_sky(&mut commands, &asset_server, meshes, materials);

    let spawn = find_spawn_position(&terrain);
    let player_name = menu_player_name(&menu_settings);

    let (player, _camera) = spawn_player(&mut commands, &player_name, spawn, &settings);
    spawn_chunk_anchor(&mut commands, spawn);
    commands.entity(player).insert((
        BlockTarget::default(),
        net::replicate::NetworkPlayer {
            name: player_name.clone(),
            selected_block: block::BlockId::DirtGrass.as_material(), // synced from hotbar
        },
        net::replicate::NetworkTransform::default(),
    ));

    if !role.is_client() {
        load_world_edits(&metadata, &mut edits, &mut pending);
        apply_pending_to_world(&mut pending, &mut voxel_world);
        commands.spawn((
            bevy_replicon::prelude::Replicated,
            net::replicate::NetworkGameSettings::default(),
            Name::new("GameSettings"),
        ));
    }

    let hotbar_layout = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(64),
        1,
        item::icon_atlas::ICON_COUNT,
        None,
        None,
    ));
    let hotbar_assets = HotbarAssets {
        image: asset_server.load("textures/hotbar_atlas.png"),
        layout: hotbar_layout,
    };
    spawn_hud(&mut commands, &hotbar_assets);
    commands.insert_resource(hotbar_assets);

    let atlas_path = "textures/voxel_atlas.png";
    if !std::path::Path::new("assets").join(atlas_path).exists() {
        warn!(
            "missing assets/{atlas_path}; run `cargo build` to generate textures from Kenney tiles"
        );
    }
    commands.insert_resource(VoxelAtlasHandle(asset_server.load(atlas_path)));

    info!("world '{}' ready (seed {})", metadata.name, metadata.seed);
}

fn warn_if_voxel_atlas_failed(
    asset_server: Res<AssetServer>,
    atlas: Option<Res<VoxelAtlasHandle>>,
    mut warned: Local<bool>,
) {
    if *warned {
        return;
    }
    let Some(atlas) = atlas else {
        return;
    };

    match asset_server.get_load_state(atlas.0.id()) {
        Some(bevy::asset::LoadState::Failed(err)) => {
            *warned = true;
            error!(
                "voxel atlas failed to load ({err}). Terrain will stay invisible until you run \
                 `cargo build` to regenerate assets/textures/voxel_atlas.png"
            );
        }
        Some(bevy::asset::LoadState::Loaded) => *warned = true,
        _ => {}
    }
}

fn position_fps_overlay(mut query: Query<(&GlobalZIndex, &mut Node)>) {
    for (z_index, mut node) in &mut query {
        if z_index.0 != FPS_OVERLAY_ZINDEX {
            continue;
        }
        node.top = Val::Px(12.0);
        node.right = Val::Px(12.0);
        node.left = Val::Auto;
        node.align_items = AlignItems::FlexEnd;
    }
}

fn toggle_performance_overlay(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<PlayerSettings>,
    mut overlay: ResMut<FpsOverlayConfig>,
) {
    if !keys.just_pressed(KeyCode::F1) {
        return;
    }

    settings.show_diagnostics = !settings.show_diagnostics;
    let enabled = settings.show_diagnostics;
    overlay.enabled = enabled;
    overlay.frame_time_graph_config.enabled = enabled;
}

fn not_loading(load_state: Res<WorldLoadState>) -> bool {
    !load_state.active
}

fn sync_diagnostics_overlay(
    settings: Res<PlayerSettings>,
    mut overlay: ResMut<FpsOverlayConfig>,
) {
    if !settings.is_changed() {
        return;
    }

    overlay.enabled = settings.show_diagnostics;
    overlay.frame_time_graph_config.enabled = settings.show_diagnostics;
}
