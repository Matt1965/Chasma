use bevy::prelude::*;
use crate::terrain::systems::load_heightmap_data;
use crate::terrain::chunking::ChunkManager;
use crate::terrain::async_chunk_loader::{AsyncChunkLoader, async_schedule_chunks, async_receive_chunks, debug_spawn_corners, cleanup_distant_chunks};

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, load_heightmap_data)
            .init_resource::<ChunkManager>()
            .init_resource::<AsyncChunkLoader>()
            .add_systems(Update, async_schedule_chunks)
            .add_systems(Update, async_receive_chunks)
            .add_systems(Update, cleanup_distant_chunks)
            .add_systems(Update, debug_spawn_corners);
    }
}
