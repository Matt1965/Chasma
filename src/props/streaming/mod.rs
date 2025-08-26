// src/props/streaming/mod.rs
pub mod chunk_bind;
pub mod lod;
pub mod systems;
pub mod plugin;
pub mod components;
pub mod queue_drain;

pub use chunk_bind::{spawn_prop_instance};
