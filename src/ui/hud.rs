use bevy::prelude::*;

use crate::block::{BlockId, HotbarSelection, HOTBAR_SIZE};
use crate::net::NetworkRole;
use crate::player::PlayerSettings;
use crate::AppState;

#[derive(Component)]
pub struct HudRoot;

#[derive(Component)]
pub struct Crosshair;

#[derive(Component)]
pub struct HotbarText;

#[derive(Component)]
pub struct NetworkInfoText;

#[derive(Component)]
pub struct NameTag;

pub fn spawn_hud(commands: &mut Commands) {
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
        });

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                bottom: Val::Px(24.0),
                justify_content: JustifyContent::Center,
                ..Default::default()
            },
            HudRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                HotbarText,
                Text::new("Hotbar"),
                TextFont {
                    font_size: 18.0,
                    ..Default::default()
                },
                TextColor(Color::WHITE),
                Node {
                    padding: UiRect::all(Val::Px(8.0)),
                    ..Default::default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            ));
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(12.0),
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
}

pub fn update_hotbar_text(selection: Res<HotbarSelection>, mut text: Single<&mut Text, With<HotbarText>>) {
    let mut slots = String::new();
    for (index, block) in BlockId::HOTBAR.iter().enumerate() {
        let marker = if index == selection.index { '[' } else { ' ' };
        let end = if index == selection.index { ']' } else { ' ' };
        slots.push_str(&format!("{marker}{}{end} ", block.label()));
    }
    text.0 = format!("{slots}\n(1-{HOTBAR_SIZE}) scroll to change");
}

pub fn hotbar_scroll(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut selection: ResMut<HotbarSelection>,
    keys: Res<ButtonInput<KeyCode>>,
) {
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
}

pub fn update_network_info(
    role: Res<NetworkRole>,
    settings: Res<PlayerSettings>,
    mut text: Single<&mut Text, With<NetworkInfoText>>,
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
    text.0 = lines.join("\n");
}

pub fn spawn_name_tag(commands: &mut Commands, player: Entity, name: &str) {
    commands.entity(player).with_children(|parent| {
        parent.spawn((
            NameTag,
            Text2d::new(name),
            TextFont {
                font_size: 22.0,
                ..Default::default()
            },
            TextColor(Color::WHITE),
            Transform::from_xyz(0.0, 2.2, 0.0),
        ));
    });
}

pub fn cleanup_hud(
    mut commands: Commands,
    hud: Query<Entity, With<HudRoot>>,
    mut next_state: ResMut<NextState<AppState>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for entity in &hud {
            commands.entity(entity).despawn();
        }
        next_state.set(AppState::MainMenu);
    }
}
