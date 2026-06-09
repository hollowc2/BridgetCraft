mod block;
mod interaction;
mod net;
mod player;
mod save;
mod ui;
mod voxel_config;
mod world_gen;

use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use bevy_voxel_world::prelude::*;

use block::HotbarSelection;
use interaction::{handle_block_interaction, update_block_target, BlockTarget};
use net::host::show_host_message;
use net::{NetworkPlugin, NetworkRole};
use player::{
    find_spawn_position, grab_cursor, mouse_look, player_movement, release_cursor, spawn_player,
    PlayerSettings,
};
use save::{auto_save_system, load_world_edits, save_on_exit, SaveTimer, WorldEdits};
use ui::hud::{
    cleanup_hud, hotbar_scroll, spawn_hud, update_hotbar_text, update_network_info,
};
use ui::menu::{cleanup_menu, menu_button_interaction, spawn_main_menu};
use voxel_config::{sync_world_seed, BridgetWorld, VoxelConfigPlugin};
use world_gen::{decorate_trees, WorldMetadata};

#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
pub enum AppState {
    #[default]
    MainMenu,
    InGame,
}

#[derive(Resource)]
struct DayNightCycle {
    timer: Timer,
    phase: f32,
}

impl Default for DayNightCycle {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(120.0, TimerMode::Repeating),
            phase: 0.15,
        }
    }
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
        .add_plugins(VoxelConfigPlugin)
        .add_plugins(NetworkPlugin)
        .init_state::<AppState>()
        .init_resource::<WorldMetadata>()
        .init_resource::<WorldEdits>()
        .init_resource::<HotbarSelection>()
        .init_resource::<NetworkRole>()
        .init_resource::<PlayerSettings>()
        .init_resource::<SaveTimer>()
        .init_resource::<DayNightCycle>()
        .add_systems(Startup, setup_ui_camera)
        .add_systems(OnEnter(AppState::MainMenu), (release_cursor, spawn_main_menu))
        .add_systems(OnExit(AppState::MainMenu), cleanup_menu)
        .add_systems(Update, menu_button_interaction.run_if(in_state(AppState::MainMenu)))
        .add_systems(
            OnEnter(AppState::InGame),
            (
                grab_cursor,
                setup_world,
                show_host_message,
            )
                .chain(),
        )
        .add_systems(OnExit(AppState::InGame), (cleanup_hud, release_cursor))
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
                update_day_night,
                settings_ui,
            )
                .run_if(in_state(AppState::InGame)),
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
    metadata: Res<WorldMetadata>,
    mut config: ResMut<BridgetWorld>,
    mut edits: ResMut<WorldEdits>,
    role: Res<NetworkRole>,
    mut voxel_world: VoxelWorld<BridgetWorld>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
) {
    config.seed = metadata.seed;

    spawn_sun_and_ambient(&mut commands);
    spawn_sky(&mut commands, meshes, materials);

    let spawn = find_spawn_position(&voxel_world);
    let player_name = match &*role {
        NetworkRole::Host { .. } => "Host",
        NetworkRole::Client { .. } => "Guest",
        NetworkRole::None => "Builder",
    };

    let player = spawn_player(&mut commands, player_name, spawn);
    commands.entity(player).insert((
        BlockTarget::default(),
        net::replicate::NetworkPlayer {
            name: player_name.to_string(),
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

fn spawn_sun_and_ambient(commands: &mut Commands) {
    let cascade_shadow_config = CascadeShadowConfigBuilder {
        maximum_distance: 256.0,
        ..default()
    }
    .build();

    commands.spawn((
        DirectionalLight {
            illuminance: 18_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(1.0, 2.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
        cascade_shadow_config,
        Name::new("Sun"),
    ));

    commands.insert_resource(GlobalAmbientLight {
        brightness: 250.0,
        ..default()
    });
}

fn spawn_sky(
    commands: &mut Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.75, 0.95),
            unlit: true,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(800.0)),
        Name::new("Sky"),
    ));
}

fn update_day_night(
    time: Res<Time>,
    mut cycle: ResMut<DayNightCycle>,
    mut lights: Query<&mut DirectionalLight>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    cycle.timer.tick(time.delta());
    if cycle.timer.just_finished() {
        cycle.phase = (cycle.phase + 0.25) % 1.0;
    }

    let daylight = (cycle.phase * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    for mut light in &mut lights {
        light.illuminance = 4_000.0 + daylight * 20_000.0;
        light.color = Color::srgb(0.95, 0.9 + daylight * 0.05, 0.8 + daylight * 0.1);
    }
    ambient.brightness = 80.0 + daylight * 220.0;
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
        });
}
