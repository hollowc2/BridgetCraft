mod audio;
mod bench;
mod block;
mod gamepad;
mod interaction;
mod net;
mod player;
mod save;
mod sky;
mod ui;
mod voxel_config;
mod world_gen;

use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
use bevy::prelude::*;
use bevy::window::PresentMode;
use bevy_egui::EguiPlugin;
use bevy_voxel_world::prelude::*;

use audio::GameAudioPlugin;
use bench::BenchPlugin;
use block::HotbarSelection;
use interaction::{
    apply_pending_to_world, flush_pending_block_edits, handle_block_interaction,
    update_block_target, BlockTarget, PendingBlockEdits,
};
use net::host::show_host_message;
use net::{NetworkPlugin, NetworkRole};
use player::{
    apply_render_settings, apply_render_settings_on_enter, find_spawn_position, grab_cursor,
    mouse_look, player_movement, release_cursor, spawn_player, sync_player_camera, FlyActivation,
    GravityMode, PlayerSettings,
    VsyncMode,
};
use save::{auto_save_system, load_world_edits, save_on_exit, SavePlugin, SaveTimer, WorldEdits};
use ui::hud::{hotbar_scroll, spawn_hud, update_hotbar_text, update_network_info};
use ui::menu::{
    cleanup_menu, menu_button_interaction, menu_input_display, menu_input_focus,
    menu_input_keyboard, menu_input_unfocus, menu_player_name, spawn_main_menu, MenuFocus,
    MenuSettings,
};
use ui::game_menu::{
    cleanup_world, game_menu_button_interaction, menu_closed, toggle_game_menu, GameMenuOpen,
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
    .add_plugins(EguiPlugin::default())
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
        sync_player_camera.run_if(in_state(AppState::InGame)),
    )
    .add_plugins(VoxelConfigPlugin)
    .add_plugins(NetworkPlugin)
    .init_state::<AppState>()
    .init_resource::<WorldMetadata>()
    .init_resource::<ProceduralTerrain>()
    .init_resource::<WorldEdits>()
    .init_resource::<PendingBlockEdits>()
    .init_resource::<HotbarSelection>()
    .init_resource::<NetworkRole>()
    .init_resource::<MenuSettings>()
    .init_resource::<MenuFocus>()
    .init_resource::<PlayerSettings>()
    .init_resource::<SaveTimer>()
    .init_resource::<DayNightCycle>()
    .init_resource::<GameMenuOpen>()
    .insert_resource(Time::<Fixed>::from_hz(60.0))
    .add_systems(Startup, setup_ui_camera)
    .add_systems(OnEnter(AppState::MainMenu), (release_cursor, spawn_main_menu))
    .add_systems(OnExit(AppState::MainMenu), cleanup_menu)
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
        OnEnter(AppState::InGame),
        (
            grab_cursor,
            sync_world_seed,
            setup_world,
            apply_render_settings_on_enter,
            show_host_message,
        )
            .chain(),
    )
    .add_systems(
        OnExit(AppState::InGame),
        (flush_pending_block_edits, cleanup_world, release_cursor).chain(),
    )
    .add_systems(Update, (toggle_performance_overlay, warn_if_voxel_atlas_failed))
    .add_systems(
        Update,
        (toggle_game_menu, game_menu_button_interaction).run_if(in_state(AppState::InGame)),
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
            handle_block_interaction,
            hotbar_scroll,
            update_hotbar_text,
            update_network_info,
            auto_save_system,
            save_on_exit,
            apply_shadow_settings,
            apply_render_settings,
            update_day_night,
            sync_diagnostics_overlay,
            settings_ui,
        )
            .run_if(in_state(AppState::InGame).and(menu_closed)),
    )
    .add_systems(
        FixedUpdate,
        player_movement.run_if(in_state(AppState::InGame).and(menu_closed)),
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
    commands.entity(player).insert((
        BlockTarget::default(),
        net::replicate::NetworkPlayer {
            name: player_name.clone(),
            selected_block: block::BlockId::DirtGrass.as_material(),
        },
        net::replicate::NetworkTransform::default(),
    ));

    if !role.is_client() {
        load_world_edits(&metadata, &mut edits, &mut pending);
        apply_pending_to_world(&mut pending, &mut voxel_world);
    }

    spawn_hud(&mut commands);

    let atlas_path = "textures/voxel_atlas.png";
    if !std::path::Path::new("assets").join(atlas_path).exists() {
        warn!(
            "missing assets/{atlas_path}; run `cargo build` to generate textures from Kenney tiles"
        );
    }
    commands.insert_resource(VoxelAtlasHandle(asset_server.load(atlas_path)));

    info!("world '{}' ready (seed {})", metadata.name, metadata.seed);
}

fn settings_ui(
    mut contexts: bevy_egui::EguiContexts,
    mut settings: ResMut<PlayerSettings>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    bevy_egui::egui::Window::new("Settings")
        .default_pos(bevy_egui::egui::pos2(12.0, 120.0))
        .show(ctx, |ui| {
            ui.label("Performance and controls");
            ui.add(
                bevy_egui::egui::Slider::new(&mut settings.render_distance, 3..=8)
                    .text("Render distance"),
            );
            ui.horizontal(|ui| {
                ui.label("Shadow quality:");
                for quality in crate::player::ShadowQuality::ALL {
                    ui.selectable_value(
                        &mut settings.shadow_quality,
                        quality,
                        quality.label(),
                    );
                }
            });
            ui.horizontal(|ui| {
                ui.label("MSAA:");
                // MSAA Off breaks bevy_voxel_world chunk rendering; offer 2x/4x only.
                for samples in [
                    bevy::render::view::Msaa::Sample2,
                    bevy::render::view::Msaa::Sample4,
                ] {
                    let label = match samples {
                        bevy::render::view::Msaa::Sample2 => "2x",
                        bevy::render::view::Msaa::Sample4 => "4x",
                        _ => "Other",
                    };
                    ui.selectable_value(&mut settings.msaa, samples, label);
                }
            });
            ui.horizontal(|ui| {
                ui.label("VSync:");
                for mode in VsyncMode::ALL {
                    ui.selectable_value(&mut settings.vsync_mode, mode, mode.label());
                }
            });
            ui.checkbox(
                &mut settings.show_diagnostics,
                "Show FPS overlay (F1)",
            );
            ui.add(
                bevy_egui::egui::Slider::new(&mut settings.mouse_sensitivity, 0.0005..=0.01)
                    .text("Mouse sensitivity"),
            );
            ui.add(
                bevy_egui::egui::Slider::new(&mut settings.gamepad_look_sensitivity, 0.5..=6.0)
                    .text("Gamepad look sensitivity"),
            );

            ui.separator();
            ui.label("Movement");
            ui.horizontal(|ui| {
                ui.label("Gravity:");
                for mode in GravityMode::ALL {
                    ui.selectable_value(&mut settings.gravity_mode, mode, mode.label());
                }
            });
            ui.horizontal(|ui| {
                ui.label("Fly mode:");
                for mode in FlyActivation::ALL {
                    ui.selectable_value(&mut settings.fly_activation, mode, mode.label());
                }
            });
            if settings.fly_activation == FlyActivation::DoubleTap {
                ui.label("Double-tap jump to fly; landing ends flight.");
            } else if settings.fly_activation == FlyActivation::Always {
                ui.label("Space rises, Shift descends.");
            }
        });
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
