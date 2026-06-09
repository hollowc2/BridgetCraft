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

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use bevy_voxel_world::prelude::*;

use audio::GameAudioPlugin;
use block::HotbarSelection;
use interaction::{handle_block_interaction, update_block_target, BlockTarget};
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
use sky::{follow_sky_to_camera, spawn_sky, spawn_sun_and_ambient, update_day_night, DayNightCycle};
use world_gen::{decorate_trees, WorldMetadata};

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
        .add_plugins(GameAudioPlugin)
        .add_plugins(VoxelConfigPlugin)
        .add_plugins(NetworkPlugin)
        .init_state::<AppState>()
        .init_resource::<WorldMetadata>()
        .init_resource::<WorldEdits>()
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
        .add_systems(OnExit(AppState::InGame), (cleanup_world, release_cursor))
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
                follow_sky_to_camera,
                update_day_night,
                settings_ui,
            )
                .run_if(in_state(AppState::InGame).and(menu_closed)),
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
    menu_settings: Res<MenuSettings>,
    mut edits: ResMut<WorldEdits>,
    role: Res<NetworkRole>,
    mut voxel_world: VoxelWorld<BridgetWorld>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_sun_and_ambient(&mut commands);
    spawn_sky(&mut commands, asset_server, meshes, materials);

    let spawn = find_spawn_position(metadata.seed);
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
        for (pos, voxel) in decorate_trees(metadata.seed, IVec3::ZERO, 48) {
            voxel_world.set_voxel(pos, voxel);
        }
        load_world_edits(&metadata, &mut edits, &mut voxel_world);
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
