use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future;

use crate::props::plugin::{TerrainChunkLoaded, PropsRegistryHandle};
use crate::props::core::{ChunkCoord, WorldSeed, PlacementResult, PropArchetypeId};
use crate::props::registry::{PropsRegistry, RenderRef};
use crate::props::queue::{SpawnQueue, SpawnRequest};
use crate::props::placement::runner::{PlacementContext, run_placement_for_chunk};
use crate::heightmap_data::{HeightSampler, SlopeSampler, HeightmapData, HeightTileCache, TerrainSampleAdapter};

#[derive(Resource, Default)]
pub struct PropPlacementTasks {
    tasks: HashMap<ChunkCoord, Task<Vec<PlacementResult>>>,
}

pub fn schedule_async_placement_tasks(
    mut tasks: ResMut<PropPlacementTasks>,
    mut events: EventReader<TerrainChunkLoaded>,
    registries: Res<Assets<PropsRegistry>>,
    handle: Res<PropsRegistryHandle>,
    seed: Res<WorldSeed>,
    heightmap: Res<HeightmapData>,
    cache: Res<HeightTileCache>,
) {
    let Some(registry) = registries.get(&handle.0) else { return };
    let archetypes = registry.archetypes.clone(); // clone just what we need

    let pool = AsyncComputeTaskPool::get();
    for TerrainChunkLoaded(chunk) in events.read() {
        let coord = chunk.coord;
        if tasks.tasks.contains_key(&coord) {
            continue;
        }

        let chunk = *chunk;
        let seed = *seed;
        let heightmap = heightmap.clone();
        let cache = cache.clone(); // Arc-backed clone
        let archetypes = archetypes.clone(); // 👈 Move this inside loop

        let task = pool.spawn(async move {
            let adapter = TerrainSampleAdapter::new(&heightmap, &cache);
            let mut all = Vec::new();

            for (i, def) in archetypes.iter().enumerate() {
                let id = PropArchetypeId(i as u32);
                let ctx = PlacementContext {
                    chunk,
                    seed,
                    archetype_id: id,
                    def,
                    sampler: &adapter,
                    slope: &adapter,
                };
                let results = run_placement_for_chunk(ctx);
                info!(
                    "Chunk {:?} - archetype {} - got {} placements",
                    chunk.coord,
                    def.name,
                    results.len()
                );
                all.extend(results);
            }

            all
        });

        tasks.tasks.insert(coord, task);
    }

}

pub fn collect_placement_results(
    mut tasks: ResMut<PropPlacementTasks>,
    mut queue: ResMut<SpawnQueue>,
    registry: Res<Assets<PropsRegistry>>,
    handle: Res<PropsRegistryHandle>,
) {
    let Some(registry) = registry.get(&handle.0) else {
        return;
    };

    tasks.tasks.retain(|coord, task| {
        if task.is_finished() {
            if let Some(results) = future::block_on(future::poll_once(task)) {
                info!("Collected {} prop placement results", results.len());
                for result in results {
                    if let Some(def) = registry.get(result.id.archetype) {
                        queue.items.push(SpawnRequest {
                            id: result.id,
                            chunk: *coord,
                            render: def.render.clone(),
                            transform: Transform {
                                translation: result.translation,
                                rotation: result.rotation,
                                scale: result.scale,
                            },
                        });
                    }
                }
            }
            false // remove finished task
        } else {
            true // keep unfinished task
        }
    });
}