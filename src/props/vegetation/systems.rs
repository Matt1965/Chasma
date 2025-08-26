use bevy::prelude::*;
use crate::props::core::{
    WorldSeed, finalize_transform, make_prop_id, PropArchetypeId,
};
use crate::props::placement::make_strategy;
use crate::props::registry::PropsRegistry;
use crate::props::plugin::{TerrainChunkLoaded, PropsRegistryHandle};
use super::rules::height_rule_from_filters;
use super::sampler::TerrainHeightSampler;
use crate::props::core::HeightSampler;

// NEW: enqueue instead of spawning directly
use crate::props::queue::{SpawnQueue, SpawnRequest};

/// System: on chunk-loaded, enqueue vegetation spawns for that chunk.
/// The queue will be drained elsewhere (shared props drainer) with a per-frame cap.
pub fn spawn_veg_on_chunk_loaded(
    mut evr: EventReader<TerrainChunkLoaded>, 
    regs: Res<Assets<PropsRegistry>>,
    handle: Res<PropsRegistryHandle>,
    seed: Res<WorldSeed>,
    mut queue: ResMut<SpawnQueue>,
    assets: Res<AssetServer>,                 // kept if your RenderRef paths need AssetServer (hashing); otherwise could be dropped
    hs: Res<TerrainHeightSampler>,            // implements HeightSampler
    sn: Res<TerrainHeightSampler>,            // implements SlopeSampler
) {
    let _ = assets; // silence unused if not needed
    let Some(reg) = regs.get(&handle.0) else { return; };

    for ev in evr.read() {
        let area = &ev.0;

        for (idx, arche) in reg.archetypes.iter().enumerate() {
            if arche.category.as_deref() != Some("vegetation") {
                continue;
            }

            let arche_id = PropArchetypeId(idx as u32);
            let strat = make_strategy(&arche.placement, arche_id);
            let probes = strat.place(*seed, area, arche_id);

            let hrule = height_rule_from_filters(&arche.filters);

            // gather height stats BEFORE filtering (handy for debugging)
            let mut h_min = f32::INFINITY;
            let mut h_max = f32::NEG_INFINITY;
            let mut h_sum = 0.0f32;
            let mut h_cnt = 0usize;

            // filter once (avoid double-sampling)
            let mut accepted = Vec::with_capacity(probes.len());

            for probe in probes {
                let h = hs.sample_height(probe.x, probe.z);

                if h.is_finite() {
                    if h < h_min { h_min = h; }
                    if h > h_max { h_max = h; }
                    h_sum += h;
                    h_cnt += 1;
                }

                if hrule.contains(h) {
                    accepted.push(probe);
                }
            }

            if h_cnt > 0 {
                let h_avg = h_sum / h_cnt as f32;
                info!(
                    "veg heights: chunk({},{}) arche='{}' -> min={:.2} avg={:.2} max={:.2} N={} | filter min={:?} max={:?}",
                    area.coord.x, area.coord.z, arche.name, h_min, h_avg, h_max, h_cnt, hrule.min, hrule.max
                );
            } else {
                info!(
                    "veg heights: chunk({},{}) arche='{}' -> no samples",
                    area.coord.x, area.coord.z, arche.name
                );
            }

            // enqueue accepted spawns (actual spawning happens in the props drainer)
            let mut enq = 0usize;
            for probe in accepted {
                let (translation, rotation, scale) =
                    finalize_transform(&probe, &*hs, &*sn, arche.height_snap);
                let transform = Transform { translation, rotation, scale, ..Default::default() };

                let id = make_prop_id(area.coord, probe.local_index.0, arche_id.0);

                queue.items.push(SpawnRequest {
                    id,
                    chunk: area.coord,
                    render: arche.render.clone(),  // RenderRef should be Clone
                    transform,
                });
                enq += 1;
            }

            info!(
                "veg: chunk({},{}) arche='{}' enqueued {}",
                area.coord.x, area.coord.z, arche.name, enq
            );
        }
    }
}
