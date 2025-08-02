// src/terrain/chunking.rs

use std::collections::HashMap;
use bevy::prelude::*;

/// How many chunks (in each direction) around the camera to keep loaded.
pub const CHUNK_RADIUS: i32 = 1;

/// Tracks which chunk coords are currently spawned,
/// and what the camera’s last‐chunk was so we only schedule on boundary‐cross.
#[derive(Resource, Default)]
pub struct ChunkManager {
    /// Map from chunk‐coord → Entity
    pub loaded:         HashMap<(i32, i32), Entity>,
    /// Last camera chunk, to detect when we need to load/unload.
    pub last_cam_chunk: Option<(i32, i32)>,
}
