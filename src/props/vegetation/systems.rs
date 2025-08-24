// src/props/vegetation/systems.rs
use bevy::prelude::*;
use crate::props::core::{
    WorldSeed, finalize_transform, make_prop_id, PropArchetypeId, HeightSampler, SlopeSampler,
};
use crate::props::placement::make_strategy;
use crate::props::registry::PropsRegistry;
use crate::props::streaming::spawn_prop_instance;
use crate::props::plugin::{TerrainChunkLoaded, PropsRegistryHandle};
use super::rules::height_rule_from_filters;
use super::sampler::TerrainHeightSampler;

/// System: on chunk-loaded, spawn vegetation for that chunk using the core samplers.
pub fn spawn_veg_on_chunk_loaded(
    mut evr: EventReader<TerrainChunkLoaded>,
    regs: Res<Assets<PropsRegistry>>,
    handle: Res<PropsRegistryHandle>,
    seed: Res<WorldSeed>,
    mut commands: Commands,
    assets: Res<AssetServer>,

    // ← our `'static` implementor of HeightSampler + SlopeSampler
    hs: Res<TerrainHeightSampler>,
    sn: Res<TerrainHeightSampler>,
) {
    // Ensure registry is live before draining events
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

            let mut placed = 0usize;
            for probe in probes {
                // Height gate using the sampler trait
                let h = hs.sample_height(probe.x, probe.z);
                if !hrule.contains(h) {
                    continue;
                }

                // Snap/rotate using your core helper (works with the trait)
                let (translation, rotation, scale) =
                    finalize_transform(&probe, &*hs, &*sn, arche.height_snap);
                let transform = Transform { translation, rotation, scale, ..Default::default() };

                // stable identity + spawn
                let id = make_prop_id(area.coord, probe.local_index.0, arche_id.0);
                spawn_prop_instance(&mut commands, &assets, id, area.coord, &arche.render, transform);
                placed += 1;
            }

            info!(
                "veg: chunk ({}, {}), arche '{}': placed {}",
                area.coord.x, area.coord.z, arche.name, placed
            );
        }
    }
}
