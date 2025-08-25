use bevy::prelude::*;
use crate::props::core::ChunkCoord;

/// Tag for quick “belonging to chunk” queries.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropChunkTag(pub ChunkCoord);