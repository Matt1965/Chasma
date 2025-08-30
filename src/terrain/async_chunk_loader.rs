// src/terrain/async_chunk_loader.rs
use bevy::prelude::*;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool, Task};

use crate::heightmap_data::{HeightTileCache, HeightmapData, Tile16};
use crate::props::core::{ChunkArea, ChunkCoord};
use crate::props::plugin::TerrainChunkLoaded;
use crate::setup::MainCamera;
use crate::terrain::chunking::{
    chunk_origin_world, chunk_world_aabb, needed_chunks_around, ChunkManager,
};
use crate::terrain::components::{ChunkAabb, ChunkKey, ChunkReady, Terrain};
use crate::terrain::lod::{ChunkLod, LodLevel};
use crate::terrain::plugin::{COLOR_EXT, COLOR_FOLDER, COLOR_PREFIX};

/// How many mesh builds we allow *in flight* to complete and be accepted per frame.
#[derive(Resource)]
pub struct IntegrationBudget(pub usize);
impl Default for IntegrationBudget {
    fn default() -> Self {
        Self(1)
    }
}

/// Optional: how many new async jobs we start per frame (if you want to throttle creation, too).
#[derive(Resource)]
pub struct MeshBuildBudget(pub usize);
impl Default for MeshBuildBudget {
    fn default() -> Self {
        Self(4)
    }
}

/// Chunk task metadata (which chunk & which LoD this task is for)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ChunkTaskInfo {
    cx: i32,
    cz: i32,
    lod: LodLevel,
}

/// Tracks async work and finished-but-not-integrated meshes.
#[derive(Resource, Default)]
pub struct AsyncChunkLoader {
    /// Active tasks: (info, task)
    pub tasks: Vec<(ChunkTaskInfo, Task<Mesh>)>,
    /// Finished meshes waiting to be integrated into the world
    pub pending: Vec<(ChunkTaskInfo, Mesh)>,
}

/// Decide desired chunks/LoD, despawn mismatches, and spawn async jobs for missing pieces.
pub fn async_schedule_chunks(
    mut commands: Commands,
    mut loader: ResMut<AsyncChunkLoader>,
    mut chunk_mgr: ResMut<ChunkManager>,
    cam_q: Query<&Transform, With<MainCamera>>,
    data: Res<HeightmapData>,
    mut cache: ResMut<HeightTileCache>,
    build_budget: Option<Res<MeshBuildBudget>>,
) {
    let Ok(cam_tf) = cam_q.single() else { return };

    // 1) Compute desired set with LoD
    chunk_mgr.desired.clear();
    for (cx, cz) in needed_chunks_around(
        cam_tf.translation,
        &data,
        crate::terrain::chunking::CHUNK_RADIUS,
    ) {
        let origin = chunk_origin_world(cx, cz, &data);
        let center = Vec3::new(
            origin.x + data.chunk_size.x * 0.5,
            0.0,
            origin.y + data.chunk_size.y * 0.5,
        );
        let dist = cam_tf.translation.truncate().distance(center.truncate());
        let lod = LodLevel::pick(dist);
        chunk_mgr.desired.insert((cx, cz), lod);
    }

    // 2) Despawn loaded chunks that are no longer desired or have wrong LoD.
    //    Also drop any queued/finished work for those chunks.
    let loaded_keys: Vec<(i32, i32)> = chunk_mgr.loaded.keys().copied().collect();
    for key in loaded_keys {
        let loaded_lod = chunk_mgr.loaded.get(&key).map(|(_, l)| *l);
        let desired_lod = chunk_mgr.desired.get(&key).copied();

        let should_remove = match (loaded_lod, desired_lod) {
            (Some(ll), Some(dl)) => ll != dl,
            (Some(_), None) => true,
            _ => false,
        };

        if should_remove {
            if let Some((ent, _)) = chunk_mgr.loaded.remove(&key) {
                commands.entity(ent).despawn();
            }
            // Drop outstanding tasks/pending for this (cx,cz)
            loader.tasks.retain(|(info, _)| !(info.cx == key.0 && info.cz == key.1));
            loader.pending.retain(|(info, _)| !(info.cx == key.0 && info.cz == key.1));
        }
    }

    // 3) Launch async jobs for desired-but-missing chunks/LoD (respect a creation budget if provided)
    let mut started_this_frame = 0usize;
    let start_budget = build_budget.map(|b| b.0).unwrap_or(usize::MAX);

    'launch: for (&(cx, cz), &lod) in &chunk_mgr.desired {
        // Already loaded at correct LoD?
        if let Some((_, current_lod)) = chunk_mgr.loaded.get(&(cx, cz)) {
            if *current_lod == lod {
                continue;
            }
        }

        // Already queued (tasks or pending) for this (cx,cz,lod)?
        let already_queued =
            loader.tasks.iter().any(|(i, _)| i.cx == cx && i.cz == cz && i.lod == lod)
            || loader.pending.iter().any(|(i, _)| i.cx == cx && i.cz == cz && i.lod == lod);
        if already_queued {
            continue;
        }

        if started_this_frame >= start_budget {
            break 'launch;
        }

        // Snapshot the tiles required for this chunk
        let cur = cache.fetch_tile(cx, cz);
        let right = cache.fetch_tile(cx + 1, cz);
        let up = cache.fetch_tile(cx, cz + 1);
        let up_right = cache.fetch_tile(cx + 1, cz + 1);
        let Some(cur) = cur else { continue };

        let data_c = data.clone();
        let grid = lod.grid_res();

        let future = async move {
            build_chunk_mesh_from_tiles(cx, cz, grid, &data_c, cur, right, up, up_right)
                .unwrap_or_else(|| debug_fallback_quad(cx, cz, &data_c))
        };

        let task = AsyncComputeTaskPool::get().spawn(future);
        loader.tasks.push((ChunkTaskInfo { cx, cz, lod }, task));
        started_this_frame += 1;
    }

    // 4) Poll active tasks; move finished ones into pending
    let mut i = 0usize;
    while i < loader.tasks.len() {
        let (info, task) = &mut loader.tasks[i];
        if let Some(mesh) = check_ready(task) {
            let info_c = *info;
            loader.pending.push((info_c, mesh));
            loader.tasks.swap_remove(i);
            continue; // don't advance i when we removed an element
        }
        i += 1;
    }
}

/// Integrate up to IntegrationBudget finished meshes per frame and emit TerrainChunkLoaded.
pub fn async_receive_chunks(
    mut commands: Commands,
    mut loader: ResMut<AsyncChunkLoader>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_mgr: ResMut<ChunkManager>,
    data: Res<HeightmapData>,
    asset_server: Res<AssetServer>,
    mut evw_chunks_loaded: EventWriter<TerrainChunkLoaded>,
    integ_budget: Res<IntegrationBudget>,
) {
    let mut integrated = 0usize;
    let mut i = 0usize;
    while i < loader.pending.len() && integrated < integ_budget.0 {
        let (info, mesh) = loader.pending.remove(i);
        integrated += 1;

        // Material + mesh
        let mesh_handle = meshes.add(mesh);
        let color_path = format!(
            "{}/{}_y{}_x{}{}",
            COLOR_FOLDER, COLOR_PREFIX, info.cz, info.cx, COLOR_EXT
        );
        let tex: Handle<Image> = asset_server.load(color_path);
        let mat = materials.add(StandardMaterial {
            base_color_texture: Some(tex),
            base_color: Color::WHITE,
            perceptual_roughness: 1.0,
            metallic: 0.0,
            ..default()
        });

        // World placement / AABB
        let (min_w, max_w) = chunk_world_aabb(info.cx, info.cz, &data);
        let origin = chunk_origin_world(info.cx, info.cz, &data);

        // Spawn terrain chunk
        let e = commands
            .spawn((
                Terrain,
                ChunkKey::new(info.cx, info.cz),
                ChunkReady,
                ChunkAabb { min: min_w, max: max_w },
                ChunkLod(info.lod),
                Transform::default(),
                Visibility::Visible,
                bevy::render::mesh::Mesh3d(mesh_handle),
                bevy::pbr::MeshMaterial3d(mat.clone()),
                Name::new(format!(
                    "Chunk ({},{}) @ ({:.1},{:.1})",
                    info.cx, info.cz, origin.x, origin.y
                )),
            ))
            .id();

        // Track in manager
        chunk_mgr.loaded.insert((info.cx, info.cz), (e, info.lod));

        // Notify props/vegetation
        let min_x = data.origin.x + (info.cx as f32) * data.chunk_size.x;
        let min_z = data.origin.y + (info.cz as f32) * data.chunk_size.y;
        let max_x = min_x + data.chunk_size.x;
        let max_z = min_z + data.chunk_size.y;

        evw_chunks_loaded.send(TerrainChunkLoaded(ChunkArea {
            coord: ChunkCoord { x: info.cx, z: info.cz },
            min_xz: Vec2::new(min_x, min_z),
            max_xz: Vec2::new(max_x, max_z),
        }));
    }
}

// ---------- Mesh building helpers (with `grid_res`) ----------

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
    let width = max_w.x - min_w.x;
    let depth = max_w.y - min_w.y;

    // RAW16 normalization
    let (rmin, rmax) = data.raw_minmax;
    let inv_span = if rmax > rmin { 1.0 / (rmax - rmin) } else { 0.0 };

    // tile extents
    let tx_max = (cur.res.x.saturating_sub(1)) as i32;
    let tz_max = (cur.res.y.saturating_sub(1)) as i32;

    // Canonical RAW sampler (seamless edges)
    let sample_raw = |u: f32, v: f32, i: usize, j: usize| -> f32 {
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        let right_edge = i == nx - 1;
        let top_edge = j == nz - 1;

        if right_edge && top_edge {
            if let Some(t) = &up_right {
                return t.get_clamped(0, 0) as f32;
            }
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

    // Height at (u, v) -> world Y
    let sample_h = |u: f32, v: f32, i: usize, j: usize| -> f32 {
        let raw = sample_raw(u, v, i, j);
        let norm = ((raw - rmin) * inv_span).clamp(0.0, 1.0);
        norm * data.height_scale
    };

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(nx * nz);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(nx * nz);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(nx * nz);

    // Finite-difference step in UV space
    let du = if nx > 1 { 1.0 / (nx as f32 - 1.0) } else { 1.0 };
    let dv = if nz > 1 { 1.0 / (nz as f32 - 1.0) } else { 1.0 };

    for j in 0..nz {
        for i in 0..nx {
            let u = i as f32 / (nx as f32 - 1.0);
            let v = j as f32 / (nz as f32 - 1.0);

            let wx = min_w.x + u * width;
            let wz = min_w.y + v * depth;
            let h = sample_h(u, v, i, j);

            let hl = sample_h((u - du).max(0.0), v, i.saturating_sub(1), j);
            let hr = sample_h((u + du).min(1.0), v, (i + 1).min(nx - 1), j);
            let hd = sample_h(u, (v - dv).max(0.0), i, j.saturating_sub(1));
            let hu = sample_h(u, (v + dv).min(1.0), i, (j + 1).min(nz - 1));

            // Convert to world-space slope (du->width, dv->depth)
            let dpx = (hr - hl) / (2.0 * du * width.max(f32::EPSILON));
            let dpz = (hu - hd) / (2.0 * dv * depth.max(f32::EPSILON));

            positions.push([wx, h, wz]);
            uvs.push([u, v]);

            let n = Vec3::new(-dpx, 1.0, -dpz).normalize_or_zero();
            normals.push([n.x, n.y, n.z]);
        }
    }

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

fn debug_fallback_quad(cx: i32, cz: i32, data: &HeightmapData) -> Mesh {
    let (min_w, max_w) = chunk_world_aabb(cx, cz, data);
    let width = max_w.x - min_w.x;
    let depth = max_w.y - min_w.y;

    let positions = vec![
        [min_w.x, 0.0, min_w.y],
        [min_w.x + width, 0.0, min_w.y],
        [min_w.x, 0.0, min_w.y + depth],
        [min_w.x + width, 0.0, min_w.y + depth],
    ];
    let uvs = vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
    let normals = vec![[0.0, 1.0, 0.0]; 4];
    let indices = vec![0, 2, 1, 1, 2, 3];

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
