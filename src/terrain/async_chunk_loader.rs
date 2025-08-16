// src/terrain/async_chunk_loader.rs
use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool, Task};

use crate::heightmap_data::{HeightTileCache, HeightmapData, Tile16};
use crate::terrain::chunking::{
    chunk_origin_world, chunk_world_aabb, needed_chunks_around, ChunkManager, CHUNK_RADIUS,
};
use crate::terrain::components::{ChunkAabb, ChunkKey, ChunkReady, Terrain};
use crate::terrain::plugin::{COLOR_FOLDER, COLOR_PREFIX, COLOR_EXT};
use crate::setup::MainCamera;

// ---------- Resource to track async work ----------
#[derive(Resource, Default)]
pub struct AsyncChunkLoader {
    pub tasks: HashMap<(i32, i32), Task<Mesh>>,
}

// ---------- Systems ----------

/// Schedule: figure out which chunks we need around the camera, despawn extras,
/// and spawn async tasks for the missing ones (passing tile snapshots into tasks).
pub fn async_schedule_chunks(
    mut commands: Commands,
    mut loader: ResMut<AsyncChunkLoader>,
    mut chunk_mgr: ResMut<ChunkManager>,
    cam_q: Query<&Transform, With<MainCamera>>,
    data: Res<HeightmapData>,
    mut cache: ResMut<HeightTileCache>,
    asset_server: Res<AssetServer>,   
) {
    let Ok(cam_tf) = cam_q.single() else { return };

    // Desired set around camera
    chunk_mgr.desired.clear();
    for key in needed_chunks_around(cam_tf.translation, &data, CHUNK_RADIUS) {
        chunk_mgr.desired.insert(key);
    }

    // Despawn no-longer-desired chunks + cancel pending tasks
    let loaded_keys: Vec<(i32, i32)> = chunk_mgr.loaded.keys().copied().collect();
    for key in loaded_keys {
        if !chunk_mgr.desired.contains(&key) {
            if let Some(entity) = chunk_mgr.loaded.remove(&key) {
                commands.entity(entity).despawn();
            }
            loader.tasks.remove(&key);
        }
    }

    // Spawn tasks for desired-but-missing chunks
    let grid = chunk_mgr.grid_res;

    for &(cx, cz) in &chunk_mgr.desired {
        if chunk_mgr.loaded.contains_key(&(cx, cz)) || loader.tasks.contains_key(&(cx, cz)) {
            continue;
        }

        // Snapshot the *exact* tile this chunk needs (Arc clones the buffer, cheap).
        let cur      = cache.fetch_tile(cx, cz);
        let right    = cache.fetch_tile(cx + 1, cz);
        let up       = cache.fetch_tile(cx, cz + 1);
        let up_right = cache.fetch_tile(cx + 1, cz + 1);

        // need at least the current tile
        let Some(cur) = cur else { continue; };

        let data_c = data.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            build_chunk_mesh_from_tiles(cx, cz, grid, &data_c, cur, right, up, up_right)
                .unwrap_or_else(|| debug_fallback_quad(cx, cz, &data_c))
        });
        loader.tasks.insert((cx, cz), task);
    }
}

/// Receive finished meshes, spawn chunk entities with components (no Bundles).
pub fn async_receive_chunks(
    mut commands: Commands,
    mut loader: ResMut<AsyncChunkLoader>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_mgr: ResMut<ChunkManager>,
    data: Res<HeightmapData>,
    asset_server: Res<AssetServer>, // needed for textures
) {
    // Drain finished tasks
    let mut finished: Vec<((i32, i32), Mesh)> = Vec::new();
    loader.tasks.retain(|&(cx, cz), task| match check_ready(task) {
        None => true,
        Some(mesh) => { finished.push(((cx, cz), mesh)); false }
    });

    // Solid fallback in case texture isn’t ready yet (meshes still render)
    let fallback = materials.add(StandardMaterial {
        base_color: Color::linear_rgb(0.72, 0.75, 0.72),
        perceptual_roughness: 0.95,
        metallic: 0.0,
        ..default()
    });

    for ((cx, cz), mesh) in finished {
        let mesh_handle = meshes.add(mesh);

        // Build Bevy-relative path: NO "assets/" prefix
        // Your on-disk file: assets/Textures/Texture_y{cz}_x{cx}.png
        let color_path = format!("{}/{}_y{}_x{}{}", COLOR_FOLDER, COLOR_PREFIX, cz, cx, COLOR_EXT);
        let color_tex: Handle<Image> = asset_server.load(color_path);

        // Per-chunk material with the texture; if texture not loaded yet, Bevy shows fallback color
        let mat = materials.add(StandardMaterial {
            base_color_texture: Some(color_tex),
            base_color: Color::WHITE,
            perceptual_roughness: 1.0,
            metallic: 0.0,
            ..default()
        });

        // World placement data (optional debug name)
        let (min_w, max_w) = chunk_world_aabb(cx, cz, &data);
        let origin = chunk_origin_world(cx, cz, &data);

        let e = commands
            .spawn((
                Terrain,
                ChunkKey::new(cx, cz),
                ChunkReady,
                ChunkAabb { min: min_w, max: max_w },
                Transform::default(),
                Visibility::Visible,
                bevy::render::mesh::Mesh3d(mesh_handle),
                bevy::pbr::MeshMaterial3d(mat.clone()),
                Name::new(format!("Chunk ({cx},{cz}) @ ({:.1},{:.1})", origin.x, origin.y)),
            ))
            .id();

        chunk_mgr.loaded.insert((cx, cz), e);
    }
}

// ---------- Mesh building helpers ----------

/// Build one chunk mesh from a *single* RAW16 tile snapshot (Arc-backed).
/// Assumes 1 tile == 1 chunk in world span.
/// Bilinear samples RAW16 -> normalized -> world Y using HeightmapData.height_scale.
/// Smooth normals via central differences in world space.
fn build_chunk_mesh_from_tiles(
    cx: i32,
    cz: i32,
    grid_res: UVec2,
    data: &HeightmapData,
    cur: Tile16,
    right: Option<Tile16>,
    up: Option<Tile16>,
    up_right: Option<Tile16>,
) -> Option<Mesh> {
    let nx = grid_res.x.max(2) as usize;
    let nz = grid_res.y.max(2) as usize;

    let (min_w, max_w) = chunk_world_aabb(cx, cz, data);
    let width  = max_w.x - min_w.x;
    let depth  = max_w.y - min_w.y;
    let step_x = width  / (nx as f32 - 1.0);
    let step_z = depth  / (nz as f32 - 1.0);

    // RAW16 normalization
    let (rmin, rmax) = data.raw_minmax;
    let inv_span = if rmax > rmin { 1.0 / (rmax - rmin) } else { 0.0 };

    // extents in tile space
    let tx_max = (cur.res.x.saturating_sub(1)) as i32;
    let tz_max = (cur.res.y.saturating_sub(1)) as i32;

    // Canonical sampler: on right/top edge, use neighbor’s first col/row
    let sample_raw = |u: f32, v: f32, i: usize, j: usize| -> f32 {
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        let right_edge = i == nx - 1;
        let top_edge   = j == nz - 1;

        if right_edge && top_edge {
            if let Some(t) = &up_right { return t.get_clamped(0, 0) as f32; }
        } else if right_edge {
            if let Some(t) = &right {
                let pz = (v * tz_max as f32).round() as i32;
                return t.get_clamped(0, pz) as f32;
            }
        } else if top_edge {
            if let Some(t) = &up {
                let px = (u * tx_max as f32).round() as i32;
                return t.get_clamped(px, 0) as f32;
            }
        }

        let px_f = u * tx_max as f32;
        let pz_f = v * tz_max as f32;
        let x0 = px_f.floor() as i32;
        let z0 = pz_f.floor() as i32;
        let x1 = (x0 + 1).min(tx_max);
        let z1 = (z0 + 1).min(tz_max);
        let dx = px_f - x0 as f32;
        let dz = pz_f - z0 as f32;

        let s00 = cur.get_clamped(x0, z0) as f32;
        let s10 = cur.get_clamped(x1, z0) as f32;
        let s01 = cur.get_clamped(x0, z1) as f32;
        let s11 = cur.get_clamped(x1, z1) as f32;

        let a = s00 * (1.0 - dx) + s10 * dx;
        let b = s01 * (1.0 - dx) + s11 * dx;
        a * (1.0 - dz) + b * dz
    };

    // Height at (u,v) -> world Y
    let sample_h = |u: f32, v: f32, i: usize, j: usize| -> f32 {
        let raw = sample_raw(u, v, i, j);
        let norm = ((raw - rmin) * inv_span).clamp(0.0, 1.0);
        norm * data.height_scale
    };

    // Build mesh
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(nx * nz);
    let mut normals:   Vec<[f32; 3]> = Vec::with_capacity(nx * nz);
    let mut uvs:       Vec<[f32; 2]> = Vec::with_capacity(nx * nz);

    // Finite diff step in UV (exactly one vertex over)
    let du = if nx > 1 { 1.0 / (nx as f32 - 1.0) } else { 1.0 };
    let dv = if nz > 1 { 1.0 / (nz as f32 - 1.0) } else { 1.0 };

    for j in 0..nz {
        for i in 0..nx {
            let u = i as f32 / (nx as f32 - 1.0);
            let v = j as f32 / (nz as f32 - 1.0);

            // World XZ at this vertex
            let wx = min_w.x + u * width;
            let wz = min_w.y + v * depth;

            // Height at center
            let h  = sample_h(u, v, i, j);

            // Heights for gradient — use canonical sampler, which crosses tiles at edges
            let hl = sample_h((u - du).max(0.0), v, i.saturating_sub(1), j);
            let hr = sample_h((u + du).min(1.0), v, (i + 1).min(nx - 1), j);
            let hd = sample_h(u, (v - dv).max(0.0), i, j.saturating_sub(1));
            let hu = sample_h(u, (v + dv).min(1.0), i, (j + 1).min(nz - 1));

            // Convert dH/du, dH/dv to world-space slopes: du→width, dv→depth
            let dpx = (hr - hl) / (2.0 * du * width.max(f32::EPSILON));
            let dpz = (hu - hd) / (2.0 * dv * depth.max(f32::EPSILON));

            // Build attributes
            positions.push([wx, h, wz]);
            uvs.push([u, v]);

            // dP/du = (width, dH, 0) normalized by width; we just need a normal vector ~ (-dH/dx, 1, -dH/dz)
            let n = Vec3::new(-dpx, 1.0, -dpz).normalize_or_zero();
            normals.push([n.x, n.y, n.z]);
        }
    }

    // Indices
    let mut indices: Vec<u32> = Vec::with_capacity((nx - 1) * (nz - 1) * 6);
    for j in 0..(nz - 1) {
        for i in 0..(nx - 1) {
            let i0 = (j * nx + i) as u32;
            let i1 = (j * nx + i + 1) as u32;
            let i2 = ((j + 1) * nx + i) as u32;
            let i3 = ((j + 1) * nx + i + 1) as u32;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}



/// If sampling failed (e.g., missing tile), spawn a flat debug quad.
fn debug_fallback_quad(cx: i32, cz: i32, data: &HeightmapData) -> Mesh {
    let (min_w, max_w) = chunk_world_aabb(cx, cz, data);
    let sx = max_w.x - min_w.x;
    let sz = max_w.y - min_w.y;

    let positions = vec![
        [min_w.x, 0.0, min_w.y],
        [min_w.x + sx, 0.0, min_w.y],
        [min_w.x, 0.0, min_w.y + sz],
        [min_w.x + sx, 0.0, min_w.y + sz],
    ];
    let normals = vec![[0.0, 1.0, 0.0]; 4];
    let uvs = vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
    let indices = vec![0u32, 2, 1, 1, 2, 3];

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
