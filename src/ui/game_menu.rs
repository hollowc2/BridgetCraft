use bevy::input::gamepad::GamepadButton;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use bevy_replicon::prelude::{SendTargets, ServerTriggerExt, ToClients};
use bevy_voxel_world::prelude::VoxelWorld;

use crate::audio::GameAudio;
use crate::gamepad::select_primary;
use crate::net::replicate::{RemotePlayerBody, WorldRevertBroadcast};
use crate::net::NetworkRole;
use crate::player::Player;
use crate::save::{revert_to_world_base, save_world, WorldEdits};
use crate::voxel_config::BridgetWorld;
use crate::world_gen::WorldMetadata;
use crate::AppState;

use super::hud::HudRoot;

#[derive(Resource, Default)]
pub struct GameMenuOpen(pub bool);

#[derive(Component)]
pub struct GameMenuRoot;

#[derive(Component)]
pub struct GameMenuButton(&'static str);

#[derive(Component)]
pub(crate) struct RevertConfirmRoot;

pub fn menu_closed(open: Res<GameMenuOpen>) -> bool {
    !open.0
}

pub fn toggle_game_menu(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(&Name, &Gamepad)>,
    role: Res<NetworkRole>,
    mut open: ResMut<GameMenuOpen>,
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

    open.0 = !open.0;

    if open.0 {
        set_cursor_grabbed(&mut cursor, false);
        if menu.is_empty() {
            spawn_game_menu(&mut commands, &role);
        }
    } else {
        set_cursor_grabbed(&mut cursor, true);
        despawn_game_menu(&mut commands, &menu, &confirm);
    }
}

pub fn game_menu_button_interaction(
    mut interaction_query: Query<
        (&Interaction, &GameMenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut open: ResMut<GameMenuOpen>,
    mut next_state: ResMut<NextState<AppState>>,
    metadata: Res<WorldMetadata>,
    mut edits: ResMut<WorldEdits>,
    role: Res<NetworkRole>,
    mut voxel_world: VoxelWorld<BridgetWorld>,
    mut cursor: Query<&mut CursorOptions>,
    mut audio: ResMut<GameAudio>,
    mut commands: Commands,
    menu: Query<Entity, With<GameMenuRoot>>,
    confirm: Query<Entity, With<RevertConfirmRoot>>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, button, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                audio.play_ui_click(&mut commands);
                *color = Color::srgb(0.15, 0.35, 0.6).into();
                match button.0 {
                    "keep_playing" => {
                        open.0 = false;
                        set_cursor_grabbed(&mut cursor, true);
                        despawn_game_menu(&mut commands, &menu, &confirm);
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
                            &mut voxel_world,
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
                        despawn_game_menu(&mut commands, &menu, &confirm);
                        next_state.set(AppState::MainMenu);
                    }
                    "quit" => {
                        if let Err(err) = save_world(&metadata, &edits) {
                            warn!("save before quitting failed: {err}");
                        }
                        open.0 = false;
                        exit.write(AppExit::Success);
                    }
                    _ => {}
                }
            }
            Interaction::Hovered => {
                audio.play_ui_rollover(&mut commands);
                *color = Color::srgb(0.25, 0.55, 0.85).into();
            }
            Interaction::None => {
                *color = Color::srgb(0.2, 0.45, 0.75).into();
            }
        }
    }
}

pub fn spawn_game_menu(commands: &mut Commands, role: &NetworkRole) {
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
            ZIndex(100),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(16.0),
                        padding: UiRect::all(Val::Px(32.0)),
                        ..Default::default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.12, 0.18, 0.95)),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("Game Menu"),
                        TextFont {
                            font_size: 42.0,
                            ..Default::default()
                        },
                        TextColor(Color::srgb(0.9, 0.95, 1.0)),
                    ));

                    let mut buttons = vec![
                        ("Keep Playing", "keep_playing"),
                        ("Main Menu", "main_menu"),
                        ("Quit Game", "quit"),
                    ];
                    if !role.is_client() {
                        buttons.insert(
                            1,
                            ("Restore Original Map", "revert_prompt"),
                        );
                    }

                    for (label, action) in buttons {
                        let is_destructive = action == "revert_prompt";
                        parent
                            .spawn((
                                Button,
                                GameMenuButton(action),
                                Node {
                                    width: Val::Px(280.0),
                                    height: Val::Px(52.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..Default::default()
                                },
                                BackgroundColor(if is_destructive {
                                    Color::srgb(0.55, 0.22, 0.22)
                                } else {
                                    Color::srgb(0.2, 0.45, 0.75)
                                }),
                            ))
                            .with_child((
                                Text::new(label),
                                TextFont {
                                    font_size: 24.0,
                                    ..Default::default()
                                },
                                TextColor(Color::WHITE),
                            ));
                    }

                    parent.spawn((
                        Text::new("Escape or Start to close"),
                        TextFont {
                            font_size: 14.0,
                            ..Default::default()
                        },
                        TextColor(Color::srgb(0.55, 0.6, 0.68)),
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            ZIndex(200),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(16.0),
                        padding: UiRect::all(Val::Px(28.0)),
                        ..Default::default()
                    },
                    BackgroundColor(Color::srgba(0.12, 0.08, 0.08, 0.98)),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("Restore Original Map?"),
                        TextFont {
                            font_size: 32.0,
                            ..Default::default()
                        },
                        TextColor(Color::srgb(1.0, 0.85, 0.85)),
                    ));
                    parent.spawn((
                        Text::new(
                            "This removes all your builds and restores the starting meadow,\ntrees, and glass landmarks. Saved changes cannot be undone.",
                        ),
                        TextFont {
                            font_size: 16.0,
                            ..Default::default()
                        },
                        TextColor(Color::srgb(0.85, 0.8, 0.8)),
                    ));

                    for (label, action, color) in [
                        ("Yes, Restore Map", "revert_confirm", Color::srgb(0.7, 0.2, 0.2)),
                        ("Cancel", "revert_cancel", Color::srgb(0.25, 0.35, 0.45)),
                    ] {
                        parent
                            .spawn((
                                Button,
                                GameMenuButton(action),
                                Node {
                                    width: Val::Px(280.0),
                                    height: Val::Px(48.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..Default::default()
                                },
                                BackgroundColor(color),
                            ))
                            .with_child((
                                Text::new(label),
                                TextFont {
                                    font_size: 22.0,
                                    ..Default::default()
                                },
                                TextColor(Color::WHITE),
                            ));
                    }
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

pub fn cleanup_world(
    mut commands: Commands,
    hud: Query<Entity, With<HudRoot>>,
    menu: Query<Entity, With<GameMenuRoot>>,
    players: Query<Entity, With<Player>>,
    remote_players: Query<Entity, With<RemotePlayerBody>>,
    world_scene: Query<Entity, With<WorldScene>>,
    chunks: Query<Entity, With<bevy_voxel_world::prelude::Chunk<BridgetWorld>>>,
    retired_chunks: Query<Entity, With<bevy_voxel_world::prelude::NeedsDespawn>>,
    mut open: ResMut<GameMenuOpen>,
) {
    open.0 = false;

    for entity in menu
        .iter()
        .chain(hud.iter())
        .chain(players.iter())
        .chain(remote_players.iter())
        .chain(world_scene.iter())
        .chain(chunks.iter())
        .chain(retired_chunks.iter())
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
