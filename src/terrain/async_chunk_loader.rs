// src/terrain/async_chunk_loader.rs

use std::collections::HashMap;
use bevy::log::info;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::tasks::futures::check_ready;
use bevy::render::mesh::{Mesh, Mesh3d};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::image::Image;

use image::{GrayImage, RgbaImage, imageops::crop_imm};

use crate::terrain::chunking::{ChunkManager, CHUNK_RADIUS};
use crate::terrain::systems::{CHUNK_SIZE, GRID_RES, build_chunk_mesh};
use crate::terrain::components::Terrain;
use crate::heightmap_data::HeightmapData;
use crate::setup::MainCamera;

/// Tracks all in-flight mesh-build tasks.
#[derive(Resource, Default)]
pub struct AsyncChunkLoader {
    pub tasks: HashMap<(i32, i32), Task<Mesh>>,
}

/// Schedules new mesh-build tasks when the camera crosses chunk boundaries.
/// Completely bounded, zero‐based chunk indices.
pub fn async_schedule_chunks(
    mut loader: ResMut<AsyncChunkLoader>,
    mut manager: ResMut<ChunkManager>,
    heightmap: Res<HeightmapData>,
    cam_q: Query<&Transform, With<MainCamera>>,
) {
    // 1) Get camera in world → chunk coords
    let tf = if let Ok(tf) = cam_q.single() { tf } else { return; };
    let lx = tf.translation.x - heightmap.origin.x;
    let lz = tf.translation.z - heightmap.origin.y;
    let cam_chunk_x = (lx / CHUNK_SIZE.x).floor() as i32;
    let cam_chunk_z = (lz / CHUNK_SIZE.y).floor() as i32;
    let cam_chunk = (cam_chunk_x, cam_chunk_z);

    // 2) Only when crossing boundary
    if manager.last_cam_chunk == Some(cam_chunk) {
        return;
    }
    manager.last_cam_chunk = Some(cam_chunk);

    // 3) Compute 0-based chunk-count & bounds
    let chunks_x = (heightmap.size.x / CHUNK_SIZE.x).round() as i32;
    let chunks_z = (heightmap.size.y / CHUNK_SIZE.y).round() as i32;
    let min_x = 0;
    let max_x = chunks_x - 1;
    let min_z = 0;
    let max_z = chunks_z - 1;

    // 4) Gather neighbors, clamp into [0..max]
    let mut want = Vec::new();
    for dx in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for dz in -CHUNK_RADIUS..=CHUNK_RADIUS {
            let raw_nx = cam_chunk_x + dx;
            let raw_nz = cam_chunk_z + dz;
            let nx = raw_nx.clamp(min_x, max_x);
            let nz = raw_nz.clamp(min_z, max_z);
            info!(
                "SCHED: raw_chunk=({},{}) → clamped_chunk=({},{})",
                raw_nx, raw_nz, nx, nz
            );
            want.push((nx, nz));
        }
    }
    want.sort_by_key(|&(x, z)| {
        let dx = x - cam_chunk_x;
        let dz = z - cam_chunk_z;
        dx*dx + dz*dz
    });

    // 5) Spawn tasks for any missing chunk
    let pool = AsyncComputeTaskPool::get();
    for coord @ (cx, cz) in want {
        if manager.loaded.contains_key(&coord) || loader.tasks.contains_key(&coord) {
            continue;
        }
        // Capture minimal data
        let height_image = heightmap.height_image.clone();
        let resolution   = heightmap.resolution;
        let size         = heightmap.size;
        let height_scale = heightmap.height_scale;

        let task: Task<Mesh> = pool.spawn(async move {
            // world→pixel scaling
            let px_u_x = resolution.x as f32 / size.x;
            let px_u_z = resolution.y as f32 / size.y;
            let crop_w = (CHUNK_SIZE.x * px_u_x).round() as u32;
            let crop_h = (CHUNK_SIZE.y * px_u_z).round() as u32;

            // compute raw pixel offset
            let raw_px = cx * crop_w as i32;
            let raw_pz = cz * crop_h as i32;

            // clamp within [0..max_offset]
            let max_px_off = (resolution.x as i32 - crop_w as i32).max(0);
            let max_pz_off = (resolution.y as i32 - crop_h as i32).max(0);
            let px = raw_px.clamp(0, max_px_off) as u32;
            let pz = raw_pz.clamp(0, max_pz_off) as u32;

            // crop & build
            let tile: GrayImage = crop_imm(&height_image, px, pz, crop_w, crop_h).to_image();
            build_chunk_mesh(
                &tile,
                GRID_RES,
                Vec2::ZERO,
                height_scale,
            )
        });

        loader.tasks.insert(coord, task);
    }
}

/// Receives completed mesh tasks and spawns the chunk entities.
pub fn async_receive_chunks(
    mut loader:    ResMut<AsyncChunkLoader>,
    mut commands:  Commands,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut textures:  ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap:     Res<HeightmapData>,
    mut manager:   ResMut<ChunkManager>,
) {
    let mut finished = Vec::new();

    for (&(cx, cz), task) in loader.tasks.iter_mut() {
        if let Some(chunk_mesh) = check_ready(task) {
            // 1) upload mesh
            let mesh_handle = meshes.add(chunk_mesh);

            // 2) compute same crop dims
            let px_u_x = heightmap.resolution.x as f32 / heightmap.size.x;
            let px_u_z = heightmap.resolution.y as f32 / heightmap.size.y;
            let crop_w = (CHUNK_SIZE.x * px_u_x).round() as u32;
            let crop_h = (CHUNK_SIZE.y * px_u_z).round() as u32;

            // 3) clamp the color‐map crop
            let raw_px = cx as i32 * crop_w as i32;
            let raw_pz = cz as i32 * crop_h as i32;
            let max_px_off = (heightmap.resolution.x as i32 - crop_w as i32).max(0);
            let max_pz_off = (heightmap.resolution.y as i32 - crop_h as i32).max(0);
            let px = raw_px.clamp(0, max_px_off) as u32;
            let pz = raw_pz.clamp(0, max_pz_off) as u32;
            let py = heightmap.resolution.y
                .saturating_sub(pz)
                .saturating_sub(crop_h);
            let tile_color: RgbaImage = crop_imm(
                &heightmap.color_image, px, py, crop_w, crop_h
            ).to_image();

            // 4) upload texture
            let mut bevy_img = Image::new(
                Extent3d {
                    width: crop_w,
                    height: crop_h,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                tile_color.into_raw(),
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            );
            bevy_img.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING;
            let image_handle = textures.add(bevy_img);

            // 5) place the chunk
            let world_pos = Vec3::new(
                heightmap.origin.x + cx as f32 * CHUNK_SIZE.x,
                0.0,
                heightmap.origin.y + cz as f32 * CHUNK_SIZE.y,
            );
            let mat_handle = materials.add(StandardMaterial {
                base_color_texture: Some(image_handle),
                perceptual_roughness: 1.0,
                ..Default::default()
            });
            let ent = commands.spawn((
                Terrain,
                Mesh3d(mesh_handle),
                MeshMaterial3d(mat_handle),
                Transform::from_translation(world_pos),
                GlobalTransform::default(),
            )).id();

            manager.loaded.insert((cx, cz), ent);
            finished.push((cx, cz));
        }
    }

    // 6) cleanup
    for coord in finished {
        loader.tasks.remove(&coord);
    }
}

/// Once per frame, despawn any chunk whose chunk‐coords are
/// more than `unload_radius` away from the camera’s current chunk.
pub fn cleanup_distant_chunks(
    mut commands: Commands,
    mut manager: ResMut<ChunkManager>,
    cam_q: Query<&Transform, With<MainCamera>>,
    heightmap: Res<HeightmapData>,
) {
    let tf = if let Ok(tf) = cam_q.single() { tf } else { return; };

    // Compute camera’s current chunk indices
    let cam_x = ((tf.translation.x - heightmap.origin.x) / CHUNK_SIZE.x)
        .floor() as i32;
    let cam_z = ((tf.translation.z - heightmap.origin.y) / CHUNK_SIZE.y)
        .floor() as i32;

    // Choose your unload radius (in chunks). Here, we use 1.5× the load radius.
    let unload_radius = (CHUNK_RADIUS * 3) / 2;

    // Collect any chunks that are too far
    let mut to_remove = Vec::new();
    for (&(cx, cz), &ent) in manager.loaded.iter() {
        let dx = (cx - cam_x).abs();
        let dz = (cz - cam_z).abs();

        if dx > unload_radius || dz > unload_radius {
            to_remove.push((cx, cz, ent));
        }
    }

    // Despawn & remove from manager
    for (cx, cz, ent) in to_remove {
        commands.entity(ent).despawn();
        manager.loaded.remove(&(cx, cz));
        info!("UNLOAD: chunk=({},{})", cx, cz);
    }
}


pub fn debug_spawn_corners(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap: Res<HeightmapData>,
) {
    // 1) Build a small cube mesh
    let cube_mesh     = Mesh::from(Cuboid::new(10.0, 1000.0, 10.0));
    let cube_handle: Handle<Mesh> = meshes.add(cube_mesh);

    // 2) Build a bright, unlit material (make it stand out!)
    let bright_material_handle = materials.add(StandardMaterial {
        base_color: Color::linear_rgb(1.0, 0.0, 0.0), // red
        unlit: true,
        ..Default::default()
    });

    // 3) Compute your four corners + center in world-space
    let p0 = Vec3::new(
        heightmap.origin.x,
        5.0,
        heightmap.origin.y,
    );
    let p1 = Vec3::new(
        heightmap.origin.x + heightmap.size.x,
        5.0,
        heightmap.origin.y,
    );
    let p2 = Vec3::new(
        heightmap.origin.x,
        5.0,
        heightmap.origin.y + heightmap.size.y,
    );
    let p3 = Vec3::new(
        heightmap.origin.x + heightmap.size.x,
        5.0,
        heightmap.origin.y + heightmap.size.y,
    );
    let center = Vec3::ZERO; // since origin is at map-center

    // 4) Spawn one cube at each position
    for &pos in &[p0, p1, p2, p3, center] {
        commands.spawn((
            Mesh3d(cube_handle.clone()),
            MeshMaterial3d(bright_material_handle.clone()),
            Transform::from_translation(pos),
            GlobalTransform::default(),
        ));
    }
}