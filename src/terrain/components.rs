// src/terrain/components.rs
use bevy::prelude::*;

/// Marker for all terrain-related entities (chunks, debug quads, etc.)
#[derive(Component)]
pub struct Terrain;

/// Identifies a specific terrain chunk by its lattice key.
#[derive(Component, Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ChunkKey {
    pub cx: i32,
    pub cz: i32,
}

impl ChunkKey {
    #[inline]
    pub const fn new(cx: i32, cz: i32) -> Self {
        Self { cx, cz }
    }
}

/// Optional: mark a chunk entity whose mesh is fully built and placed.
/// (Useful if you run post-processing systems over ready chunks.)
#[derive(Component)]
pub struct ChunkReady;

/// Optional: attach to a chunk if you need per-entity world-space AABB for culling/debug.
#[derive(Component, Copy, Clone, Debug)]
pub struct ChunkAabb {
    pub min: Vec2, // world XZ
    pub max: Vec2, // world XZ
}
