use bevy::prelude::*;
use crate::heightmap_data::{HeightmapData, HeightTileCache, sample_height};
use crate::terrain::chunking::{chunk_origin_world, world_to_chunk_local};

#[derive(Component, Copy, Clone, Eq, PartialEq, Hash, Debug)]   // ← add Component
pub struct ChunkCoords {
    pub x: i32,
    pub z: i32,
}

#[derive(Component, Copy, Clone, Debug, Default)]               // ← add Component
pub struct LocalOffset {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub fn world_to_chunk_and_local(
    world_x: f32,
    world_z: f32,
    data: &HeightmapData,
) -> Option<(ChunkCoords, LocalOffset)> {
    let world = Vec2::new(world_x, world_z);
    let ((cx, cz), local) = world_to_chunk_local(world, data)?;
    Some((
        ChunkCoords { x: cx, z: cz },
        LocalOffset { x: local.x, y: 0.0, z: local.y },
    ))
}

pub fn chunk_local_to_world(coords: ChunkCoords, local: LocalOffset, data: &HeightmapData) -> Vec2 {
    let origin = chunk_origin_world(coords.x, coords.z, data);
    origin + Vec2::new(local.x, local.z)
}

pub fn sample_height_in_chunk(
    coords: ChunkCoords,
    local: LocalOffset,
    data: &HeightmapData,
    cache: &mut HeightTileCache,
) -> Option<f32> {
    let w = chunk_local_to_world(coords, local, data);
    sample_height(w.x, w.y, data, cache)
}
