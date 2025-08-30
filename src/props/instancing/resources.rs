// src/props/instancing/resources.rs

use bevy::prelude::*;
use std::collections::HashMap;
use crate::props::core::{ChunkCoord, PropArchetypeId};

#[derive(Resource, Default)]
pub struct InstanceBatches {
    pub by_key: HashMap<(ChunkCoord, PropArchetypeId), Entity>,
}

#[derive(Resource)]
pub struct PropsInstancingConfig {
    pub max_merges_per_frame: usize,
    pub max_visible_distance_sq: f32,
}
impl Default for PropsInstancingConfig {
    fn default() -> Self {
        Self {
            max_merges_per_frame: 1,        // 1 per frame avoids spikes
            max_visible_distance_sq: 350.0 * 350.0, // ~350m cull for vegetation
        }
    }
}
