// src/props/plugin.rs
//! Props plugin wiring (small & glue-only).
//! - Registers the PropsRegistry asset/loader
//! - Exposes chunk load/unload events (terrain can emit these)
//! - Holds settings + world seed + registry handle
//! - Kicks off loading the registry and logs when it's ready
//! - Drains the shared SpawnQueue each frame

use bevy::prelude::*;

use super::core::{ChunkArea, ChunkCoord, WorldSeed};
use super::registry::{PropsRegistry, PropsRegistryAssetPlugin};
use super::queue::{SpawnQueue, SpawnQueueConfig, SpawnRequest};
use super::streaming::spawn_prop_instance;

/// Configure where the registry manifest lives and the world seed.
#[derive(Resource, Clone)]
pub struct PropsSettings {
    /// Path to the registry manifest (RON). Example: "props/archetypes.props.ron"
    pub registry_path: String,
    /// Global seed for deterministic placement.
    pub world_seed: u64,
}
impl Default for PropsSettings {
    fn default() -> Self {
        Self {
            registry_path: "props/archetypes.props.ron".to_string(),
            world_seed: 1337,
        }
    }
}

/// Handle to the loaded PropsRegistry asset.
#[derive(Resource, Default)]
pub struct PropsRegistryHandle(pub Handle<PropsRegistry>);

/// Fired by terrain when a chunk becomes active in the world.
#[derive(Event, Clone, Copy)]
pub struct TerrainChunkLoaded(pub ChunkArea);

/// Fired by terrain when a chunk is removed/unloaded.
#[derive(Event, Clone, Copy)]
pub struct TerrainChunkUnloaded(pub ChunkCoord);

pub struct PropsPlugin;

impl Plugin for PropsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PropsRegistryAssetPlugin)
            // small, declarative resources
            .init_resource::<PropsSettings>()
            .init_resource::<PropsRegistryHandle>()
            // shared spawn queue + config (NEW)
            .init_resource::<SpawnQueue>()
            .init_resource::<SpawnQueueConfig>()
            // derive WorldSeed from PropsSettings
            .add_systems(Startup, init_world_seed_from_settings)
            .add_systems(Update, log_chunk_events)
            // chunk events that terrain should emit
            .add_event::<TerrainChunkLoaded>()
            .add_event::<TerrainChunkUnloaded>()
            // registry load + monitor
            .add_systems(Startup, load_registry)
            .add_systems(Update, monitor_registry_ready)
            // drain queued spawns every frame (NEW)
            .add_systems(Update, drain_spawn_queue);
    }
}

/// Startup: insert WorldSeed based on PropsSettings (avoids mutable/immutable borrow clash).
fn init_world_seed_from_settings(mut commands: Commands, settings: Res<PropsSettings>) {
    commands.insert_resource(WorldSeed(settings.world_seed));
}

/// Startup: request loading the registry manifest, store handle.
fn load_registry(
    mut handle_res: ResMut<PropsRegistryHandle>,
    settings: Res<PropsSettings>,
    assets: Res<AssetServer>,
) {
    if handle_res.0.is_strong() { return; }
    let h: Handle<PropsRegistry> = assets.load(settings.registry_path.as_str());
    handle_res.0 = h;
    info!(
        "Props: loading registry from '{}', world_seed={}",
        settings.registry_path, settings.world_seed
    );
}

/// Update: log once when the registry becomes available.
fn monitor_registry_ready(
    handle_res: Res<PropsRegistryHandle>,
    registries: Res<Assets<PropsRegistry>>,
    mut logged: Local<bool>,
) {
    if *logged { return; }
    if registries.get(&handle_res.0).is_some() {
        *logged = true;
        info!("Props: registry loaded and ready");
    }
}

fn log_chunk_events(mut evr: EventReader<crate::props::plugin::TerrainChunkLoaded>) {
    for ev in evr.read() {
        info!(
            "Props: got TerrainChunkLoaded at ({}, {})",
            ev.0.coord.x, ev.0.coord.z
        );
    }
}

/// Drain up to `max_per_frame` queued spawns and instantiate them.
fn drain_spawn_queue(
    mut commands: Commands,
    mut queue: ResMut<SpawnQueue>,
    cfg: Res<SpawnQueueConfig>,
    assets: Res<AssetServer>,
) {
    if queue.items.is_empty() { return; }

    let to_spawn = cfg.max_per_frame.min(queue.items.len());
    // LIFO is fast; switch to a small ring buffer if FIFO matters later.
    for _ in 0..to_spawn {
        if let Some(SpawnRequest { id, chunk, render, transform }) = queue.items.pop() {
            spawn_prop_instance(&mut commands, &assets, id, chunk, &render, transform);
        }
    }
    // Optional: uncomment to watch it working
    // info!("Props: drained {} queued spawns ({} remaining)", to_spawn, queue.items.len());
}
