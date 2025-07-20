// src/unit/mod.rs

// these sub‐modules stay private
mod components;
mod systems;
mod plugin;

// re-export the one thing callers actually need:
pub use plugin::UnitPlugin;