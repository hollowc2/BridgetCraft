use bevy::prelude::*;
use noise::{HybridMulti, NoiseFn, Perlin};

const SPLASH_SIZE: i32 = 18;
const TILES: &str = "kenney_voxel-pack/PNG/Tiles";

#[derive(Component)]
pub struct MenuSplashScene;

#[derive(Component)]
pub struct MenuSplashCamera;

#[derive(Clone)]
struct SplashMaterials {
    grass: Handle<StandardMaterial>,
    sand: Handle<StandardMaterial>,
    leaves: Handle<StandardMaterial>,
    trunk: Handle<StandardMaterial>,
}

pub fn spawn_menu_splash(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let seed = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0) as u32)
        .wrapping_mul(1_103_515_245);

    let height_noise = build_preview_height_noise(seed);
    let tree_noise = Perlin::new(seed.wrapping_add(77_007));
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let splash_materials = build_splash_materials(&asset_server, &mut materials);

    for x in 0..SPLASH_SIZE {
        for z in 0..SPLASH_SIZE {
            let height = preview_height(&height_noise, x, z);
            let density = tree_noise.get([x as f64 * 0.21, z as f64 * 0.21]);
            let has_tree = density > 0.62;
            let ground = if has_tree {
                &splash_materials.leaves
            } else if height <= 4 {
                &splash_materials.sand
            } else {
                &splash_materials.grass
            };

            commands.spawn((
                MenuSplashScene,
                Mesh3d(cube.clone()),
                MeshMaterial3d(ground.clone()),
                Transform::from_xyz(
                    x as f32 - SPLASH_SIZE as f32 * 0.5,
                    height as f32 + 0.5,
                    z as f32 - SPLASH_SIZE as f32 * 0.5,
                ),
                GlobalTransform::default(),
                Visibility::default(),
            ));

            if has_tree {
                for trunk_y in 1..=3 {
                    commands.spawn((
                        MenuSplashScene,
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(splash_materials.trunk.clone()),
                        Transform::from_xyz(
                            x as f32 - SPLASH_SIZE as f32 * 0.5,
                            height as f32 + trunk_y as f32 + 0.5,
                            z as f32 - SPLASH_SIZE as f32 * 0.5,
                        ),
                        GlobalTransform::default(),
                        Visibility::default(),
                    ));
                }
            }
        }
    }

    commands.spawn((
        MenuSplashScene,
        DirectionalLight {
            illuminance: 14_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.1, 0.6, 0.0)),
    ));

    commands.spawn((
        MenuSplashScene,
        MenuSplashCamera,
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.05, 0.08, 0.14).into()),
            ..default()
        },
        Transform::from_xyz(14.0, 11.0, 14.0).looking_at(Vec3::new(0.0, 4.0, 0.0), Vec3::Y),
        GlobalTransform::default(),
    ));
}

fn build_splash_materials(
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> SplashMaterials {
    // Use Kenney tile PNGs directly. Do not use `textures/voxel_atlas.png` here: bevy_voxel_world
    // reinterprets that asset as a 2D texture array, which StandardMaterial cannot sample.
    let mut tile_material = |path: &str| {
        materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load(format!("{TILES}/{path}"))),
            perceptual_roughness: 0.92,
            metallic: 0.0,
            ..default()
        })
    };

    SplashMaterials {
        grass: tile_material("grass_top.png"),
        sand: tile_material("sand.png"),
        leaves: tile_material("leaves_transparent.png"),
        trunk: tile_material("trunk_side.png"),
    }
}

pub fn rotate_menu_splash(
    time: Res<Time>,
    mut cameras: Query<&mut Transform, With<MenuSplashCamera>>,
) {
    for mut transform in &mut cameras {
        let center = Vec3::new(0.0, 4.0, 0.0);
        let offset = transform.translation - center;
        let angle = time.delta_secs() * 0.18;
        let rotated = Vec3::new(
            offset.x * angle.cos() - offset.z * angle.sin(),
            offset.y,
            offset.x * angle.sin() + offset.z * angle.cos(),
        );
        transform.translation = center + rotated;
        transform.look_at(center, Vec3::Y);
    }
}

pub fn cleanup_menu_splash(mut commands: Commands, scene: Query<Entity, With<MenuSplashScene>>) {
    for entity in &scene {
        commands.entity(entity).despawn();
    }
}

fn build_preview_height_noise(seed: u32) -> HybridMulti<Perlin> {
    let mut noise = HybridMulti::<Perlin>::new(seed);
    noise.octaves = 3;
    noise.frequency = 0.42;
    noise.lacunarity = 2.0;
    noise.persistence = 0.45;
    noise
}

fn preview_height(noise: &HybridMulti<Perlin>, x: i32, z: i32) -> i32 {
    let sample = noise.get([x as f64 * 0.12, z as f64 * 0.12]);
    3 + (sample * 4.0).round() as i32
}
