use bevy::color::Mix;
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap};
use bevy::prelude::*;
use crate::player::PlayerCamera;
use crate::ui::game_menu::WorldScene;

const SKY_HALF_EXTENT: f32 = 512.0;
const CELESTIAL_DISTANCE: f32 = 460.0;
const CELESTIAL_SIZE: f32 = 52.0;
const DAY_LENGTH_SECS: f32 = 480.0;

const SIDE_TEXTURE: &str = "kenney_voxel-pack/PNG/Other/skybox_sideClouds.png";
const TOP_TEXTURE: &str = "kenney_voxel-pack/PNG/Other/skybox_top.png";
const BOTTOM_TEXTURE: &str = "kenney_voxel-pack/PNG/Other/skybox_bottom.png";
const SUN_TEXTURE: &str = "kenney_voxel-pack/PNG/Other/sun.png";
const MOON_TEXTURE: &str = "kenney_voxel-pack/PNG/Other/moon.png";

#[derive(Resource)]
pub(crate) struct DayNightCycle {
    pub phase: f32,
}

impl Default for DayNightCycle {
    fn default() -> Self {
        Self { phase: 0.2 }
    }
}

#[derive(Resource)]
pub(crate) struct SkyMaterials {
    side: Handle<StandardMaterial>,
    top: Handle<StandardMaterial>,
    bottom: Handle<StandardMaterial>,
}

#[derive(Component)]
pub(crate) struct SkyRoot;

#[derive(Component)]
struct SkyFace;

#[derive(Component, Clone, Copy)]
pub(crate) enum CelestialBody {
    Sun,
    Moon,
}

#[derive(Component)]
pub(crate) struct SunLight;

/// Unit vector from world origin toward the sun. Phase 0.25 = overhead (noon).
fn sun_direction(phase: f32) -> Vec3 {
    let angle = phase * std::f32::consts::TAU;
    Vec3::new(angle.cos(), angle.sin(), 0.0)
}

fn moon_direction(phase: f32) -> Vec3 {
    sun_direction((phase + 0.5) % 1.0)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 >= edge1 {
        return (x >= edge1) as u32 as f32;
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn horizon_fade(elevation: f32) -> f32 {
    smoothstep(-0.04, 0.06, elevation)
}

/// World-space rotation for a directional light shining from `sun_dir` (ground → sun).
fn directional_light_rotation(sun_dir: Vec3) -> Quat {
    let sun_dir = sun_dir.normalize_or_zero();
    if sun_dir.length_squared() < f32::EPSILON {
        return Quat::IDENTITY;
    }
    // Bevy directional lights shine along their forward axis (-Z).
    Quat::from_rotation_arc(Vec3::NEG_Z, -sun_dir)
}

pub(crate) fn spawn_sun_and_ambient(commands: &mut Commands) {
    let cascade_shadow_config = CascadeShadowConfigBuilder {
        num_cascades: 4,
        first_cascade_far_bound: 24.0,
        maximum_distance: 192.0,
        ..default()
    }
    .build();

    commands.insert_resource(DirectionalLightShadowMap { size: 2048 });

    commands.spawn((
        WorldScene,
        SunLight,
        DirectionalLight {
            illuminance: 18_000.0,
            shadows_enabled: true,
            // Voxel faces are axis-aligned; lower bias keeps shadows glued to blocks.
            shadow_depth_bias: 0.008,
            shadow_normal_bias: 0.6,
            ..default()
        },
        Transform::from_rotation(directional_light_rotation(sun_direction(0.2))),
        cascade_shadow_config,
        Name::new("Sun"),
    ));

    commands.insert_resource(GlobalAmbientLight {
        brightness: 180.0,
        ..default()
    });
}

pub(crate) fn spawn_sky(
    commands: &mut Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let side_texture = asset_server.load(SIDE_TEXTURE);
    let top_texture = asset_server.load(TOP_TEXTURE);
    let bottom_texture = asset_server.load(BOTTOM_TEXTURE);
    let sun_texture = asset_server.load(SUN_TEXTURE);
    let moon_texture = asset_server.load(MOON_TEXTURE);

    let side_material = materials.add(StandardMaterial {
        base_color_texture: Some(side_texture),
        base_color: Color::WHITE,
        unlit: true,
        ..default()
    });
    let top_material = materials.add(StandardMaterial {
        base_color_texture: Some(top_texture),
        base_color: Color::WHITE,
        unlit: true,
        ..default()
    });
    let bottom_material = materials.add(StandardMaterial {
        base_color_texture: Some(bottom_texture),
        base_color: Color::WHITE,
        unlit: true,
        ..default()
    });

    commands.insert_resource(SkyMaterials {
        side: side_material.clone(),
        top: top_material.clone(),
        bottom: bottom_material.clone(),
    });

    let half = SKY_HALF_EXTENT;
    let plane_half = Vec2::splat(half);

    let sky_root = commands
        .spawn((
            WorldScene,
            SkyRoot,
            Transform::default(),
            Visibility::default(),
            Name::new("SkyRoot"),
        ))
        .id();

    let faces = [
        (Vec3::new(half, 0.0, 0.0), Vec3::NEG_X, side_material.clone()),
        (Vec3::new(-half, 0.0, 0.0), Vec3::X, side_material.clone()),
        (Vec3::new(0.0, 0.0, half), Vec3::NEG_Z, side_material.clone()),
        (Vec3::new(0.0, 0.0, -half), Vec3::Z, side_material),
        (Vec3::new(0.0, half, 0.0), Vec3::NEG_Y, top_material),
        (Vec3::new(0.0, -half, 0.0), Vec3::Y, bottom_material),
    ];

    for (position, normal, material) in faces {
        commands.entity(sky_root).with_children(|parent| {
            parent.spawn((
                SkyFace,
                Mesh3d(meshes.add(Plane3d::new(normal, plane_half))),
                MeshMaterial3d(material),
                Transform::from_translation(position),
            ));
        });
    }

    let sun_material = materials.add(StandardMaterial {
        base_color_texture: Some(sun_texture),
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        emissive: LinearRgba::from(Color::srgb(1.0, 0.92, 0.55)),
        ..default()
    });
    let moon_material = materials.add(StandardMaterial {
        base_color_texture: Some(moon_texture),
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        emissive: LinearRgba::from(Color::srgb(0.75, 0.78, 0.9)),
        ..default()
    });

    let celestial_half = Vec2::splat(CELESTIAL_SIZE * 0.5);
    let celestial_mesh = meshes.add(Plane3d::new(Vec3::Z, celestial_half));

    commands.entity(sky_root).with_children(|parent| {
        parent.spawn((
            CelestialBody::Sun,
            Mesh3d(celestial_mesh.clone()),
            MeshMaterial3d(sun_material),
            Transform::default(),
            Visibility::Visible,
            Name::new("SunSprite"),
        ));
        parent.spawn((
            CelestialBody::Moon,
            Mesh3d(celestial_mesh),
            MeshMaterial3d(moon_material),
            Transform::default(),
            Visibility::Visible,
            Name::new("MoonSprite"),
        ));
    });
}

pub(crate) fn follow_sky_to_camera(
    camera: Query<&GlobalTransform, With<PlayerCamera>>,
    mut sky: Query<&mut Transform, With<SkyRoot>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let Ok(mut sky_transform) = sky.single_mut() else {
        return;
    };
    sky_transform.translation = camera.translation();
}

pub(crate) fn update_day_night(
    time: Res<Time>,
    mut cycle: ResMut<DayNightCycle>,
    sky_materials: Option<Res<SkyMaterials>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut sun_lights: Query<
        (&mut DirectionalLight, &mut Transform),
        (With<SunLight>, Without<CelestialBody>),
    >,
    mut celestial: Query<
        (&CelestialBody, &mut Transform, &mut Visibility),
        Without<SunLight>,
    >,
) {
    cycle.phase = (cycle.phase + time.delta_secs() / DAY_LENGTH_SECS) % 1.0;

    let sun_dir = sun_direction(cycle.phase);
    let moon_dir = moon_direction(cycle.phase);
    let daylight = sun_dir.y.clamp(0.0, 1.0);
    let twilight = 1.0 - smoothstep(0.0, 0.12, sun_dir.y);

    if let Some(sky_materials) = sky_materials {
        let night_tint = Color::srgb(0.3, 0.38, 0.58);
        let side_color = Color::WHITE.mix(&night_tint, twilight * 0.85);
        let top_color = Color::srgb(0.86, 0.93, 1.0).mix(
            &Color::srgb(0.04, 0.06, 0.16),
            twilight * 0.95,
        );
        let bottom_color = Color::srgb(0.72, 0.86, 1.0).mix(
            &Color::srgb(0.02, 0.04, 0.1),
            twilight * 0.9,
        );

        for (handle, color) in [
            (&sky_materials.side, side_color),
            (&sky_materials.top, top_color),
            (&sky_materials.bottom, bottom_color),
        ] {
            if let Some(material) = materials.get_mut(handle) {
                material.base_color = color;
            }
        }
    }

    for (mut light, mut transform) in &mut sun_lights {
        let active_dir = if sun_dir.y > 0.0 {
            sun_dir
        } else if moon_dir.y > 0.0 {
            moon_dir
        } else {
            Vec3::Y
        };

        transform.rotation = directional_light_rotation(active_dir);
        transform.translation = Vec3::ZERO;

        if sun_dir.y > 0.0 {
            let elevation = sun_dir.y.clamp(0.0, 1.0);
            light.illuminance = 3_000.0 + elevation * 22_000.0;
            light.color = Color::srgb(1.0, 0.94 + elevation * 0.04, 0.82 + elevation * 0.1);
            light.shadows_enabled = elevation > 0.08;
        } else if moon_dir.y > 0.0 {
            let elevation = moon_dir.y.clamp(0.0, 1.0);
            light.illuminance = 300.0 + elevation * 900.0;
            light.color = Color::srgb(0.65, 0.72, 0.95);
            light.shadows_enabled = false;
        } else {
            light.illuminance = 0.0;
            light.shadows_enabled = false;
        }
    }

    ambient.brightness = 30.0 + daylight * 170.0 + (1.0 - daylight) * moon_dir.y.max(0.0) * 60.0;
    ambient.color = if daylight > 0.05 {
        Color::srgb(0.92, 0.94, 1.0)
    } else {
        Color::srgb(0.45, 0.5, 0.75)
    };

    for (body, mut transform, mut visibility) in &mut celestial {
        let (direction, fade) = match body {
            CelestialBody::Sun => (sun_dir, horizon_fade(sun_dir.y)),
            CelestialBody::Moon => (moon_dir, horizon_fade(moon_dir.y)),
        };

        if fade <= 0.001 {
            *visibility = Visibility::Hidden;
            continue;
        }

        *visibility = Visibility::Visible;
        *transform = Transform::from_translation(direction * CELESTIAL_DISTANCE)
            .looking_at(Vec3::ZERO, Vec3::Y);
        transform.scale = Vec3::splat(fade);
    }
}
