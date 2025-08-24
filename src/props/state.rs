// src/props/state.rs
use bevy::prelude::*;
use std::collections::HashMap;

use crate::heightmap_data::HeightmapData; // for key_from_world()
use crate::props::core::PropArchetypeId;

/// Marker on every spawned prop instance.
#[derive(Component)]
pub struct PropInstance {
    /// Unique per-instance id (choose your own scheme).
    pub id: u64,
    /// Which archetype this came from.
    pub archetype: PropArchetypeId,
    /// Chunk key this instance belongs to (cx, cz).
    pub chunk_key: (i32, i32),
}

/// Runtime index of props, bucketed by chunk key.
/// NOTE: no dependency on ChunkCoord — just (i32, i32).
#[derive(Resource, Default)]
pub struct PropsState {
    by_chunk: HashMap<(i32, i32), Vec<Entity>>,
}

impl PropsState {
    /// Register a newly spawned entity under a chunk key.
    pub fn insert_by_key(&mut self, key: (i32, i32), ent: Entity) {
        self.by_chunk.entry(key).or_default().push(ent);
    }

    /// Register using world position by deriving the chunk key from HeightmapData.
    pub fn insert_by_world(
        &mut self,
        world_x: f32,
        world_z: f32,
        hmd: &HeightmapData,
        ent: Entity,
    ) {
        if let Some(key) = key_from_world(world_x, world_z, hmd) {
            self.insert_by_key(key, ent);
        }
    }

    /// Despawn all props in a chunk (by key).
    pub fn despawn_chunk(&mut self, key: (i32, i32), commands: &mut Commands) {
        if let Some(ents) = self.by_chunk.remove(&key) {
            for e in ents {
                commands.entity(e).despawn();
            }
        }
    }

    /// Remove the entity from bookkeeping (does not despawn).
    pub fn remove_entity(&mut self, key: (i32, i32), ent: Entity) {
        if let Some(v) = self.by_chunk.get_mut(&key) {
            if let Some(i) = v.iter().position(|&e| e == ent) {
                v.swap_remove(i);
            }
            if v.is_empty() {
                self.by_chunk.remove(&key);
            }
        }
    }
}

/// Compute the chunk key (cx, cz) from world-space (x, z) using HeightmapData.
/// Returns None if outside the terrain bounds.
pub fn key_from_world(x: f32, z: f32, hmd: &HeightmapData) -> Option<(i32, i32)> {
    let lx = x - hmd.origin.x;
    let lz = z - hmd.origin.y;
    if lx < 0.0 || lz < 0.0 || lx >= hmd.size.x || lz >= hmd.size.y {
        return None;
    }
    let cx = (lx / hmd.chunk_size.x).floor() as i32;
    let cz = (lz / hmd.chunk_size.y).floor() as i32;
    Some((cx, cz))
}
