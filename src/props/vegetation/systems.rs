use bevy::prelude::*;

use crate::props::core::{
    WorldSeed, ChunkArea, PropArchetypeId, HeightSampler, SlopeSampler,
    finalize_transform, make_prop_id,
};
use crate::props::placement::make_strategy;
use crate::props::registry::PropsRegistry;
use crate::props::streaming::spawn_prop_instance;
use crate::props::plugin::{TerrainChunkLoaded, PropsRegistryHandle};

use super::plugin::FlatGround; // or your real terrain samplers
use super::rules::{height_rule_from_filters, HeightRule};

/// System: when chunks load, spawn vegetation for them.
/// Wire this in your VegetationPlugin: `.add_systems(Update, on_chunk_loaded_spawn_vegetation)`
pub fn on_chunk_loaded_spawn_vegetation(
    mut evr: EventReader<TerrainChunkLoaded>,
    regs: Res<Assets<PropsRegistry>>,
    handle: Res<PropsRegistryHandle>,
    seed: Res<WorldSeed>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    // Replace FlatGround with your real sampler resources when ready
    ground: Res<FlatGround>,
) {
    let Some(registry) = regs.get(&handle.0) else { return; };

    for ev in evr.read() {
        spawn_vegetation_for_chunk_internal(
            &mut commands,
            &assets,
            registry,
            *seed,
            &ev.0,
            &*ground, // HeightSampler
            &*ground, // SlopeSampler
        );
    }
}

/// Internal: does the actual per-archetype placement inside one chunk.
fn spawn_vegetation_for_chunk_internal(
    commands: &mut Commands,
    assets: &AssetServer,
    registry: &PropsRegistry,
    world_seed: WorldSeed,
    area: &ChunkArea,
    hs: &dyn HeightSampler,
    sn: &dyn SlopeSampler,
) {
    for (idx, arche) in registry.archetypes.iter().enumerate() {
        // Only vegetation for now (simple category gate)
        if arche.category.as_deref() != Some("vegetation") {
            continue;
        }

        let arche_id = PropArchetypeId(idx as u32);

        // Deterministic placement strategy (grid / poisson etc. from registry)
        let strat = make_strategy(&arche.placement, arche_id);
        let probes = strat.place(world_seed, area, arche_id);

        // Vegetation-specific rule: altitude bounds only (for now)
        let hrule: HeightRule = height_rule_from_filters(&arche.filters);

        // Optional debug: how many candidates we’re processing
        info!("Veg: {} -> {} probes", arche.name, probes.len());

        for probe in probes {
            // Altitude gate (no slope/biome here—keep rules simple on purpose)
            let h = hs.sample_height(probe.x, probe.z);
            if !hrule.contains(h) {
                continue;
            }

            // Final transform (height snap + optional normal align)
            let (translation, rotation, scale) =
                finalize_transform(&probe, hs, sn, arche.height_snap);
            let transform = Transform {
                translation,
                rotation,
                scale,
                ..Default::default()
            };

            // Stable identity and spawn
            let id = make_prop_id(area.coord, probe.local_index.0, arche_id.0);
            spawn_prop_instance(commands, assets, id, area.coord, &arche.render, transform);
        }
    }
}
