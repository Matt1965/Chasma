// src/props/streaming/chunk_bind.rs
//! Chunk-aware spawn/despawn glue.

use bevy::prelude::*;
use crate::props::core::{PropId, ChunkCoord};
use crate::props::plugin::TerrainChunkUnloaded;
use crate::props::registry::RenderRef;
use super::{PropInstance, PropChunkTag, spawn_render_ref};

/// Spawn a single prop instance, tag it with stable identity + chunk tag, and return the root.
pub fn spawn_prop_instance(
    commands: &mut Commands,
    assets: &AssetServer,
    id: PropId,
    chunk: ChunkCoord,
    render: &RenderRef,
    transform: Transform,
) -> Entity {
    let root = spawn_render_ref(commands, assets, None, render, transform);
    commands.entity(root).insert((PropInstance { id }, PropChunkTag(chunk)));
    root
}

/// Despawn all props that belong to an unloaded chunk.
pub fn despawn_chunk_props(
    mut evr: EventReader<TerrainChunkUnloaded>,
    q: Query<(Entity, &PropChunkTag)>,
    mut commands: Commands,
) {
    for ev in evr.read() {
        let coord = ev.0;
        for (e, tag) in q.iter() {
            if tag.0 == coord {
                commands.entity(e).despawn();
            }
        }
    }
}
