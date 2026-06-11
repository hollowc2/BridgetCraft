use bevy::prelude::*;
use bevy_voxel_world::prelude::*;

use crate::player::PlayerSettings;
use crate::voxel_config::{horizontal_spawn_column_count, BridgetWorld};

const INITIAL_LOAD_FADE_SECS: f32 = 0.35;
const LOAD_STALL_DISMISS_SECS: f32 = 2.5;
const LOAD_ABSOLUTE_TIMEOUT_SECS: f32 = 20.0;

#[derive(Resource)]
pub struct WorldLoadState {
    pub active: bool,
    pub target_chunks: u32,
    pub loaded_chunks: u32,
    pub last_meshed_count: u32,
    pub stalled_secs: f32,
    pub elapsed_secs: f32,
    pub fade: Timer,
}

impl Default for WorldLoadState {
    fn default() -> Self {
        Self {
            active: false,
            target_chunks: 0,
            loaded_chunks: 0,
            last_meshed_count: 0,
            stalled_secs: 0.0,
            elapsed_secs: 0.0,
            fade: Timer::from_seconds(INITIAL_LOAD_FADE_SECS, TimerMode::Once),
        }
    }
}

#[derive(Component)]
pub struct LoadingOverlay;

#[derive(Component)]
pub struct LoadingBarFill;

#[derive(Component)]
pub struct LoadingStatusText;

pub fn begin_world_load(
    mut load_state: ResMut<WorldLoadState>,
    settings: Res<PlayerSettings>,
) {
    // Surface columns near the player are a better progress estimate than the full 3D
    // spawn sphere; ray-based spawning often leaves a few edge columns without meshes.
    let target = horizontal_spawn_column_count(settings.render_distance);
    *load_state = WorldLoadState {
        active: true,
        target_chunks: target,
        loaded_chunks: 0,
        last_meshed_count: 0,
        stalled_secs: 0.0,
        elapsed_secs: 0.0,
        fade: Timer::from_seconds(INITIAL_LOAD_FADE_SECS, TimerMode::Once),
    };
}

pub fn spawn_loading_overlay(mut commands: Commands) {
    commands
        .spawn((
            LoadingOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.82)),
            ZIndex(200),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Px(420.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(14.0),
                        ..Default::default()
                    },
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("Generating world…"),
                        TextFont {
                            font_size: 28.0,
                            ..Default::default()
                        },
                        TextColor(Color::srgb(0.9, 0.95, 1.0)),
                    ));

                    panel
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(18.0),
                                border: UiRect::all(Val::Px(2.0)),
                                padding: UiRect::all(Val::Px(2.0)),
                                ..Default::default()
                            },
                            BackgroundColor(Color::srgba(0.1, 0.14, 0.2, 0.9)),
                        ))
                        .with_child((
                            LoadingBarFill,
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..Default::default()
                            },
                            BackgroundColor(Color::srgb(0.28, 0.62, 0.95)),
                        ));

                    panel.spawn((
                        LoadingStatusText,
                        Text::new("0% · 0 / 0 chunks"),
                        TextFont {
                            font_size: 16.0,
                            ..Default::default()
                        },
                        TextColor(Color::srgb(0.65, 0.72, 0.82)),
                    ));
                });
        });
}

pub fn update_loading_progress(
    time: Res<Time>,
    mut commands: Commands,
    mut load_state: ResMut<WorldLoadState>,
    chunks: Query<(), (With<Chunk<BridgetWorld>>, With<Mesh3d>)>,
    mut fill: Query<&mut Node, With<LoadingBarFill>>,
    mut status: Query<&mut Text, With<LoadingStatusText>>,
    mut overlay: Query<&mut BackgroundColor, With<LoadingOverlay>>,
    overlay_entities: Query<Entity, With<LoadingOverlay>>,
) {
    if !load_state.active {
        return;
    }

    load_state.elapsed_secs += time.delta_secs();
    load_state.loaded_chunks = chunks.iter().count() as u32;
    if load_state.loaded_chunks != load_state.last_meshed_count {
        load_state.last_meshed_count = load_state.loaded_chunks;
        load_state.stalled_secs = 0.0;
    } else {
        load_state.stalled_secs += time.delta_secs();
    }

    if load_state.stalled_secs >= LOAD_STALL_DISMISS_SECS && load_state.loaded_chunks > 0 {
        load_state.target_chunks = load_state.loaded_chunks;
    }

    let progress = if load_state.target_chunks == 0 {
        1.0
    } else {
        (load_state.loaded_chunks as f32 / load_state.target_chunks as f32).clamp(0.0, 1.0)
    };

    for mut node in &mut fill {
        node.width = Val::Percent(progress * 100.0);
    }

    for mut text in &mut status {
        text.0 = format!(
            "{}% · {} / {} chunks",
            (progress * 100.0).round() as u32,
            load_state.loaded_chunks,
            load_state.target_chunks,
        );
    }

    let ready = progress >= 0.92
        || load_state.loaded_chunks >= load_state.target_chunks
        || (load_state.stalled_secs >= LOAD_STALL_DISMISS_SECS && load_state.loaded_chunks > 0)
        || load_state.elapsed_secs >= LOAD_ABSOLUTE_TIMEOUT_SECS;
    if ready {
        load_state.fade.tick(time.delta());
        let alpha = 0.82 * (1.0 - load_state.fade.fraction());
        for mut background in &mut overlay {
            *background = Color::srgba(0.02, 0.04, 0.08, alpha).into();
        }
        if load_state.fade.is_finished() {
            load_state.active = false;
            for entity in &overlay_entities {
                commands.entity(entity).despawn();
            }
        }
    }
}

pub fn cleanup_loading_overlay(
    mut commands: Commands,
    mut load_state: ResMut<WorldLoadState>,
    overlay: Query<Entity, With<LoadingOverlay>>,
) {
    *load_state = WorldLoadState::default();
    for entity in &overlay {
        commands.entity(entity).despawn();
    }
}
