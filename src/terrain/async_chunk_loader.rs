// src/terrain/async_chunk_loader.rs

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use image::{GrayImage, imageops::crop_imm};

use crate::terrain::components::Terrain;
use crate::terrain::chunking::{ChunkManager, TerrainAssets, CHUNK_RADIUS};
use crate::terrain::systems::{build_chunk_mesh, CHUNK_SIZE, GRID_RES};
use crate::heightmap_data::HeightmapData;
use crate::setup::MainCamera;

/// Tracks all currently in‐flight mesh builds.
#[derive(Resource, Default)]
pub struct AsyncChunkLoader {
    /// Maps chunk coords → the mesh‐building Task
    pub tasks: HashMap<(i32, i32), Task<Mesh>>,
}

/// 1) On Update: schedule any newly needed chunks as background tasks.
pub fn async_schedule_chunks(
    mut loader: ResMut<AsyncChunkLoader>,
    mut manager: ResMut<ChunkManager>,
    heightmap: Res<HeightmapData>,
    cam_q: Query<&Transform, With<MainCamera>>,
) {
    // figure out which 512×512 chunk the camera is in
    let tf = if let Ok(tf) = cam_q.single() { tf } else { return; };
    let lx = tf.translation.x - heightmap.origin.x;
    let lz = tf.translation.z - heightmap.origin.y;
    let cam_chunk = (
        (lx / CHUNK_SIZE.x).floor() as i32,
        (lz / CHUNK_SIZE.y).floor() as i32,
    );

    // only re-schedule when we cross into a new chunk
    if manager.last_cam_chunk == Some(cam_chunk) {
        return;
    }
    manager.last_cam_chunk = Some(cam_chunk);

    // build the set of chunk‐coords we now want around the camera
    let mut want = Vec::new();
    for dx in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for dz in -CHUNK_RADIUS..=CHUNK_RADIUS {
            want.push((cam_chunk.0 + dx, cam_chunk.1 + dz));
        }
    }

    let pool = AsyncComputeTaskPool::get();

    for coord in want {
        // skip if already loaded or already in-flight
        if manager.loaded.contains_key(&coord)
            || loader.tasks.contains_key(&coord)
        {
            continue;
        }

        // compute this chunk’s world‐min corner
        let origin = Vec2::new(
            heightmap.origin.x + coord.0 as f32 * CHUNK_SIZE.x,
            heightmap.origin.y + coord.1 as f32 * CHUNK_SIZE.y,
        );
        let map_origin   = heightmap.origin;
        let image        = heightmap.image.clone();
        let resolution   = heightmap.resolution;
        let size         = heightmap.size;
        let height_scale = heightmap.height_scale;

        // spawn the mesh‐build on the compute pool
        let task: Task<Mesh> = pool.spawn(async move {
            // 1) compute pixel‐to‐world scale
            let px_u_x = resolution.x as f32 / size.x;
            let px_u_z = resolution.y as f32 / size.y;
            // 2) how many pixels this 512×512‐unit chunk covers
            let crop_w = (CHUNK_SIZE.x * px_u_x).round() as u32;
            let crop_h = (CHUNK_SIZE.y * px_u_z).round() as u32;
            // 3) map world corner → pixel coords in master image
            let raw_x = (origin.x - map_origin.x) * px_u_x;
            let raw_z = (origin.y - map_origin.y) * px_u_z;
            let px = raw_x.clamp(0.0, (resolution.x - crop_w) as f32) as u32;
            let pz = raw_z.clamp(0.0, (resolution.y - crop_h) as f32) as u32;
            // 4) crop & build the mesh
            let tile: GrayImage = crop_imm(&image, px, pz, crop_w, crop_h)
                .to_image();
            build_chunk_mesh(&tile, GRID_RES, CHUNK_SIZE, height_scale)
        });

        // track it until completion
        loader.tasks.insert(coord, task);
    }
}

/// 2) On Update: poll all in‐flight tasks, and as each finishes,
///    upload its mesh, spawn the chunk Entity, and mark it “loaded.”
pub fn async_receive_chunks(
    mut loader:   ResMut<AsyncChunkLoader>,
    mut commands: Commands,
    mut meshes:   ResMut<Assets<Mesh>>,
    assets:       Res<TerrainAssets>,
    mut manager:  ResMut<ChunkManager>,
    heightmap:    Res<HeightmapData>,
) {
    let mut completed = Vec::new();

    for (&coord, task) in loader.tasks.iter_mut() {
        // poll_once returns `Some(mesh)` if the task finished this frame
        if let Some(mesh) = block_on(poll_once(task)) {
            // upload to Bevy’s Assets to get a Handle<Mesh>
            let mesh_handle = meshes.add(mesh);

            // compute spawn position
            let half = CHUNK_SIZE * 0.5;
            let origin = Vec2::new(
                heightmap.origin.x + coord.0 as f32 * CHUNK_SIZE.x,
                heightmap.origin.y + coord.1 as f32 * CHUNK_SIZE.y,
            );
            let translation = Vec3::new(
                origin.x + half.x,
                0.0,
                origin.y + half.y,
            );

            // spawn your Terrain bundle
            let ent = commands.spawn((
                Terrain,
                Transform::from_translation(translation),
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                ViewVisibility::default(),
                Mesh3d(mesh_handle),
                MeshMaterial3d(assets.material.clone()),
            ))
            .id();

            // record it as loaded
            manager.loaded.insert(coord, ent);
            completed.push(coord);
        }
    }

    // forget all tasks that are now done
    for coord in completed {
        loader.tasks.remove(&coord);
    }
}
