// src/props/vegetation/samplers.rs
use bevy::prelude::*;
use std::sync::Mutex;

use crate::heightmap_data::{HeightmapData, HeightTileCache, sample_height, HeightSampler, SlopeSampler};

/// A `'static` resource that can answer height (and later slope) queries
/// by reading your RAW16 tile cache.
#[derive(Resource)]
pub struct TerrainHeightSampler {
    data: HeightmapData,
    cache: Mutex<HeightTileCache>,
}

impl TerrainHeightSampler {
    pub fn new_from(data: &HeightmapData, cache_cfg: &HeightTileCache) -> Self {
        // Recreate an empty cache with the same config (we only copy the IO config, not tiles)
        let mut new_cache = HeightTileCache::new(&cache_cfg.folder, cache_cfg.tile_resolution);
        new_cache.filename_prefix = cache_cfg.filename_prefix.clone();
        new_cache.filename_ext = cache_cfg.filename_ext.clone();

        Self {
            data: data.clone(),             // HeightmapData is Clone in your code
            cache: Mutex::new(new_cache),   // keep our own cache for thread-safe access
        }
    }
}

impl HeightSampler for TerrainHeightSampler {
    fn sample_height(&self, x: f32, z: f32) -> f32 {
        // lock the cache and sample; fall back to 0.0 if OOB or tile missing
        let mut guard = self.cache.lock().expect("height cache mutex poisoned");
        sample_height(x, z, &self.data, &mut *guard).unwrap_or(0.0)
    }
}