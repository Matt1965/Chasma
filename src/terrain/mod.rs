mod plugin;
mod systems;
mod chunking;
mod components;
mod async_chunk_loader;   // ← add this line
mod water;

pub use plugin::TerrainPlugin;
pub use systems::load_heightmap_data;
pub use components::{ChunkCoords, LocalOffset};