use bevy::prelude::*;

use crate::heightmap_data::{HeightTileCache, HeightmapData};
use crate::terrain::async_chunk_loader::{
    async_receive_chunks, async_schedule_chunks, AsyncChunkLoader, IntegrationBudget, MeshBuildBudget, // ← added IntegrationBudget
};
use crate::terrain::systems::{init_terrain_params, CHUNK_SIZE};
use crate::terrain::water::{spawn_water, WaterLevel};

// ---- Configure these to match your Gaea export ----
const RAW_FOLDER: &str = "assets/heightmaps";   // where your *.raw16 tiles live
const FILENAME_PREFIX: &str = "Heightmap";      // -> {prefix}_y{cz}_x{cx}.raw16
const FILENAME_EXT: &str = ".r16";              // UshortRaw16
pub const COLOR_FOLDER: &str = "textures";      // folder containing color tiles
pub const COLOR_PREFIX: &str = "Texture";       // -> {prefix}_y{cz}_x{cx}{ext}
pub const COLOR_EXT: &str = ".png";             // your exported color tile ext
const TILE_RES_X: u32 = 1024;
const TILE_RES_Z: u32 = 1024;
const TILES_X: i32 = 16;
const TILES_Z: i32 = 16;

// RAW normalization
const RAW_MIN: f32 = 0.0;
const RAW_MAX: f32 = 65535.0;

// World mapping
const TERRAIN_ORIGIN_X: f32 = 0.0;
const TERRAIN_ORIGIN_Z: f32 = 0.0;
const HEIGHT_SCALE_METERS: f32 = 600.0;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        // Compute total world size from chunk size and tile counts
        let world_size = Vec2::new(
            CHUNK_SIZE.x * TILES_X as f32,
            CHUNK_SIZE.y * TILES_Z as f32,
        );

        // Global heightmap metadata
        let hmd = HeightmapData {
            size: world_size,
            origin: Vec2::new(TERRAIN_ORIGIN_X, TERRAIN_ORIGIN_Z),
            height_scale: HEIGHT_SCALE_METERS,
            chunk_size: CHUNK_SIZE, // reaffirmed in init_terrain_params
            raw_minmax: (RAW_MIN, RAW_MAX),
        };

        // RAW16 tile cache
        let mut cache = HeightTileCache::new(RAW_FOLDER, UVec2::new(TILE_RES_X, TILE_RES_Z));
        cache.filename_prefix = FILENAME_PREFIX.to_string();
        cache.filename_ext = FILENAME_EXT.to_string();

        app
            // Core resources
            .insert_resource(hmd)
            .insert_resource(cache)
            .insert_resource(WaterLevel(40.0))
            .insert_resource(AsyncChunkLoader::default())
            .insert_resource(MeshBuildBudget::default())
            .insert_resource(IntegrationBudget::default()) // ← add the budget resource
            // Initialize chunk manager + push CHUNK_SIZE into HeightmapData
            .add_systems(Startup, init_terrain_params)
            .add_systems(Startup, spawn_water)
            // Streaming pipeline (unchanged order; budget is enforced in async_receive_chunks)
            .add_systems(Update, (async_schedule_chunks, async_receive_chunks).chain());
    }
}
