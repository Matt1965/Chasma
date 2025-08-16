// src/terrain/systems.rs
use bevy::prelude::*;

use crate::heightmap_data::HeightmapData;
use crate::terrain::chunking::ChunkManager;

/// Vertex grid per chunk (X,Z). Use odd counts (e.g., 65) so the edge vertices line up.
pub const GRID_RES: UVec2 = UVec2::new(65, 65);

/// World-space width/length of one chunk (X,Z) in the same units as your scene (e.g., meters).
pub const CHUNK_SIZE: Vec2 = Vec2::new(256.0, 256.0);

/// Startup: create the ChunkManager with your target grid resolution,
/// and ensure `HeightmapData.chunk_size` matches the chosen CHUNK_SIZE.
pub fn init_terrain_params(mut commands: Commands, mut hmd: ResMut<HeightmapData>) {
    // Keep all systems in agreement about how big a chunk is in world units.
    hmd.chunk_size = CHUNK_SIZE;

    // Insert/replace the chunk manager with the vertex grid we want.
    commands.insert_resource(ChunkManager::new(GRID_RES));
}

/// Optional: if you ever want to change CHUNK_SIZE or GRID_RES at runtime,
/// call this to reapply the constants to the resources.
pub fn reapply_terrain_params(mut hmd: ResMut<HeightmapData>, mut cm: ResMut<ChunkManager>) {
    hmd.chunk_size = CHUNK_SIZE;
    cm.grid_res = GRID_RES;
}
