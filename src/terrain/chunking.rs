// src/terrain/chunking.rs
use bevy::math::{UVec2, Vec2, Vec3};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::heightmap_data::HeightmapData;
use crate::terrain::lod::LodLevel;

/// How many chunks around the focus center to keep
pub const CHUNK_RADIUS: i32 = 2;

/// Tracks what chunks are spawned and their entities (with LoD).
#[derive(Resource, Default)]
pub struct ChunkManager {
    /// Chunk entity and its LoD by (cx, cz)
    pub loaded: HashMap<(i32, i32), (Entity, LodLevel)>,
    /// Desired set with LoD chosen for each key, cleared each frame by scheduler
    pub desired: HashMap<(i32, i32), LodLevel>,
}

impl ChunkManager {
    pub fn new() -> Self {
        Self {
            loaded: HashMap::new(),
            desired: HashMap::new(),
        }
    }
}


/// Dimensions of the chunk lattice (count of chunks in X and Z).
#[derive(Debug, Clone, Copy)]
pub struct ChunkCounts {
    pub x: i32,
    pub z: i32,
}

/// Return how many chunks fit across the terrain in X and Z (ceil).
#[inline]
pub fn chunk_counts(data: &HeightmapData) -> ChunkCounts {
    let nx = (data.size.x / data.chunk_size.x).ceil() as i32;
    let nz = (data.size.y / data.chunk_size.y).ceil() as i32;
    ChunkCounts { x: nx.max(1), z: nz.max(1) }
}

/// Returns true if (cx,cz) is within the global terrain grid.
#[inline]
pub fn chunk_in_bounds(cx: i32, cz: i32, counts: ChunkCounts) -> bool {
    cx >= 0 && cz >= 0 && cx < counts.x && cz < counts.z
}

/// Convert a chunk index to the world-space origin (bottom-left corner) of that chunk.
#[inline]
pub fn chunk_origin_world(cx: i32, cz: i32, data: &HeightmapData) -> Vec2 {
    data.origin + Vec2::new(cx as f32 * data.chunk_size.x, cz as f32 * data.chunk_size.y)
}

/// Given a world position (x,z), compute the chunk indices and the local position inside the chunk.
/// Returns None if outside the global terrain.
pub fn world_to_chunk_local(world_xz: Vec2, data: &HeightmapData) -> Option<((i32, i32), Vec2)> {
    let lx = world_xz.x - data.origin.x;
    let lz = world_xz.y - data.origin.y;

    if lx < 0.0 || lz < 0.0 || lx >= data.size.x || lz >= data.size.y {
        return None;
    }

    let cx = (lx / data.chunk_size.x).floor() as i32;
    let cz = (lz / data.chunk_size.y).floor() as i32;

    let local_x = lx - (cx as f32 * data.chunk_size.x);
    let local_z = lz - (cz as f32 * data.chunk_size.y);

    Some(((cx, cz), Vec2::new(local_x, local_z)))
}

/// Map a world position (x,z) to normalized coordinates inside its chunk (u,v in [0,1]).
/// Returns None if outside the global terrain.
pub fn world_to_chunk_uv(world_xz: Vec2, data: &HeightmapData) -> Option<((i32, i32), Vec2)> {
    let (key, local) = world_to_chunk_local(world_xz, data)?;
    let u = (local.x / data.chunk_size.x).clamp(0.0, 1.0);
    let v = (local.y / data.chunk_size.y).clamp(0.0, 1.0);
    Some((key, Vec2::new(u, v)))
}

/// Clamp a world position into the legal terrain rectangle (no wrapping).
#[inline]
pub fn clamp_world_to_terrain(mut world_xz: Vec2, data: &HeightmapData) -> Vec2 {
    world_xz.x = world_xz
        .x
        .clamp(data.origin.x, data.origin.x + data.size.x - f32::EPSILON);
    world_xz.y = world_xz
        .y
        .clamp(data.origin.y, data.origin.y + data.size.y - f32::EPSILON);
    world_xz
}

/// Iterate all chunk keys that should be present around a center point (camera/hero).
/// - radius: how many rings around the center (use CHUNK_RADIUS)
/// - The set is clipped to terrain bounds (no negative or overflow indices).
pub fn needed_chunks_around(
    center_world: Vec3,
    data: &HeightmapData,
    radius: i32,
) -> impl Iterator<Item = (i32, i32)> {
    let counts = chunk_counts(data);

    // Find center chunk (if outside, clamp to border chunk)
    let mut center = Vec2::new(center_world.x, center_world.z);
    center = clamp_world_to_terrain(center, data);

    let ((ccx, ccz), _) = world_to_chunk_local(center, data)
        .expect("clamped center must lie inside terrain");

    let mut keys: Vec<(i32, i32)> = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let cx = ccx + dx;
            let cz = ccz + dz;
            if chunk_in_bounds(cx, cz, counts) {
                keys.push((cx, cz));
            }
        }
    }
    keys.into_iter()
}

/// Convenience: returns the AABB (min,max) world corners of a given chunk.
pub fn chunk_world_aabb(cx: i32, cz: i32, data: &HeightmapData) -> (Vec2, Vec2) {
    let min = chunk_origin_world(cx, cz, data);
    let max = min + data.chunk_size;
    (min, max)
}

/// Convert a chunk key and a normalized (u,v) in [0,1] to world (x,z).
#[inline]
pub fn chunk_uv_to_world(cx: i32, cz: i32, uv: Vec2, data: &HeightmapData) -> Vec2 {
    let origin = chunk_origin_world(cx, cz, data);
    origin + Vec2::new(uv.x * data.chunk_size.x, uv.y * data.chunk_size.y)
}
