use bevy::prelude::*;
use crate::terrain::systems::load_heightmap_data;
use crate::terrain::chunking::ChunkManager;
use crate::terrain::async_chunk_loader::{AsyncChunkLoader, async_schedule_chunks, async_receive_chunks, debug_spawn_corners, cleanup_distant_chunks};

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app
            // Load your heightmap once at startup
            .add_systems(Startup, load_heightmap_data)

            // Init the two key resources
            .init_resource::<ChunkManager>()
            .init_resource::<AsyncChunkLoader>()

            // On Update:
            // 1. schedule new chunk‐build tasks
            .add_systems(Update, async_schedule_chunks)
            // 2. after scheduling, poll & receive any finished tasks
            .add_systems(Update, async_receive_chunks.after(async_schedule_chunks))
            // 3. then clean up distant chunks
            .add_systems(Update, cleanup_distant_chunks.after(async_receive_chunks))
            // 4. optional debug spawner (runs last)
            .add_systems(Update, debug_spawn_corners.after(cleanup_distant_chunks));
    }
}
