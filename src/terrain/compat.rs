use bevy::prelude::*;
use crate::heightmap_data::HeightmapData;
use crate::terrain::chunking::world_to_chunk_local;

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
