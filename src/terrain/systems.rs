use bevy::prelude::*;
use bevy::render::mesh::{Mesh, Indices};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::math::{Vec2, UVec2};
use image::{GrayImage};
use crate::heightmap_data::HeightmapData;

pub const CHUNK_SIZE: Vec2 = Vec2::splat(512.0);
pub const GRID_RES: (u32, u32) = (512, 512);

/// 1) Load your master 16 384×8 192 heightmap into a resource.
pub fn load_heightmap_data(mut commands: Commands) {
    let dyn_image = image::open("assets/Heightmaps/World_Heightmap.hmp.png")
        .expect("World_Heightmap.hmp.png not found")
        .to_luma8();
    let resolution = UVec2::new(dyn_image.width(), dyn_image.height());
    let world_size = Vec2::new(16_384.0, 8_192.0);
    // center the map so (0,0) is in the middle
    let origin = -world_size * 0.5;

    commands.insert_resource(HeightmapData {
        image: dyn_image,
        resolution,
        size: world_size,
        height_scale: 300.0,
        origin,
    });
}

/// 2) Build a `Mesh` from a `GrayImage` tile, mapping the full tile resolution
///    onto a GRID_RES. This ensures we sample across the entire slice, not just
///    the first 128 pixels.
pub fn build_chunk_mesh(
    tile: &GrayImage,
    (res_x, res_z): (u32, u32),
    world_size: Vec2,
    height_scale: f32,
) -> Mesh {
    let verts_x = res_x + 1;
    let verts_z = res_z + 1;
    let dx = world_size.x / res_x as f32;
    let dz = world_size.y / res_z as f32;

    // precompute max pixel indices
    let max_px = tile.width().saturating_sub(1);
    let max_pz = tile.height().saturating_sub(1);

    // 1) Positions & UVs
    let mut positions = Vec::with_capacity((verts_x * verts_z) as usize);
    let mut uvs       = Vec::with_capacity((verts_x * verts_z) as usize);
    for j in 0..=res_z {
        let v = j as f32 / res_z as f32;
        let pz = (v * max_pz as f32).round() as u32;
        for i in 0..=res_x {
            let u = i as f32 / res_x as f32;
            let px = (u * max_px as f32).round() as u32;

            let x = i as f32 * dx - world_size.x * 0.5;
            let z = j as f32 * dz - world_size.y * 0.5;
            let h = tile.get_pixel(px, pz)[0] as f32 / 255.0 * height_scale;

            positions.push([x, h, z]);
            uvs.push([u, v]);
        }
    }

    // 2) Indices (two tris per quad)
    let mut indices = Vec::with_capacity((res_x * res_z * 6) as usize);
    for j in 0..res_z {
        for i in 0..res_x {
            let a = j * verts_x + i;
            let c = a + verts_x;
            indices.extend_from_slice(&[a, c, a + 1, a + 1, c, c + 1]);
        }
    }

    // 3) Assemble the mesh
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    let normals = vec![[0.0, 1.0, 0.0]; (verts_x * verts_z) as usize];
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}