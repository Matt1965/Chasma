// src/props/placement/mod.rs
//! Deterministic placement strategies and helpers.

use crate::props::core::{
    PlacementProbe, PlacementStrategy, PropArchetypeId, ChunkArea, LocalSpawnIndex,
};
use crate::props::registry::PlacementStrategyDef;
use std::sync::Arc;

mod grid;
mod poisson;
pub mod masks;
pub mod runner;

pub use grid::{GridPlacement};
pub use poisson::{PoissonPlacement};

/// Factory: build a boxed strategy from a registry `PlacementStrategyDef`.
pub fn make_strategy(def: &PlacementStrategyDef, arche: PropArchetypeId) -> Arc<dyn PlacementStrategy> {
    match def {
        PlacementStrategyDef::Grid { cell, jitter, cap } => {
            Arc::new(GridPlacement::new(*cell, *jitter, cap.unwrap_or(usize::MAX), arche))
        }
        PlacementStrategyDef::Poisson { radius, tries, cap } => {
            Arc::new(PoissonPlacement::new(*radius, *tries, cap.unwrap_or(usize::MAX), arche))
        }
    }
}

/// Helper to clamp a point inside an XZ AABB.
#[inline]
pub fn clamp_into_chunk(x: f32, z: f32, chunk: &ChunkArea) -> (f32, f32) {
    (
        x.clamp(chunk.min_xz.x, chunk.max_xz.x),
        z.clamp(chunk.min_xz.y, chunk.max_xz.y),
    )
}

/// Common output builder: sequential local indices.
#[inline]
pub fn make_probe(local: u32, x: f32, z: f32, rot_y: f32, scale: f32) -> PlacementProbe {
    PlacementProbe { x, z, rot_y, scale, local_index: LocalSpawnIndex(local) }
}
