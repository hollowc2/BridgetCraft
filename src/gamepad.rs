use bevy::prelude::*;

/// Picks the best connected gamepad from an iterator, ignoring keyboards and other non-controller devices.
pub fn select_primary<'a>(
    gamepads: impl Iterator<Item = (&'a Name, &'a Gamepad)>,
) -> Option<&'a Gamepad> {
    gamepads
        .filter(|(name, _)| !is_non_gamepad(name.as_str()))
        .min_by_key(|(name, _)| gamepad_priority(name.as_str()))
        .map(|(_, pad)| pad)
}

fn is_non_gamepad(name: &str) -> bool {
    let lower = name.to_lowercase();
    [
        "keychron",
        "keyboard",
        "mouse",
        "system control",
        "touchpad",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn gamepad_priority(name: &str) -> u8 {
    let lower = name.to_lowercase();
    if lower.contains("xbox") || lower.contains("wireless receiver") {
        0
    } else if lower.contains("controller") || lower.contains("gamepad") {
        1
    } else {
        2
    }
}
