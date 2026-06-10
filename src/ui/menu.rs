use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::audio::GameAudio;
use crate::net::NetworkRole;
use crate::world_gen::WorldMetadata;
use crate::AppState;

const MAX_NAME_LEN: usize = 32;
const MAX_ADDRESS_LEN: usize = 64;

#[derive(Resource)]
pub struct MenuSettings {
    pub player_name: String,
    pub world_name: String,
    pub join_address: String,
}

impl Default for MenuSettings {
    fn default() -> Self {
        Self {
            player_name: "Pebble Picker".to_string(),
            world_name: "Whispering Brickshire".to_string(),
            join_address: "127.0.0.1:7777".to_string(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuInputField {
    PlayerName,
    WorldName,
    JoinAddress,
}

#[derive(Resource)]
pub struct MenuFocus {
    pub active: Option<MenuInputField>,
    pub cursor_blink: Timer,
}

impl Default for MenuFocus {
    fn default() -> Self {
        Self {
            active: None,
            cursor_blink: Timer::from_seconds(0.5, TimerMode::Repeating),
        }
    }
}

#[derive(Component)]
pub struct MenuRoot;

#[derive(Component)]
pub struct MenuButton(&'static str);

#[derive(Component)]
pub struct MenuTextInput(pub MenuInputField);

#[derive(Component)]
pub struct MenuInputLabel;

pub fn spawn_main_menu(mut commands: Commands, settings: Res<MenuSettings>) {
    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..Default::default()
            },
            BackgroundColor(Color::srgba(0.08, 0.12, 0.18, 0.72)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("BridgetCraft"),
                TextFont {
                    font_size: 48.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
            ));

            spawn_text_input(
                parent,
                MenuInputField::PlayerName,
                "Player name",
                &settings.player_name,
            );
            spawn_text_input(
                parent,
                MenuInputField::WorldName,
                "World name",
                &settings.world_name,
            );

            for (label, action) in [
                ("Singleplayer", "singleplayer"),
                ("Host Game", "host"),
                ("Join Game", "join"),
                ("Quit", "quit"),
            ] {
                parent
                    .spawn((
                        Button,
                        MenuButton(action),
                        Node {
                            width: Val::Px(260.0),
                            height: Val::Px(52.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..Default::default()
                        },
                        BackgroundColor(Color::srgb(0.2, 0.45, 0.75)),
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

            spawn_text_input(
                parent,
                MenuInputField::JoinAddress,
                "Join address",
                &settings.join_address,
            );

            parent.spawn((
                Text::new("Click a field to edit · Enter or Escape to finish"),
                TextFont {
                    font_size: 14.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.55, 0.6, 0.68)),
            ));
        });
}

fn spawn_text_input(
    parent: &mut ChildSpawnerCommands,
    field: MenuInputField,
    label: &str,
    value: &str,
) {
    parent
        .spawn((
            Button,
            MenuTextInput(field),
            Node {
                width: Val::Px(360.0),
                height: Val::Px(40.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 0.85)),
        ))
        .with_child((
            MenuInputLabel,
            Text::new(format!("{label}: {value}")),
            TextFont {
                font_size: 20.0,
                ..Default::default()
            },
            TextColor(Color::WHITE),
        ));
}

pub fn menu_input_focus(
    mut focus: ResMut<MenuFocus>,
    mut interaction_query: Query<
        (&Interaction, &MenuTextInput, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, input, mut color) in &mut interaction_query {
        let selected = focus.active == Some(input.0);
        match *interaction {
            Interaction::Pressed => {
                focus.active = Some(input.0);
                focus.cursor_blink.reset();
                *color = input_focus_color(true).into();
            }
            Interaction::Hovered => {
                *color = input_focus_color(selected).into();
            }
            Interaction::None => {
                *color = input_focus_color(selected).into();
            }
        }
    }
}

pub fn menu_input_keyboard(
    focus: Res<MenuFocus>,
    mut settings: ResMut<MenuSettings>,
    mut keyboard_events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let Some(active) = focus.active else {
        return;
    };

    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
        return;
    }

    if keys.just_pressed(KeyCode::Backspace) {
        let value = menu_field_value_mut(&mut settings, active);
        value.pop();
        return;
    }

    let max_len = match active {
        MenuInputField::JoinAddress => MAX_ADDRESS_LEN,
        _ => MAX_NAME_LEN,
    };

    for event in keyboard_events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }

        let chars = menu_typed_characters(event);
        if chars.is_empty() {
            continue;
        }

        let value = menu_field_value_mut(&mut settings, active);
        for ch in chars {
            if value.len() >= max_len {
                break;
            }
            if !ch.is_control() {
                value.push(ch);
            }
        }
    }
}

fn menu_typed_characters(event: &KeyboardInput) -> Vec<char> {
    if let Some(text) = &event.text {
        return text.chars().filter(|ch| !ch.is_control()).collect();
    }

    match &event.logical_key {
        Key::Character(text) => text.chars().filter(|ch| !ch.is_control()).collect(),
        _ => Vec::new(),
    }
}

pub fn menu_input_unfocus(
    mut focus: ResMut<MenuFocus>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if focus.active.is_some()
        && (keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter))
    {
        focus.active = None;
    }
}

pub fn menu_input_display(
    time: Res<Time>,
    mut focus: ResMut<MenuFocus>,
    settings: Res<MenuSettings>,
    mut labels: Query<(&MenuTextInput, &mut Text, &mut TextColor), With<MenuInputLabel>>,
    mut backgrounds: Query<(&MenuTextInput, &mut BackgroundColor), With<Button>>,
) {
    focus.cursor_blink.tick(time.delta());
    let show_cursor = focus.active.is_some() && focus.cursor_blink.fraction() < 0.5;

    for (input, mut text, mut color) in &mut labels {
        let (label, value) = menu_field_display(&settings, input.0);
        let mut display = format!("{label}: {value}");
        if focus.active == Some(input.0) && show_cursor {
            display.push('|');
        }
        **text = display;
        *color = if focus.active == Some(input.0) {
            TextColor(Color::srgb(0.95, 0.98, 1.0))
        } else {
            TextColor(Color::WHITE)
        };
    }

    for (input, mut color) in &mut backgrounds {
        let selected = focus.active == Some(input.0);
        *color = input_focus_color(selected).into();
    }
}

pub fn menu_button_interaction(
    mut interaction_query: Query<
        (&Interaction, &MenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut next_state: ResMut<NextState<AppState>>,
    mut role: ResMut<NetworkRole>,
    mut metadata: ResMut<WorldMetadata>,
    settings: Res<MenuSettings>,
    mut focus: ResMut<MenuFocus>,
    mut exit: MessageWriter<AppExit>,
    mut audio: ResMut<GameAudio>,
    mut commands: Commands,
) {
    for (interaction, button, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                audio.play_ui_click(&mut commands);
                *color = Color::srgb(0.15, 0.35, 0.6).into();
                focus.active = None;
                match button.0 {
                    "singleplayer" => {
                        role.set_singleplayer();
                        metadata.name = trimmed_or_default(
                            &settings.world_name,
                            &MenuSettings::default().world_name,
                        );
                        next_state.set(AppState::InGame);
                    }
                    "host" => {
                        role.set_host(7777);
                        metadata.name = trimmed_or_default(
                            &settings.world_name,
                            &MenuSettings::default().world_name,
                        );
                        next_state.set(AppState::InGame);
                    }
                    "join" => {
                        let address = trimmed_or_default(
                            &settings.join_address,
                            &MenuSettings::default().join_address,
                        );
                        role.set_client(address);
                        metadata.name = "Joined World".to_string();
                        next_state.set(AppState::InGame);
                    }
                    "quit" => {
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

pub fn cleanup_menu(mut commands: Commands, menu: Query<Entity, With<MenuRoot>>) {
    for entity in &menu {
        commands.entity(entity).despawn();
    }
}

fn input_focus_color(selected: bool) -> Color {
    if selected {
        Color::srgba(0.22, 0.38, 0.58, 0.95)
    } else {
        Color::srgba(0.15, 0.2, 0.28, 0.85)
    }
}

fn menu_field_value_mut<'a>(settings: &'a mut MenuSettings, field: MenuInputField) -> &'a mut String {
    match field {
        MenuInputField::PlayerName => &mut settings.player_name,
        MenuInputField::WorldName => &mut settings.world_name,
        MenuInputField::JoinAddress => &mut settings.join_address,
    }
}

fn menu_field_display(settings: &MenuSettings, field: MenuInputField) -> (&'static str, &str) {
    match field {
        MenuInputField::PlayerName => ("Player name", settings.player_name.as_str()),
        MenuInputField::WorldName => ("World name", settings.world_name.as_str()),
        MenuInputField::JoinAddress => ("Join address", settings.join_address.as_str()),
    }
}

fn trimmed_or_default(value: &str, default: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn menu_player_name(settings: &MenuSettings) -> String {
    trimmed_or_default(
        &settings.player_name,
        &MenuSettings::default().player_name,
    )
}
