use bevy::input::gamepad::GamepadButton;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use bevy_replicon::prelude::{SendTargets, ServerTriggerExt, ToClients};
use bevy_voxel_world::prelude::NeedsDespawn;
use crate::audio::GameAudio;
use crate::game_settings::GameSettings;
use crate::interaction::PendingBlockEdits;
use crate::gamepad::select_primary;
use crate::net::replicate::{RemotePlayerBody, WorldRevertBroadcast};
use crate::net::NetworkRole;
use crate::player::{
    FlyActivation, GravityMode, Player, PlayerCamera, PlayerSettings, ShadowQuality, VsyncMode,
};
use crate::save::{revert_to_world_base, save_world, WorldEdits};
use crate::voxel_config::BridgetWorld;
use crate::world_gen::WorldMetadata;
use crate::AppState;

use super::hud::HudRoot;

#[derive(Resource, Default)]
pub struct GameMenuOpen(pub bool);

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum GameMenuPanel {
    #[default]
    Main,
    Settings,
}

#[derive(Resource, Default)]
pub struct GameMenuPanelState(pub GameMenuPanel);

#[derive(Component)]
pub struct GameMenuRoot;

#[derive(Component)]
pub(crate) struct GameMenuContent;

#[derive(Component)]
pub struct GameMenuButton(&'static str);

#[derive(Clone, Copy)]
enum GameMenuButtonVariant {
    Primary,
    Normal,
    Subtle,
    Danger,
}

#[derive(Component)]
pub(crate) struct GameMenuButtonStyle(GameMenuButtonVariant);

#[derive(Component)]
pub(crate) struct RevertConfirmRoot;

const MENU_OVERLAY: Color = Color::srgba(0.02, 0.05, 0.12, 0.72);
const MENU_PANEL: Color = Color::srgba(0.06, 0.1, 0.16, 0.94);
const MENU_PANEL_BORDER: Color = Color::srgba(0.28, 0.55, 0.92, 0.55);
const MENU_ACCENT: Color = Color::srgb(0.35, 0.72, 1.0);
const MENU_TITLE: Color = Color::srgb(0.92, 0.96, 1.0);
const MENU_HINT: Color = Color::srgb(0.45, 0.52, 0.62);

fn button_variant_color(variant: GameMenuButtonVariant, state: Interaction) -> Color {
    match (variant, state) {
        (GameMenuButtonVariant::Primary, Interaction::Pressed) => Color::srgb(0.12, 0.42, 0.72),
        (GameMenuButtonVariant::Primary, Interaction::Hovered) => Color::srgb(0.22, 0.58, 0.95),
        (GameMenuButtonVariant::Primary, Interaction::None) => Color::srgb(0.18, 0.48, 0.82),
        (GameMenuButtonVariant::Normal, Interaction::Pressed) => Color::srgb(0.12, 0.28, 0.48),
        (GameMenuButtonVariant::Normal, Interaction::Hovered) => Color::srgb(0.2, 0.42, 0.68),
        (GameMenuButtonVariant::Normal, Interaction::None) => Color::srgb(0.14, 0.32, 0.52),
        (GameMenuButtonVariant::Subtle, Interaction::Pressed) => Color::srgb(0.1, 0.14, 0.22),
        (GameMenuButtonVariant::Subtle, Interaction::Hovered) => Color::srgb(0.16, 0.22, 0.32),
        (GameMenuButtonVariant::Subtle, Interaction::None) => Color::srgb(0.1, 0.14, 0.2),
        (GameMenuButtonVariant::Danger, Interaction::Pressed) => Color::srgb(0.42, 0.14, 0.14),
        (GameMenuButtonVariant::Danger, Interaction::Hovered) => Color::srgb(0.62, 0.2, 0.2),
        (GameMenuButtonVariant::Danger, Interaction::None) => Color::srgb(0.34, 0.12, 0.12),
    }
}

pub fn menu_closed(open: Res<GameMenuOpen>) -> bool {
    !open.0
}

pub fn game_menu_settings_open(
    open: Res<GameMenuOpen>,
    panel: Res<GameMenuPanelState>,
) -> bool {
    open.0 && panel.0 == GameMenuPanel::Settings
}

pub fn toggle_game_menu(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(&Name, &Gamepad)>,
    mut open: ResMut<GameMenuOpen>,
    mut panel: ResMut<GameMenuPanelState>,
    mut cursor: Query<&mut CursorOptions>,
    mut commands: Commands,
    menu: Query<Entity, With<GameMenuRoot>>,
    confirm: Query<Entity, With<RevertConfirmRoot>>,
) {
    let menu_pressed = keys.just_pressed(KeyCode::Escape)
        || select_primary(gamepads.iter())
            .is_some_and(|gamepad| gamepad.just_pressed(GamepadButton::Start));
    if !menu_pressed {
        return;
    }

    if open.0 {
        if panel.0 == GameMenuPanel::Settings {
            panel.0 = GameMenuPanel::Main;
            return;
        }

        open.0 = false;
        panel.0 = GameMenuPanel::Main;
        set_cursor_grabbed(&mut cursor, true);
        despawn_game_menu(&mut commands, &menu, &confirm);
    } else {
        open.0 = true;
        panel.0 = GameMenuPanel::Main;
        set_cursor_grabbed(&mut cursor, false);
        if menu.is_empty() {
            spawn_game_menu(&mut commands);
        }
    }
}

pub fn sync_game_menu_content_visibility(
    panel: Res<GameMenuPanelState>,
    mut content: Query<&mut Visibility, With<GameMenuContent>>,
) {
    let visible = panel.0 == GameMenuPanel::Main;
    for mut visibility in &mut content {
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub fn game_menu_button_interaction(
    mut interaction_query: Query<
        (
            &Interaction,
            &GameMenuButton,
            &GameMenuButtonStyle,
            &mut BackgroundColor,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut open: ResMut<GameMenuOpen>,
    mut panel: ResMut<GameMenuPanelState>,
    mut next_state: ResMut<NextState<AppState>>,
    metadata: Res<WorldMetadata>,
    mut edits: ResMut<WorldEdits>,
    role: Res<NetworkRole>,
    mut pending: ResMut<PendingBlockEdits>,
    mut cursor: Query<&mut CursorOptions>,
    game_settings: Res<GameSettings>,
    mut audio: ResMut<GameAudio>,
    mut commands: Commands,
    menu: Query<Entity, With<GameMenuRoot>>,
    confirm: Query<Entity, With<RevertConfirmRoot>>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, button, style, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                audio.play_ui_click(&mut commands, &game_settings);
                *color = button_variant_color(style.0, Interaction::Pressed).into();
                match button.0 {
                    "keep_playing" => {
                        open.0 = false;
                        panel.0 = GameMenuPanel::Main;
                        set_cursor_grabbed(&mut cursor, true);
                        despawn_game_menu(&mut commands, &menu, &confirm);
                    }
                    "settings" => {
                        panel.0 = GameMenuPanel::Settings;
                    }
                    "revert_prompt" => {
                        if confirm.is_empty() {
                            spawn_revert_confirm(&mut commands);
                        }
                    }
                    "revert_confirm" => {
                        if role.is_client() {
                            warn!("only the host can restore the original map");
                        } else if let Err(err) = revert_to_world_base(
                            &metadata,
                            &mut edits,
                            &mut pending,
                            true,
                        ) {
                            warn!("failed to restore original map: {err}");
                        } else if role.is_host() {
                            commands.server_trigger(ToClients {
                                targets: SendTargets::CLIENTS_ONLY,
                                message: WorldRevertBroadcast,
                            });
                        }
                        despawn_revert_confirm(&mut commands, &confirm);
                    }
                    "revert_cancel" => {
                        despawn_revert_confirm(&mut commands, &confirm);
                    }
                    "main_menu" => {
                        if let Err(err) = save_world(&metadata, &edits) {
                            warn!("save before returning to menu failed: {err}");
                        }
                        open.0 = false;
                        panel.0 = GameMenuPanel::Main;
                        despawn_game_menu(&mut commands, &menu, &confirm);
                        next_state.set(AppState::MainMenu);
                    }
                    "quit" => {
                        if let Err(err) = save_world(&metadata, &edits) {
                            warn!("save before quitting failed: {err}");
                        }
                        open.0 = false;
                        panel.0 = GameMenuPanel::Main;
                        exit.write(AppExit::Success);
                    }
                    _ => {}
                }
            }
            Interaction::Hovered => {
                audio.play_ui_rollover(&mut commands, &game_settings);
                *color = button_variant_color(style.0, Interaction::Hovered).into();
            }
            Interaction::None => {
                *color = button_variant_color(style.0, Interaction::None).into();
            }
        }
    }
}

fn spawn_menu_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: &'static str,
    variant: GameMenuButtonVariant,
    width: f32,
    height: f32,
    font_size: f32,
) {
    parent
        .spawn((
            Button,
            GameMenuButton(action),
            GameMenuButtonStyle(variant),
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..Default::default()
            },
            BackgroundColor(button_variant_color(variant, Interaction::None)),
            BorderColor::all(Color::srgba(0.45, 0.7, 1.0, 0.18)),
        ))
        .with_child((
            Text::new(label),
            TextFont {
                font_size,
                ..Default::default()
            },
            TextColor(Color::WHITE),
        ));
}

pub fn spawn_game_menu(commands: &mut Commands) {
    commands
        .spawn((
            GameMenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            BackgroundColor(MENU_OVERLAY),
            ZIndex(100),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    GameMenuContent,
                    Node {
                        width: Val::Px(360.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(12.0),
                        padding: UiRect::axes(Val::Px(36.0), Val::Px(32.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..Default::default()
                    },
                    BackgroundColor(MENU_PANEL),
                    BorderColor::all(MENU_PANEL_BORDER),
                ))
                .with_children(|parent| {
                    parent
                        .spawn(Node {
                            flex_shrink: 0.0,
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..Default::default()
                        })
                        .with_child((
                            Text::new("Paused"),
                            TextFont {
                                font_size: 22.0,
                                ..Default::default()
                            },
                            TextColor(MENU_ACCENT),
                        ));
                    parent.spawn((
                        Text::new("Game Menu"),
                        TextFont {
                            font_size: 44.0,
                            ..Default::default()
                        },
                        TextColor(MENU_TITLE),
                    ));
                    parent.spawn((
                        Node {
                            width: Val::Px(120.0),
                            height: Val::Px(3.0),
                            margin: UiRect::vertical(Val::Px(4.0)),
                            ..Default::default()
                        },
                        BackgroundColor(MENU_ACCENT),
                    ));

                    spawn_menu_button(
                        parent,
                        "Keep Playing",
                        "keep_playing",
                        GameMenuButtonVariant::Primary,
                        300.0,
                        56.0,
                        26.0,
                    );

                    parent.spawn((
                        Node {
                            width: Val::Percent(88.0),
                            height: Val::Px(1.0),
                            margin: UiRect::vertical(Val::Px(6.0)),
                            ..Default::default()
                        },
                        BackgroundColor(Color::srgba(0.35, 0.55, 0.82, 0.28)),
                    ));

                    for (label, action) in [
                        ("Settings", "settings"),
                        ("Main Menu", "main_menu"),
                    ] {
                        spawn_menu_button(
                            parent,
                            label,
                            action,
                            GameMenuButtonVariant::Normal,
                            300.0,
                            48.0,
                            22.0,
                        );
                    }

                    spawn_menu_button(
                        parent,
                        "Quit Game",
                        "quit",
                        GameMenuButtonVariant::Danger,
                        300.0,
                        44.0,
                        20.0,
                    );

                    parent.spawn((
                        Text::new("Escape or Start to close"),
                        TextFont {
                            font_size: 13.0,
                            ..Default::default()
                        },
                        TextColor(MENU_HINT),
                        Node {
                            margin: UiRect::top(Val::Px(10.0)),
                            ..Default::default()
                        },
                    ));
                });
        });
}

fn spawn_revert_confirm(commands: &mut Commands) {
    commands
        .spawn((
            RevertConfirmRoot,
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
                        width: Val::Px(400.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(14.0),
                        padding: UiRect::all(Val::Px(28.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..Default::default()
                    },
                    BackgroundColor(Color::srgba(0.14, 0.08, 0.08, 0.97)),
                    BorderColor::all(Color::srgba(0.85, 0.3, 0.3, 0.55)),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("Restore Original Map?"),
                        TextFont {
                            font_size: 30.0,
                            ..Default::default()
                        },
                        TextColor(Color::srgb(1.0, 0.82, 0.82)),
                    ));
                    parent.spawn((
                        Text::new(
                            "This removes all your builds and restores the starting meadow,\ntrees, and glass landmarks. Saved changes cannot be undone.",
                        ),
                        TextFont {
                            font_size: 15.0,
                            ..Default::default()
                        },
                        TextColor(Color::srgb(0.82, 0.76, 0.76)),
                    ));

                    spawn_menu_button(
                        parent,
                        "Yes, Restore Map",
                        "revert_confirm",
                        GameMenuButtonVariant::Danger,
                        300.0,
                        48.0,
                        22.0,
                    );
                    spawn_menu_button(
                        parent,
                        "Cancel",
                        "revert_cancel",
                        GameMenuButtonVariant::Subtle,
                        300.0,
                        44.0,
                        20.0,
                    );
                });
        });
}

fn despawn_revert_confirm(
    commands: &mut Commands,
    confirm: &Query<Entity, With<RevertConfirmRoot>>,
) {
    for entity in confirm {
        commands.entity(entity).despawn();
    }
}

fn despawn_game_menu(
    commands: &mut Commands,
    menu: &Query<Entity, With<GameMenuRoot>>,
    confirm: &Query<Entity, With<RevertConfirmRoot>>,
) {
    despawn_revert_confirm(commands, confirm);
    for entity in menu {
        commands.entity(entity).despawn();
    }
}

#[derive(Component)]
pub struct WorldScene;

pub fn retire_world_chunks(
    mut commands: Commands,
    chunks: Query<Entity, (With<bevy_voxel_world::prelude::Chunk<BridgetWorld>>, Without<NeedsDespawn>)>,
) {
    for entity in &chunks {
        commands.entity(entity).insert(NeedsDespawn);
    }
}

pub fn cleanup_world(
    mut commands: Commands,
    hud: Query<Entity, With<HudRoot>>,
    menu: Query<Entity, With<GameMenuRoot>>,
    players: Query<Entity, With<Player>>,
    cameras: Query<Entity, With<PlayerCamera>>,
    chunk_anchors: Query<Entity, With<crate::player::ChunkSpawnAnchor>>,
    remote_players: Query<Entity, With<RemotePlayerBody>>,
    world_scene: Query<Entity, With<WorldScene>>,
    mut open: ResMut<GameMenuOpen>,
    mut panel: ResMut<GameMenuPanelState>,
    mut config: ResMut<BridgetWorld>,
) {
    open.0 = false;
    panel.0 = GameMenuPanel::Main;
    config.max_spawn_per_frame = BridgetWorld::default().max_spawn_per_frame;

    for entity in menu
        .iter()
        .chain(hud.iter())
        .chain(players.iter())
        .chain(cameras.iter())
        .chain(chunk_anchors.iter())
        .chain(remote_players.iter())
        .chain(world_scene.iter())
    {
        commands.entity(entity).despawn();
    }
}

fn set_cursor_grabbed(cursor: &mut Query<&mut CursorOptions>, grabbed: bool) {
    for mut options in cursor.iter_mut() {
        options.visible = !grabbed;
        options.grab_mode = if grabbed {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::None
        };
    }
}

fn apply_settings_egui_style(ctx: &bevy_egui::egui::Context) {
    let mut style = (*ctx.style()).clone();
    let visuals = &mut style.visuals;
    visuals.window_fill = bevy_egui::egui::Color32::from_rgba_premultiplied(10, 16, 26, 245);
    visuals.panel_fill = bevy_egui::egui::Color32::from_rgba_premultiplied(10, 16, 26, 245);
    visuals.window_stroke = bevy_egui::egui::Stroke::new(
        1.5,
        bevy_egui::egui::Color32::from_rgba_premultiplied(70, 140, 230, 140),
    );
    visuals.widgets.noninteractive.bg_fill =
        bevy_egui::egui::Color32::from_rgba_premultiplied(16, 24, 36, 220);
    visuals.widgets.inactive.bg_fill =
        bevy_egui::egui::Color32::from_rgba_premultiplied(24, 40, 62, 230);
    visuals.widgets.hovered.bg_fill =
        bevy_egui::egui::Color32::from_rgba_premultiplied(34, 58, 88, 240);
    visuals.widgets.active.bg_fill =
        bevy_egui::egui::Color32::from_rgba_premultiplied(46, 92, 148, 250);
    visuals.selection.bg_fill =
        bevy_egui::egui::Color32::from_rgba_premultiplied(56, 118, 188, 180);
    ctx.set_style(style);
}

pub fn settings_ui(
    mut contexts: bevy_egui::EguiContexts,
    mut settings: ResMut<PlayerSettings>,
    mut game_settings: ResMut<GameSettings>,
    mut panel: ResMut<GameMenuPanelState>,
    role: Res<NetworkRole>,
    mut commands: Commands,
    confirm: Query<Entity, With<RevertConfirmRoot>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    apply_settings_egui_style(ctx);

    bevy_egui::egui::Window::new("Settings")
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .anchor(bevy_egui::egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.heading("Performance");
            ui.label("Rendering and diagnostics");
            ui.add(
                bevy_egui::egui::Slider::new(&mut settings.render_distance, 3..=8)
                    .text("Render distance"),
            );
            ui.horizontal(|ui| {
                ui.label("Shadow quality:");
                for quality in ShadowQuality::ALL {
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
            ui.heading("Audio");
            ui.add(
                bevy_egui::egui::Slider::new(&mut game_settings.master_volume, 0.0..=1.0)
                    .text("Master volume"),
            );
            ui.add(
                bevy_egui::egui::Slider::new(&mut game_settings.sfx_volume, 0.0..=1.0)
                    .text("SFX volume"),
            );
            ui.add(
                bevy_egui::egui::Slider::new(&mut game_settings.ui_volume, 0.0..=1.0)
                    .text("UI volume"),
            );
            game_settings.master_volume = GameSettings::clamp_volume(game_settings.master_volume);
            game_settings.sfx_volume = GameSettings::clamp_volume(game_settings.sfx_volume);
            game_settings.ui_volume = GameSettings::clamp_volume(game_settings.ui_volume);

            ui.separator();
            ui.heading("Movement");
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

            ui.separator();
            ui.heading("Game");
            if role.is_client() {
                ui.label(format!(
                    "Game speed: {:.2}x (set by host)",
                    game_settings.speed
                ));
            } else {
                ui.add(
                    bevy_egui::egui::Slider::new(
                        &mut game_settings.speed,
                        GameSettings::MIN_SPEED..=GameSettings::MAX_SPEED,
                    )
                    .text("Game speed")
                    .suffix("x"),
                );
                game_settings.speed = GameSettings::clamp_speed(game_settings.speed);
            }

            if !role.is_client() {
                ui.separator();
                ui.heading("World");
                ui.label("Reset terrain to the generated starting map.");
                if ui
                    .add(
                        bevy_egui::egui::Button::new("Restore Original Map")
                            .fill(bevy_egui::egui::Color32::from_rgba_premultiplied(120, 36, 36, 230)),
                    )
                    .clicked()
                    && confirm.is_empty()
                {
                    spawn_revert_confirm(&mut commands);
                }
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("← Back to Game Menu").clicked() {
                    panel.0 = GameMenuPanel::Main;
                }
                ui.label("Escape to go back");
            });
        });
}
