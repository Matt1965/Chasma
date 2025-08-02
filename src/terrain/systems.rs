use bevy::prelude::*;
use std::path::Path;
use bevy::render::mesh::{Mesh, Indices};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::math::{Vec2, UVec2};
use image::{GrayImage};
use crate::heightmap_data::HeightmapData;
use image::{io::Reader as ImageReader, RgbaImage};

pub const CHUNK_SIZE: Vec2 = Vec2::splat(1024.0);
pub const GRID_RES: (u32, u32) = (512, 512);
pub const HEIGHT_SCALE: f32 = 1000.0;

/// Startup system: load the full 16 K×16 K maps into CPU RAM only.
pub fn load_heightmap_data(
    mut commands: Commands,
) {
    // 1) Build absolute paths:
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let height_path = base.join("Heightmaps").join("New_World.hmp.png");
    let color_path  = base.join("Textures").   join("New_World_Texture.png");

    // 2) Decode into `image::GrayImage` / `RgbaImage` on CPU **without** the 512 MiB cap
    let mut height_reader = ImageReader::open(&height_path)
        .unwrap_or_else(|e| panic!("opening {:?}: {}", height_path, e));
    height_reader.no_limits();  // ← disable the default 512 MiB limit :contentReference[oaicite:0]{index=0}
    let height_image: GrayImage = height_reader
        .decode()
        .expect("decoding heightmap PNG")
        .into_luma8();

    let mut color_reader = ImageReader::open(&color_path)
        .unwrap_or_else(|e| panic!("opening {:?}: {}", color_path, e));
    color_reader.no_limits();   // ← likewise for your color map
    let color_image: RgbaImage = color_reader
        .decode()
        .expect("decoding color PNG")
        .into_rgba8();

    // 3) Gather metadata (no GPU here)
    let (w, h)     = (color_image.width(), color_image.height());
    let resolution = UVec2::new(w, h);
    let size       = Vec2::new(w as f32, h as f32);
    let origin     = Vec2::new(-size.x * 0.5, -size.y * 0.5);

    // 4) Insert the resource—and *only* the CPU images
    commands.insert_resource(HeightmapData {
        height_image,
        color_image,
        resolution,
        size,
        height_scale: HEIGHT_SCALE,
        origin,
    });
}

/// Builds a chunk’s mesh. UVs here go 0…1 across the chunk, matching the
/// texture tile you’ll upload in async_receive_chunks.
pub fn build_chunk_mesh(
    tile:          &GrayImage,    // your grayscale tile for heights
    (res_x, res_z): (u32, u32),   // grid resolution per-chunk
    chunk_origin:  Vec2,          // world‐space lower‐left corner of this chunk
    height_scale:  f32,
) -> Mesh {
    let verts_x = res_x + 1;
    let verts_z = res_z + 1;
    let dx = CHUNK_SIZE.x / res_x as f32;
    let dz = CHUNK_SIZE.y / res_z as f32;

    let max_px = tile.width().saturating_sub(1);
    let max_pz = tile.height().saturating_sub(1);

    // 1) Build vertex positions & LOCAL UVs
    let mut positions = Vec::with_capacity((verts_x * verts_z) as usize);
    let mut uvs       = Vec::with_capacity((verts_x * verts_z) as usize);
    for j in 0..=res_z {
        for i in 0..=res_x {
            // world‐space position
            let world_x = chunk_origin.x + i as f32 * dx;
            let world_z = chunk_origin.y + j as f32 * dz;

            // sample height from the GrayImage (bilinear if you like)
            let px = ((i as f32 / res_x as f32) * max_px as f32).round() as u32;
            let pz = ((j as f32 / res_z as f32) * max_pz as f32).round() as u32;
            let h  = tile.get_pixel(px, pz)[0] as f32 / 255.0 * height_scale;

            positions.push([world_x, h, world_z]);

            // ← LOCAL UVs from 0.0 to 1.0
            let u = i as f32 / res_x as f32;
            let v = 1.0 - (j as f32 / res_z as f32);
            uvs.push([u, v]);
        }
    }

    // 2) Build indices (two tris per quad)
    let mut indices = Vec::with_capacity((res_x * res_z * 6) as usize);
    for j in 0..res_z {
        for i in 0..res_x {
            let a = j * verts_x + i;
            let c = a + verts_x;
            indices.extend_from_slice(&[a, c, a + 1, a + 1, c, c + 1]);
        }
    }

    // 3) Assemble Mesh
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL,   vec![[0.0,1.0,0.0]; (verts_x*verts_z) as usize]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0,     uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}