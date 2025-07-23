// src/terrain/plugin.rs

use bevy::prelude::*;
use crate::state::GameState;
use crate::terrain::systems::{load_heightmap_data, camera_grounding_system};
use crate::terrain::chunking::{preload_terrain_assets, ChunkManager};
use crate::terrain::async_chunk_loader::{
    AsyncChunkLoader,
    async_schedule_chunks,
    async_receive_chunks,
};

/// Plugin that loads your heightmap, preloads material, 
/// grounds the camera, then streams chunks asynchronously.
pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app
            // 1) Load the HeightmapData resource first
            .add_systems(Startup, load_heightmap_data)
            // 2) Preload just the terrain material (after heightmap exists)
            .add_systems(Startup, preload_terrain_assets.after(load_heightmap_data))
            // 3) Initialize our chunk‐tracking & async‐loader resources
            .init_resource::<ChunkManager>()
            .init_resource::<AsyncChunkLoader>()
            // 4) Every frame (during gameplay), snap the camera Y to the terrain
            .add_systems(
                Update,
                camera_grounding_system
                    .run_if(in_state(GameState::Running))
            )
            // 5) Then schedule any newly needed chunks off-thread
            .add_systems(
                Update,
                async_schedule_chunks
                    .after(camera_grounding_system)
                    .run_if(in_state(GameState::Running))
            )
            // 6) Then pull in finished meshes and spawn them
            .add_systems(
                Update,
                async_receive_chunks
                    .after(async_schedule_chunks)
                    .run_if(in_state(GameState::Running))
            );
    }
}

