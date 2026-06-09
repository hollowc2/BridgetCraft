use std::fs;
use std::path::Path;

fn main() {
    let out_dir = Path::new("assets/textures");
    fs::create_dir_all(out_dir).expect("failed to create assets/textures");

    let tiles_dir = Path::new("assets/kenney_voxel-pack/PNG/Tiles");
    let textures = [
        "grass_top.png",
        "dirt_grass.png",
        "dirt.png",
        "stone.png",
        "sand.png",
        "wood.png",
        "brick_red.png",
        "brick_grey.png",
        "glass.png",
        "gravel_stone.png",
        "greystone_sand.png",
        "snow.png",
        "leaves_transparent.png",
        "trunk_top.png",
        "trunk_side.png",
        "trunk_white_top.png",
        "trunk_white_side.png",
        "water.png",
        "greystone.png",
        "fence_wood.png",
        "cotton_blue.png",
        "redstone_emerald.png",
        "grass1.png",
    ];

    let tile_size = 128;
    let atlas_height = tile_size * textures.len() as u32;
    let mut atlas = image::RgbaImage::new(tile_size, atlas_height);

    for (index, file) in textures.iter().enumerate() {
        let path = tiles_dir.join(file);
        let tile = image::open(&path).unwrap_or_else(|err| {
            panic!("failed to load texture {}: {err}", path.display());
        });
        let tile = tile.to_rgba8();
        assert_eq!(tile.width(), tile_size);
        assert_eq!(tile.height(), tile_size);
        image::imageops::overlay(
            &mut atlas,
            &tile,
            0,
            (index as u32 * tile_size) as i64,
        );
    }

    let atlas_path = out_dir.join("voxel_atlas.png");
    atlas
        .save(&atlas_path)
        .unwrap_or_else(|err| panic!("failed to save {}: {err}", atlas_path.display()));

    build_sky_cubemap(out_dir);

    println!("cargo:rerun-if-changed=build.rs");
    for file in textures {
        println!("cargo:rerun-if-changed={}", tiles_dir.join(file).display());
    }
}

fn build_sky_cubemap(out_dir: &Path) {
    let other_dir = Path::new("assets/kenney_voxel-pack/PNG/Other");
    let face_size = 256u32;
    let faces = [
        other_dir.join("skybox_sideClouds.png"), // +X
        other_dir.join("skybox_sideClouds.png"), // -X
        other_dir.join("skybox_top.png"),        // +Y
        other_dir.join("skybox_bottom.png"),     // -Y
        other_dir.join("skybox_sideClouds.png"), // +Z
        other_dir.join("skybox_sideClouds.png"), // -Z
    ];

    let mut cubemap = image::RgbaImage::new(face_size, face_size * faces.len() as u32);
    for (index, path) in faces.iter().enumerate() {
        let face = image::open(path).unwrap_or_else(|err| {
            panic!("failed to load skybox face {}: {err}", path.display());
        });
        let face = face
            .resize_exact(face_size, face_size, image::imageops::FilterType::Triangle)
            .to_rgba8();
        image::imageops::overlay(
            &mut cubemap,
            &face,
            0,
            (index as u32 * face_size) as i64,
        );
    }

    let cubemap_path = out_dir.join("sky_cubemap.png");
    cubemap
        .save(&cubemap_path)
        .unwrap_or_else(|err| panic!("failed to save {}: {err}", cubemap_path.display()));

    for path in faces {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-changed={}", cubemap_path.display());
}
