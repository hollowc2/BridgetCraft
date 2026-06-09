use bevy::prelude::*;

use crate::net::NetworkRole;
use crate::world_gen::WorldMetadata;
use crate::AppState;

#[derive(Component)]
pub struct MenuRoot;

#[derive(Component)]
pub struct MenuSceneCamera;

#[derive(Component)]
pub struct MenuButton(&'static str);

#[derive(Component)]
pub struct WorldNameInput;

#[derive(Component)]
pub struct JoinAddressInput;

#[derive(Component)]
pub struct PlayerNameInput;

pub fn spawn_main_menu(mut commands: Commands) {
    commands.spawn((
        MenuSceneCamera,
        Camera2d,
        Camera {
            order: 0,
            ..default()
        },
    ));

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
            BackgroundColor(Color::srgb(0.08, 0.12, 0.18)),
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

            parent.spawn((
                PlayerNameInput,
                Text::new("Player: Builder"),
                TextFont {
                    font_size: 20.0,
                    ..Default::default()
                },
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                WorldNameInput,
                Text::new("World: New World"),
                TextFont {
                    font_size: 20.0,
                    ..Default::default()
                },
                TextColor(Color::WHITE),
            ));

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

            parent.spawn((
                JoinAddressInput,
                Text::new("Join address: 127.0.0.1:7777"),
                TextFont {
                    font_size: 18.0,
                    ..Default::default()
                },
                TextColor(Color::srgb(0.8, 0.85, 0.9)),
            ));
        });
}

pub fn menu_button_interaction(
    mut interaction_query: Query<
        (&Interaction, &MenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut next_state: ResMut<NextState<AppState>>,
    mut role: ResMut<NetworkRole>,
    mut metadata: ResMut<WorldMetadata>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, button, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = Color::srgb(0.15, 0.35, 0.6).into();
                match button.0 {
                    "singleplayer" => {
                        role.set_singleplayer();
                        metadata.name = "New World".to_string();
                        next_state.set(AppState::InGame);
                    }
                    "host" => {
                        role.set_host(7777);
                        metadata.name = "Hosted World".to_string();
                        next_state.set(AppState::InGame);
                    }
                    "join" => {
                        role.set_client("127.0.0.1:7777".to_string());
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
                *color = Color::srgb(0.25, 0.55, 0.85).into();
            }
            Interaction::None => {
                *color = Color::srgb(0.2, 0.45, 0.75).into();
            }
        }
    }
}

pub fn cleanup_menu(
    mut commands: Commands,
    menu: Query<Entity, With<MenuRoot>>,
    cameras: Query<Entity, With<MenuSceneCamera>>,
) {
    for entity in &menu {
        commands.entity(entity).despawn();
    }
    for entity in &cameras {
        commands.entity(entity).despawn();
    }
}
