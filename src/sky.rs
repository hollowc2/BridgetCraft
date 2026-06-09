use bevy::color::Mix;
use bevy::core_pipeline::Skybox;
use bevy::light::{CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap};
use bevy::prelude::*;
use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
use crate::player::{PlayerCamera, PlayerSettings, ShadowQuality};
use crate::ui::game_menu::WorldScene;

const SKY_CUBEMAP: &str = "textures/sky_cubemap.png";
const CELESTIAL_DISTANCE: f32 = 460.0;
const CELESTIAL_SIZE: f32 = 52.0;
const DAY_LENGTH_SECS: f32 = 480.0;
const DAY_SKY_BRIGHTNESS: f32 = 1_000.0;
const NIGHT_SKY_BRIGHTNESS: f32 = 140.0;

const SUN_TEXTURE: &str = "kenney_voxel-pack/PNG/Other/sun.png";
const MOON_TEXTURE: &str = "kenney_voxel-pack/PNG/Other/moon.png";

#[derive(Resource)]
pub(crate) struct SkyCubemap {
    image: Handle<Image>,
    configured: bool,
}

#[derive(Resource)]
pub(crate) struct DayNightCycle {
    pub phase: f32,
}

impl Default for DayNightCycle {
    fn default() -> Self {
        Self { phase: 0.2 }
    }
}

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

fn sky_brightness(twilight: f32) -> f32 {
    DAY_SKY_BRIGHTNESS + (NIGHT_SKY_BRIGHTNESS - DAY_SKY_BRIGHTNESS) * twilight * 0.9
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

fn cascade_shadow_config_for(quality: ShadowQuality) -> CascadeShadowConfig {
    CascadeShadowConfigBuilder {
        num_cascades: match quality {
            ShadowQuality::Off | ShadowQuality::Low => 2,
            ShadowQuality::High => 4,
        },
        first_cascade_far_bound: 24.0,
        maximum_distance: 192.0,
        ..default()
    }
    .build()
}

fn shadow_map_size_for(quality: ShadowQuality) -> usize {
    match quality {
        ShadowQuality::Off | ShadowQuality::Low => 1024,
        ShadowQuality::High => 2048,
    }
}

pub(crate) fn spawn_sun_and_ambient(commands: &mut Commands, settings: &PlayerSettings) {
    let cascade_shadow_config = cascade_shadow_config_for(settings.shadow_quality);

    commands.insert_resource(DirectionalLightShadowMap {
        size: shadow_map_size_for(settings.shadow_quality),
    });

    commands.spawn((
        WorldScene,
        SunLight,
        DirectionalLight {
            illuminance: 18_000.0,
            shadows_enabled: settings.shadow_quality != ShadowQuality::Off,
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

pub(crate) fn spawn_sky(commands: &mut Commands, asset_server: &Res<AssetServer>) {
    let cubemap = asset_server.load(SKY_CUBEMAP);
    commands.insert_resource(SkyCubemap {
        image: cubemap,
        configured: false,
    });
}

pub(crate) fn spawn_celestial_bodies(
    commands: &mut Commands,
    camera: Entity,
    asset_server: &Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sun_texture = asset_server.load(SUN_TEXTURE);
    let moon_texture = asset_server.load(MOON_TEXTURE);

    let sun_material = materials.add(StandardMaterial {
        base_color_texture: Some(sun_texture),
        base_color: Color::srgb(1.0, 0.95, 0.7),
        alpha_mode: AlphaMode::Mask(0.2),
        unlit: true,
        ..default()
    });
    let moon_material = materials.add(StandardMaterial {
        base_color_texture: Some(moon_texture),
        base_color: Color::srgb(0.85, 0.88, 0.95),
        alpha_mode: AlphaMode::Mask(0.2),
        unlit: true,
        ..default()
    });

    let celestial_half = Vec2::splat(CELESTIAL_SIZE * 0.5);
    let celestial_mesh = meshes.add(Plane3d::new(Vec3::Z, celestial_half));

    commands.entity(camera).with_children(|parent| {
        parent.spawn((
            WorldScene,
            CelestialBody::Sun,
            Mesh3d(celestial_mesh.clone()),
            MeshMaterial3d(sun_material),
            Transform::default(),
            Visibility::Hidden,
            Name::new("SunSprite"),
        ));
        parent.spawn((
            WorldScene,
            CelestialBody::Moon,
            Mesh3d(celestial_mesh),
            MeshMaterial3d(moon_material),
            Transform::default(),
            Visibility::Hidden,
            Name::new("MoonSprite"),
        ));
    });
}

pub(crate) fn configure_sky_cubemap(
    mut cubemap: ResMut<SkyCubemap>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<(Entity, Option<&mut Skybox>), With<PlayerCamera>>,
    mut commands: Commands,
) {
    if cubemap.configured {
        return;
    }

    if !asset_server.is_loaded_with_dependencies(&cubemap.image) {
        return;
    }

    let Some(image) = images.get_mut(&cubemap.image) else {
        return;
    };

    if image.texture_descriptor.array_layer_count() == 1 {
        image
            .reinterpret_stacked_2d_as_array(image.height() / image.width())
            .expect("sky cubemap should be a vertical stack of square faces");
        image.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..default()
        });
    }

    let brightness = sky_brightness(0.0);

    for (entity, existing) in &mut cameras {
        match existing {
            Some(mut skybox) => {
                skybox.image = cubemap.image.clone();
                skybox.brightness = brightness;
            }
            None => {
                commands.entity(entity).insert(Skybox {
                    image: cubemap.image.clone(),
                    brightness,
                    ..default()
                });
            }
        }
    }

    cubemap.configured = true;
}

pub(crate) fn apply_shadow_settings(
    settings: Res<PlayerSettings>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
    mut sun_lights: Query<
        (&mut DirectionalLight, &mut CascadeShadowConfig),
        (With<SunLight>, Without<CelestialBody>),
    >,
) {
    if !settings.is_changed() {
        return;
    }

    shadow_map.size = shadow_map_size_for(settings.shadow_quality);
    let cascade_shadow_config = cascade_shadow_config_for(settings.shadow_quality);

    for (mut light, mut cascade) in &mut sun_lights {
        *cascade = cascade_shadow_config.clone();
        if settings.shadow_quality == ShadowQuality::Off {
            light.shadows_enabled = false;
        }
    }
}

pub(crate) fn update_day_night(
    time: Res<Time>,
    settings: Res<PlayerSettings>,
    mut cycle: ResMut<DayNightCycle>,
    mut ambient: ResMut<GlobalAmbientLight>,
    camera: Query<&Transform, With<PlayerCamera>>,
    mut sun_lights: Query<
        (&mut DirectionalLight, &mut Transform),
        (With<SunLight>, Without<CelestialBody>),
    >,
    mut celestial: Query<
        (&CelestialBody, &mut Transform, &mut Visibility),
        Without<SunLight>,
    >,
    mut skyboxes: Query<&mut Skybox, With<PlayerCamera>>,
    mut cached_brightness: Local<f32>,
) {
    cycle.phase = (cycle.phase + time.delta_secs() / DAY_LENGTH_SECS) % 1.0;

    let sun_dir = sun_direction(cycle.phase);
    let moon_dir = moon_direction(cycle.phase);
    let daylight = sun_dir.y.clamp(0.0, 1.0);
    let twilight = 1.0 - smoothstep(0.0, 0.12, sun_dir.y);
    let brightness = sky_brightness(twilight);

    if (*cached_brightness - brightness).abs() > 0.5 {
        *cached_brightness = brightness;
        for mut skybox in &mut skyboxes {
            skybox.brightness = brightness;
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
            light.shadows_enabled =
                settings.shadow_quality != ShadowQuality::Off && elevation > 0.08;
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
        Color::srgb(0.45, 0.5, 0.75).mix(
            &Color::srgb(0.3, 0.38, 0.58),
            twilight * 0.35,
        )
    };

    let Ok(camera_transform) = camera.single() else {
        return;
    };
    let camera_rotation = camera_transform.rotation;

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
        let local_offset = camera_rotation.inverse() * (direction * CELESTIAL_DISTANCE);
        let mut billboard = Transform::from_translation(local_offset);
        billboard.look_at(Vec3::ZERO, Vec3::Y);
        billboard.scale = Vec3::splat(fade);
        *transform = billboard;
    }
}
