// src/props/queue.rs
use bevy::prelude::*;
use crate::props::core::{ChunkCoord, PropId};
use crate::props::registry::RenderRef;

/// One spawn request (what to spawn, where, how).
#[derive(Clone)]
pub struct SpawnRequest {
    pub id: PropId,
    pub chunk: ChunkCoord,
    pub render: RenderRef,
    pub transform: Transform,
}

/// Queue resource (filled by vegetation/buildings/etc).
#[derive(Resource, Default)]
pub struct SpawnQueue {
    pub items: Vec<SpawnRequest>,
}

/// config: how many to actually spawn per frame
#[derive(Resource)]
pub struct SpawnQueueConfig {
    pub max_per_frame: usize,
}
impl Default for SpawnQueueConfig {
    fn default() -> Self { Self { max_per_frame: 200 } }
}
