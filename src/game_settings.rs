use bevy::prelude::*;

use crate::net::replicate::NetworkGameSettings;
use crate::net::NetworkRole;

#[derive(Resource, Clone, Copy, Debug)]
pub struct GameSettings {
    pub speed: f32,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self { speed: 1.0 }
    }
}

impl GameSettings {
    pub const MIN_SPEED: f32 = 0.25;
    pub const MAX_SPEED: f32 = 4.0;

    pub fn clamp_speed(speed: f32) -> f32 {
        speed.clamp(Self::MIN_SPEED, Self::MAX_SPEED)
    }
}

pub fn reset_game_settings(mut settings: ResMut<GameSettings>, mut time: ResMut<Time<Virtual>>) {
    settings.speed = 1.0;
    time.set_relative_speed(1.0);
}

pub fn apply_game_speed(settings: Res<GameSettings>, mut time: ResMut<Time<Virtual>>) {
    let speed = GameSettings::clamp_speed(settings.speed);
    if (time.relative_speed() - speed).abs() > f32::EPSILON {
        time.set_relative_speed(speed);
    }
}

pub fn sync_game_settings_to_network(
    settings: Res<GameSettings>,
    role: Res<NetworkRole>,
    mut network: Query<&mut NetworkGameSettings>,
) {
    if role.is_client() || !settings.is_changed() {
        return;
    }

    for mut net in &mut network {
        net.speed = settings.speed;
    }
}

pub fn sync_network_game_settings(
    network: Query<&NetworkGameSettings, Changed<NetworkGameSettings>>,
    mut settings: ResMut<GameSettings>,
    role: Res<NetworkRole>,
) {
    if !role.is_client() {
        return;
    }

    for net in &network {
        settings.speed = GameSettings::clamp_speed(net.speed);
    }
}
