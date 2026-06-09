mod audio;
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
use bevy_egui::EguiPlugin;
use bevy_voxel_world::prelude::*;

use audio::GameAudioPlugin;
use block::HotbarSelection;
use interaction::{
    apply_pending_to_world, flush_pending_block_edits, handle_block_interaction,
    update_block_target, BlockTarget, PendingBlockEdits,
};
use net::host::show_host_message;
use net::{NetworkPlugin, NetworkRole};
use player::{
    find_spawn_position, grab_cursor, mouse_look, player_movement, release_cursor, spawn_player,
    FlyActivation, GravityMode, PlayerSettings,
};
use save::{auto_save_system, load_world_edits, save_on_exit, SaveTimer, WorldEdits};
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
    apply_shadow_settings, configure_sky_cubemap, follow_sky_to_camera, spawn_sky,
    spawn_sun_and_ambient, update_day_night, DayNightCycle,
};
use world_gen::{ProceduralTerrain, WorldMetadata};

#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
pub enum AppState {
    #[default]
    MainMenu,
    InGame,
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "BridgetCraft".into(),
                    resolution: (1280, 720).into(),
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
        .add_plugins(GameAudioPlugin)
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
                show_host_message,
            )
                .chain(),
        )
        .add_systems(
            OnExit(AppState::InGame),
            (flush_pending_block_edits, cleanup_world, release_cursor).chain(),
        )
        .add_systems(
            Update,
            toggle_performance_overlay,
        )
        .add_systems(
            Update,
            (toggle_game_menu, game_menu_button_interaction).run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (
                sync_world_seed,
                mouse_look,
                player_movement,
                update_block_target,
                handle_block_interaction,
                hotbar_scroll,
                update_hotbar_text,
                update_network_info,
                auto_save_system,
                save_on_exit,
                configure_sky_cubemap,
                follow_sky_to_camera,
                apply_shadow_settings,
                update_day_night,
                sync_diagnostics_overlay,
                settings_ui,
            )
                .run_if(in_state(AppState::InGame).and(menu_closed)),
        )
        .add_systems(
            PostUpdate,
            flush_pending_block_edits.run_if(in_state(AppState::InGame)),
        )
        .run();
}

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
    spawn_sky(&mut commands, asset_server, meshes, materials);

    let spawn = find_spawn_position(&terrain);
    let player_name = menu_player_name(&menu_settings);

    let player = spawn_player(&mut commands, &player_name, spawn);
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
