//! Props plugin wiring (glue).
//! - Registry asset/loader
//! - Chunk load/unload events
//! - WorldSeed + settings
//! - Spawn queue + **instancing** systems (router + batch rebuild + cull + cleanup)

use bevy::prelude::*;

use super::core::{ChunkArea, ChunkCoord, WorldSeed};
use super::registry::{PropsRegistry, PropsRegistryAssetPlugin};
use super::queue::{SpawnQueue, SpawnQueueConfig};

use crate::props::instancing::resources::{InstanceBatches, PropsInstancingConfig};
use crate::props::instancing::systems::{
    drain_spawn_queue_into_batches,
    rebuild_dirty_batches,
    cull_batches_by_distance,
    cleanup_batches_on_chunk_unloaded,
};

/// Configure where the registry manifest lives and the world seed.
#[derive(Resource, Clone)]
pub struct PropsSettings {
    pub registry_path: String,
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
            .init_resource::<PropsSettings>()
            .init_resource::<PropsRegistryHandle>()
            .init_resource::<SpawnQueue>()
            .init_resource::<SpawnQueueConfig>()
            .init_resource::<InstanceBatches>()
            .init_resource::<PropsInstancingConfig>()
            .add_systems(Startup, (init_world_seed_from_settings, load_registry))
            .add_systems(Update, (monitor_registry_ready, log_chunk_events))

            .add_event::<TerrainChunkLoaded>()
            .add_event::<TerrainChunkUnloaded>()

            // ---- instancing schedule (CPU-merge, no rebuild churn) ----
            .add_systems(
                Update,
                drain_spawn_queue_into_batches, // enqueue -> batches
            )
            .add_systems(
                Update,
                rebuild_dirty_batches.after(drain_spawn_queue_into_batches),
            )
            .add_systems(
                Update,
                cull_batches_by_distance.after(rebuild_dirty_batches),
            )
            .add_systems(
                Update,
                cleanup_batches_on_chunk_unloaded.after(cull_batches_by_distance),
            );
    }
}

/// Startup: insert WorldSeed based on PropsSettings.
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
