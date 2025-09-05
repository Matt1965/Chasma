use bevy::prelude::*;
use std::collections::HashMap;
use crate::props::core::{ChunkCoord, PropArchetypeId};

/// One logical batch per (chunk, archetype).
#[derive(Component)]
pub struct InstanceBatch {
    pub chunk: ChunkCoord,
    pub archetype: PropArchetypeId,
    /// The shared source mesh to duplicate.
    pub base_mesh: Handle<Mesh>,
    /// The material to apply to the merged mesh.
    pub material: Handle<StandardMaterial>,
    /// Accumulated instance transforms (world space).
    pub instances: Vec<Transform>,
    /// True when instances changed since last build.
    pub dirty: bool,
    /// How many instances were baked into the last merged mesh.
    pub last_built_count: usize,
    pub building: bool,
}

impl InstanceBatch {
    #[inline]
    pub fn mark_dirty(&mut self) { self.dirty = true; }

    pub fn clear_build_flags(&mut self, count: usize) {
        self.building = false;
        self.dirty = false;
        self.last_built_count = count;
    }
}

#[derive(Component, Default)]
pub struct BatchStats {
    pub instance_count: u32,
    pub merged_vertex_count: u32,
}
