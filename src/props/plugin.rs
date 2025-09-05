use bevy::prelude::*;

use super::core::{ChunkArea, ChunkCoord, WorldSeed};
use super::registry::{PropsRegistry, PropsRegistryAssetPlugin};
use super::queue::{SpawnQueue, SpawnQueueConfig};

use crate::props::instancing::resources::{InstanceBatches, PropsInstancingConfig, MergeIntegrationQueue};
use crate::props::instancing::systems::{
    drain_spawn_queue_into_batches,
    rebuild_dirty_batches,
    poll_merge_tasks,
    integrate_finished_merges,
    cleanup_batches_on_chunk_unloaded,
};
use crate::props::instancing::async_spawn::{
    schedule_async_placement_tasks,
    collect_placement_results,
    PropPlacementTasks,
};

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum PropSystemSet {
    AsyncPlacement,
    DrainQueue,
    RebuildBatches,
    MergeIntegration,
    CleanupBatches,
}

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

#[derive(Resource, Default)]
pub struct PropsRegistryHandle(pub Handle<PropsRegistry>);

#[derive(Event, Clone, Copy)]
pub struct TerrainChunkLoaded(pub ChunkArea);

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
            .init_resource::<MergeIntegrationQueue>()
            .init_resource::<PropPlacementTasks>()
            .add_event::<TerrainChunkLoaded>()
            .add_event::<TerrainChunkUnloaded>()
            .add_systems(Startup, (
                init_world_seed_from_settings,
                load_registry,
            ))

            // ---------- Configure System Sets ----------
            .configure_sets(Update, (
                PropSystemSet::AsyncPlacement,
                PropSystemSet::DrainQueue.after(PropSystemSet::AsyncPlacement),
                PropSystemSet::RebuildBatches.after(PropSystemSet::DrainQueue),
                PropSystemSet::MergeIntegration.after(PropSystemSet::RebuildBatches),
                PropSystemSet::CleanupBatches.after(PropSystemSet::MergeIntegration),
            ))

            // ---------- Registry / Logging ----------
            .add_systems(Update, (
                monitor_registry_ready,
                log_chunk_events,
            ))

            // ---------- Async Placement ----------
            .add_systems(Update, (
                schedule_async_placement_tasks
                    .run_if(registry_ready)
                    .in_set(PropSystemSet::AsyncPlacement),
                collect_placement_results
                    .run_if(registry_ready)
                    .in_set(PropSystemSet::AsyncPlacement),
            ))

            // ---------- Instancing pipeline ----------
            .add_systems(Update, drain_spawn_queue_into_batches.in_set(PropSystemSet::DrainQueue))
            .add_systems(Update, rebuild_dirty_batches.in_set(PropSystemSet::RebuildBatches))
            .add_systems(Update, poll_merge_tasks.in_set(PropSystemSet::MergeIntegration))
            .add_systems(Update, integrate_finished_merges.in_set(PropSystemSet::MergeIntegration))
            .add_systems(Update, cleanup_batches_on_chunk_unloaded.in_set(PropSystemSet::CleanupBatches));
    }
}

fn registry_ready(
    handle: Res<PropsRegistryHandle>,
    regs: Res<Assets<PropsRegistry>>,
) -> bool {
    regs.get(&handle.0).is_some()
}

fn init_world_seed_from_settings(mut commands: Commands, settings: Res<PropsSettings>) {
    commands.insert_resource(WorldSeed(settings.world_seed));
}

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

fn log_chunk_events(mut evr: EventReader<TerrainChunkLoaded>) {
    for ev in evr.read() {
        info!(
            "Props: got TerrainChunkLoaded at ({}, {})",
            ev.0.coord.x, ev.0.coord.z
        );
    }
}
