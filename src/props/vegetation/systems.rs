use bevy::prelude::*;
use crate::props::core::{WorldSeed, finalize_transform, make_prop_id, PropArchetypeId};
use crate::props::placement::make_strategy;
use crate::props::registry::PropsRegistry;
use crate::props::plugin::{TerrainChunkLoaded, PropsRegistryHandle};
use crate::props::queue::{SpawnQueue, SpawnRequest};
use crate::heightmap_data::{HeightSampler, SlopeSampler};

use super::rules::height_rule_from_filters;
use super::plugin::VegSampler;

pub fn spawn_veg_on_chunk_loaded(
    mut evr: EventReader<TerrainChunkLoaded>,
    regs: Res<Assets<PropsRegistry>>,
    handle: Res<PropsRegistryHandle>,
    seed: Res<WorldSeed>,
    mut queue: ResMut<SpawnQueue>,
    sampler: Res<VegSampler>,
) {
    let Some(reg) = regs.get(&handle.0) else { return };

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

            let mut accepted = Vec::with_capacity(probes.len());

            let mut h_min = f32::INFINITY;
            let mut h_max = f32::NEG_INFINITY;
            let mut h_sum = 0.0;
            let mut h_cnt = 0;

            for probe in probes {
                let h = sampler.0.sample_height(probe.x, probe.z);

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
            }

            let mut enq = 0;
            for probe in accepted {
                let (translation, rotation, scale) = finalize_transform(
                    &probe, &sampler.0, &sampler.0, arche.height_snap,
                );
                let transform = Transform {
                    translation,
                    rotation,
                    scale,
                    ..Default::default()
                };

                let id = make_prop_id(area.coord, probe.local_index.0, arche_id.0);

                queue.items.push(SpawnRequest {
                    id,
                    chunk: area.coord,
                    render: arche.render.clone(),
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
