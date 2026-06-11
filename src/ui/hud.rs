use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::gamepad::select_primary;
use crate::interaction::BlockBreakState;
use crate::item::{HotbarAssets, HotbarSelection, HOTBAR, HOTBAR_SIZE};
use crate::net::replicate::{
    ChatBroadcast, ChatInput, ChatLog, ClientChatMessage, NetworkPlayer, CHAT_MAX_MESSAGE_LEN,
};
use crate::net::NetworkRole;
use crate::player::{Player, PlayerSettings};
use crate::ui::game_menu::GameMenuOpen;
use bevy_replicon::prelude::{ClientTriggerExt, SendTargets, ServerTriggerExt, ToClients};

const SLOT_SIZE: f32 = 52.0;
const SLOT_GAP: f32 = 4.0;

#[derive(Component)]
pub struct HudRoot;

#[derive(Component)]
pub struct Crosshair;

#[derive(Component)]
pub struct HotbarRoot;

#[derive(Component)]
pub struct HotbarSlotIcon {
    pub index: usize,
}

#[derive(Component)]
pub struct HotbarSelectionBorder;

#[derive(Component)]
pub struct HotbarSlotLabel;

#[derive(Component)]
pub struct BreakProgressTrack;

#[derive(Component)]
pub struct BreakProgressFill;

#[derive(Component)]
pub struct NetworkInfoText;

#[derive(Component)]
pub struct ChatLogText;

#[derive(Component)]
pub struct ChatInputPrompt;

pub fn spawn_hud(commands: &mut Commands, hotbar_assets: &HotbarAssets) {
    commands
        .spawn((
            HudRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..Default::default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Crosshair,
                Node {
                    width: Val::Px(12.0),
                    height: Val::Px(12.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..Default::default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
            ));

            parent
                .spawn((
                    BreakProgressTrack,
                    Node {
                        width: Val::Px(80.0),
                        height: Val::Px(5.0),
                        margin: UiRect::top(Val::Px(14.0)),
                        ..Default::default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                    Visibility::Hidden,
                ))
                .with_children(|bar| {
                    bar.spawn((
                        BreakProgressFill,
                        Node {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            ..Default::default()
                        },
                        BackgroundColor(Color::srgb(0.95, 0.85, 0.2)),
                    ));
                });
        });

    let hotbar_width = HOTBAR_SIZE as f32 * SLOT_SIZE + (HOTBAR_SIZE - 1) as f32 * SLOT_GAP;

    commands
        .spawn((
            HotbarRoot,
            Node {
                width: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                bottom: Val::Px(24.0),
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                ..Default::default()
            },
            HudRoot,
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    width: Val::Px(hotbar_width),
                    height: Val::Px(SLOT_SIZE),
                    column_gap: Val::Px(SLOT_GAP),
                    flex_direction: FlexDirection::Row,
                    ..Default::default()
                })
                .with_children(|row| {
                    for (index, slot) in HOTBAR.iter().enumerate() {
                        row.spawn((
                            HotbarSlotIcon { index },
                            Node {
                                width: Val::Px(SLOT_SIZE),
                                height: Val::Px(SLOT_SIZE),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(2.0)),
                                ..Default::default()
                            },
                            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                            BorderColor::all(Color::srgba(0.25, 0.25, 0.25, 0.9)),
                        ))
                        .with_children(|slot_root| {
                            slot_root.spawn((
                                ImageNode::from_atlas_image(
                                    hotbar_assets.image.clone(),
                                    TextureAtlas {
                                        index: slot.icon_index() as usize,
                                        layout: hotbar_assets.layout.clone(),
                                    },
                                ),
                                Node {
                                    width: Val::Px(SLOT_SIZE - 8.0),
                                    height: Val::Px(SLOT_SIZE - 8.0),
                                    ..Default::default()
                                },
                            ));

                            if index == 0 {
                                slot_root.spawn((
                                    HotbarSelectionBorder,
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0),
                                        position_type: PositionType::Absolute,
                                        border: UiRect::all(Val::Px(2.0)),
                                        ..Default::default()
                                    },
                                    BorderColor::all(Color::srgb(1.0, 1.0, 1.0)),
                                    Visibility::Visible,
                                ));
                            } else {
                                slot_root.spawn((
                                    HotbarSelectionBorder,
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0),
                                        position_type: PositionType::Absolute,
                                        border: UiRect::all(Val::Px(2.0)),
                                        ..Default::default()
                                    },
                                    BorderColor::all(Color::srgb(1.0, 1.0, 1.0)),
                                    Visibility::Hidden,
                                ));
                            }
                        });
                    }
                });

            parent.spawn((
                HotbarSlotLabel,
                Text::new(HOTBAR[0].label()),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.96, 1.0)),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                    ..Default::default()
                },
            ));
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                right: Val::Px(12.0),
                align_items: AlignItems::FlexEnd,
                ..Default::default()
            },
            HudRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                NetworkInfoText,
                Text::new(""),
                TextFont {
                    font_size: 16.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
                Node {
                    padding: UiRect::all(Val::Px(6.0)),
                    ..Default::default()
                },
            ));
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                bottom: Val::Px(96.0),
                max_width: Val::Px(420.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..Default::default()
            },
            HudRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                ChatLogText,
                Text::new(""),
                TextFont {
                    font_size: 14.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
                Node {
                    padding: UiRect::all(Val::Px(6.0)),
                    ..Default::default()
                },
            ));
            parent.spawn((
                ChatInputPrompt,
                Text::new(""),
                TextFont {
                    font_size: 14.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.75, 0.85, 1.0)),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                Node {
                    padding: UiRect::all(Val::Px(6.0)),
                    ..Default::default()
                },
                Visibility::Hidden,
            ));
        });
}

pub fn update_hotbar_hud(
    selection: Res<HotbarSelection>,
    break_state: Res<BlockBreakState>,
    slots: Query<(&HotbarSlotIcon, &Children)>,
    mut borders: Query<
        &mut Visibility,
        (With<HotbarSelectionBorder>, Without<BreakProgressTrack>),
    >,
    mut track: Query<
        (&mut Visibility, &Children),
        (With<BreakProgressTrack>, Without<HotbarSelectionBorder>),
    >,
    mut fills: Query<&mut Node, With<BreakProgressFill>>,
    mut label: Single<&mut Text, With<HotbarSlotLabel>>,
) {
    if selection.is_changed() {
        label.0 = HOTBAR[selection.index].label().to_string();

        for (slot, children) in &slots {
            let visible = slot.index == selection.index;
            for child in children.iter() {
                if let Ok(mut visibility) = borders.get_mut(child) {
                    *visibility = if visible {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
            }
        }
    }

    let show = break_state.progress > 0.0 && break_state.progress < 1.0;
    for (mut visibility, children) in &mut track {
        *visibility = if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        for child in children.iter() {
            if let Ok(mut node) = fills.get_mut(child) {
                node.width = Val::Percent((break_state.progress.clamp(0.0, 1.0) * 100.0) as f32);
            }
        }
    }
}

pub fn hotbar_scroll(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut selection: ResMut<HotbarSelection>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(&Name, &Gamepad)>,
    chat_input: Res<ChatInput>,
) {
    if chat_input.active {
        return;
    }

    for event in wheel.read() {
        if event.y > 0.0 {
            selection.index = (selection.index + HOTBAR_SIZE - 1) % HOTBAR_SIZE;
        } else if event.y < 0.0 {
            selection.index = (selection.index + 1) % HOTBAR_SIZE;
        }
    }

    let number_keys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (index, key) in number_keys.iter().enumerate() {
        if keys.just_pressed(*key) {
            selection.index = index;
        }
    }

    if let Some(gamepad) = select_primary(gamepads.iter()) {
        if gamepad.just_pressed(GamepadButton::DPadLeft) {
            selection.index = (selection.index + HOTBAR_SIZE - 1) % HOTBAR_SIZE;
        }
        if gamepad.just_pressed(GamepadButton::DPadRight) {
            selection.index = (selection.index + 1) % HOTBAR_SIZE;
        }
    }
}

pub fn update_network_info(
    role: Res<NetworkRole>,
    settings: Res<PlayerSettings>,
    players: Query<&NetworkPlayer>,
    mut text: Single<&mut Text, With<NetworkInfoText>>,
    mut last_snapshot: Local<String>,
) {
    let mut lines = vec![
        format!("Mode: {}", role.label()),
        format!("Render distance: {}", settings.render_distance),
        format!("Mouse sensitivity: {:.4}", settings.mouse_sensitivity),
    ];
    if let Some(addr) = role.display_address() {
        lines.push(format!("Address: {addr}"));
    }
    if let Some(err) = role.last_error() {
        lines.push(format!("Error: {err}"));
    }

    let mut names: Vec<_> = players.iter().map(|player| player.name.clone()).collect();
    names.sort();
    if !names.is_empty() {
        lines.push(format!("Players ({})", names.len()));
        for name in names {
            lines.push(format!("  • {name}"));
        }
    }

    let snapshot = lines.join("\n");
    if snapshot != *last_snapshot {
        text.0 = snapshot.clone();
        *last_snapshot = snapshot;
    }
}

pub fn update_chat_log_hud(chat_log: Res<ChatLog>, mut log_text: Single<&mut Text, With<ChatLogText>>) {
    if chat_log.is_changed() {
        log_text.0 = chat_log.messages.join("\n");
    }
}

pub fn update_chat_input_hud(
    chat_input: Res<ChatInput>,
    mut prompt: Query<
        (&mut Text, &mut Visibility),
        (With<ChatInputPrompt>, Without<ChatLogText>),
    >,
) {
    if !chat_input.is_changed() {
        return;
    }

    let Ok((mut prompt_text, mut visibility)) = prompt.single_mut() else {
        return;
    };

    if chat_input.active {
        *visibility = Visibility::Visible;
        prompt_text.0 = format!("> {}", chat_input.buffer);
    } else {
        *visibility = Visibility::Hidden;
        prompt_text.0.clear();
    }
}

pub fn chat_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut chat_input: ResMut<ChatInput>,
    mut chat_log: ResMut<ChatLog>,
    menu_open: Res<GameMenuOpen>,
    role: Res<NetworkRole>,
    local_player: Query<&Name, With<Player>>,
    mut keyboard_events: MessageReader<KeyboardInput>,
    mut commands: Commands,
) {
    if menu_open.0 {
        return;
    }

    if keys.just_pressed(KeyCode::KeyT) && !chat_input.active {
        chat_input.active = true;
        chat_input.buffer.clear();
        return;
    }

    if !chat_input.active {
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        chat_input.active = false;
        chat_input.buffer.clear();
        return;
    }

    if keys.just_pressed(KeyCode::Enter) {
        let text = chat_input.buffer.trim().to_string();
        if !text.is_empty() {
            let sender = local_player
                .single()
                .map(|name| name.as_str().to_string())
                .unwrap_or_else(|_| "Player".to_string());

            match &*role {
                NetworkRole::Client { .. } => {
                    commands.client_trigger(ClientChatMessage { sender, text });
                }
                NetworkRole::Host { .. } => {
                    chat_log.push(&sender, &text);
                    commands.server_trigger(ToClients {
                        targets: SendTargets::CLIENTS_ONLY,
                        message: ChatBroadcast { sender, text },
                    });
                }
                NetworkRole::None => {
                    chat_log.push(&sender, &text);
                }
            }
        }
        chat_input.active = false;
        chat_input.buffer.clear();
        return;
    }

    if keys.just_pressed(KeyCode::Backspace) {
        chat_input.buffer.pop();
        return;
    }

    for event in keyboard_events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }

        let chars = chat_typed_characters(event);
        for ch in chars {
            if chat_input.buffer.len() >= CHAT_MAX_MESSAGE_LEN {
                break;
            }
            if !ch.is_control() {
                chat_input.buffer.push(ch);
            }
        }
    }
}

fn chat_typed_characters(event: &KeyboardInput) -> Vec<char> {
    if let Some(text) = &event.text {
        return text.chars().filter(|ch| !ch.is_control()).collect();
    }

    match &event.logical_key {
        Key::Character(text) => text.chars().filter(|ch| !ch.is_control()).collect(),
        _ => Vec::new(),
    }
}
