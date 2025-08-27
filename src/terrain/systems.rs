use bevy::prelude::*;

use crate::heightmap_data::{HeightTileCache, HeightmapData};
use crate::terrain::chunking::ChunkManager;
use crate::terrain::async_chunk_loader::AsyncChunkLoader;
use crate::terrain::water::WaterLevel;

/// Vertex grid per chunk (X,Z). Use odd counts so edges align.
pub const GRID_RES: UVec2 = UVec2::new(65, 65);

/// World-space size of one chunk (X,Z).
pub const CHUNK_SIZE: Vec2 = Vec2::new(256.0, 256.0);

pub fn init_terrain_params(mut commands: Commands, mut hmd: ResMut<HeightmapData>) {
    hmd.chunk_size = CHUNK_SIZE;
    commands.insert_resource(ChunkManager::new());
}

/// All the knobs that used to be `const` in plugin.rs
#[derive(Resource, Clone)]
pub struct TerrainConfig {
    pub raw_folder: &'static str,
    pub filename_prefix: &'static str,
    pub filename_ext: &'static str,

    pub color_folder: &'static str,
    pub color_prefix: &'static str,
    pub color_ext: &'static str,

    pub tile_res_x: u32,
    pub tile_res_z: u32,
    pub tiles_x: i32,
    pub tiles_z: i32,

    pub raw_min: f32,
    pub raw_max: f32,

    pub origin_x: f32,
    pub origin_z: f32,
    pub height_scale_m: f32,

    pub default_water_level: f32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            raw_folder: "assets/heightmaps",
            filename_prefix: "Heightmap",
            filename_ext: ".r16",

            color_folder: "textures",
            color_prefix: "Texture",
            color_ext: ".png",

            tile_res_x: 1024,
            tile_res_z: 1024,
            tiles_x: 16,
            tiles_z: 16,

            raw_min: 0.0,
            raw_max: 65535.0,

            origin_x: 0.0,
            origin_z: 0.0,
            height_scale_m: 600.0,

            default_water_level: 40.0,
        }
    }
}

/// Startup: build and insert core terrain resources (HeightmapData, cache, loader, water level).
pub fn init_terrain_resources(
    mut commands: Commands,
    cfg: Res<TerrainConfig>,
) {
    // Compute total world size from chunk size and tile counts
    let world_size = Vec2::new(CHUNK_SIZE.x * cfg.tiles_x as f32,
                               CHUNK_SIZE.y * cfg.tiles_z as f32);

    // Global heightmap metadata
    let hmd = HeightmapData {
        size: world_size,
        origin: Vec2::new(cfg.origin_x, cfg.origin_z),
        height_scale: cfg.height_scale_m,
        chunk_size: CHUNK_SIZE,
        raw_minmax: (cfg.raw_min, cfg.raw_max),
    };

    // RAW16 tile cache
    let mut cache = HeightTileCache::new(cfg.raw_folder, UVec2::new(cfg.tile_res_x, cfg.tile_res_z));
    cache.filename_prefix = cfg.filename_prefix.to_string();
    cache.filename_ext = cfg.filename_ext.to_string();

    // Insert resources used by terrain pipeline
    commands.insert_resource(hmd);
    commands.insert_resource(cache);
    commands.insert_resource(WaterLevel(cfg.default_water_level));
    commands.insert_resource(AsyncChunkLoader::default());
}