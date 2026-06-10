use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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
    write_png_atomically(&atlas_path, &atlas);

    build_sky_cubemap(out_dir);

    // Only depend on inputs. Do NOT rerun-if-changed on generated outputs: build.rs writes
    // those files, so watching them forces a full rebuild on every `cargo run`.
    println!("cargo:rerun-if-changed=build.rs");
    for file in textures {
        println!("cargo:rerun-if-changed={}", tiles_dir.join(file).display());
    }
}

/// Write PNG to `path` via a temp file + rename so Bevy never reads a half-written atlas.
fn write_png_atomically(path: &Path, image: &image::RgbaImage) {
    let tmp = temp_path(path);

    {
        let mut file = fs::File::create(&tmp)
            .unwrap_or_else(|err| panic!("failed to create {}: {err}", tmp.display()));
        let mut buffer = Vec::new();
        image::DynamicImage::ImageRgba8(image.clone())
            .write_to(
                &mut std::io::Cursor::new(&mut buffer),
                image::ImageFormat::Png,
            )
            .unwrap_or_else(|err| panic!("failed to encode {}: {err}", path.display()));
        file.write_all(&buffer)
            .unwrap_or_else(|err| panic!("failed to write {}: {err}", tmp.display()));
        file.sync_all()
            .unwrap_or_else(|err| panic!("failed to flush {}: {err}", tmp.display()));
    }

    // Sanity-check before exposing the file to the asset server.
    let bytes = fs::read(&tmp)
        .unwrap_or_else(|err| panic!("failed to read back {}: {err}", tmp.display()));
    image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .unwrap_or_else(|err| panic!("generated PNG is unreadable at {}: {err}", tmp.display()));

    fs::rename(&tmp, path).unwrap_or_else(|err| {
        panic!(
            "failed to move {} -> {}: {err}",
            tmp.display(),
            path.display()
        )
    });
}

fn temp_path(path: &Path) -> PathBuf {
    path.with_extension("tmp.png")
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
    write_png_atomically(&cubemap_path, &cubemap);

    for path in faces {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
