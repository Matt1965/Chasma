mod plugin;
mod systems;
mod chunking;
mod components;
mod async_chunk_loader;

// Re-export your plugin and label so they’re public:
pub use plugin::TerrainPlugin;

// Also re-export the systems if you liked before:
pub use systems::{load_heightmap_data};
pub use components::{ChunkCoords, LocalOffset};
