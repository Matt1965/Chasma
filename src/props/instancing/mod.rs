//! CPU instancing (mesh merging) for props.
//! Groups spawns by (ChunkCoord, PropArchetypeId) into a single batch entity,
//! then builds one combined Mesh per batch using the base mesh transformed by
//! all queued instance transforms. This reduces draw calls drastically while
//! staying compatible with StandardMaterial / Bevy PBR.

pub mod components;
pub mod resources;
pub mod systems;
pub mod async_spawn;

pub use components::{InstanceBatch, BatchStats};