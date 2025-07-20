use bevy::prelude::*;
use crate::terrain::systems::load_heightmap_data;
use crate::terrain::chunking::{
    preload_terrain_assets,
    ChunkManager,
    chunk_streaming_system,
    wrap_chunks,
    apply_chunked_transform,
};

/// Plugin that loads your heightmap, preloads assets,
/// then streams & rebases terrain chunks around the camera.
pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app
            // 1) Load the HeightmapData resource first
            .add_systems(Startup, load_heightmap_data)
            // 2) Preload mesh & material handles once (after heightmap is ready)
            .add_systems(Startup, preload_terrain_assets.after(load_heightmap_data))
            // 3) Initialize our chunk‐tracker
            .init_resource::<ChunkManager>()
            // 4) On Update, spawn/despawn chunks *only* when the camera crosses a chunk boundary
            .add_systems(Update, chunk_streaming_system.after(preload_terrain_assets))
            // 5) Wrap any chunked entity that drifted outside its chunk back into chunk‐coords
            .add_systems(Update, wrap_chunks.after(chunk_streaming_system))
            // 6) Finally, rebase those chunked entities into the camera’s local frame
            .add_systems(Update, apply_chunked_transform.after(wrap_chunks));
    }
}
