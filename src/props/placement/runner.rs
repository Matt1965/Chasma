use bevy::prelude::*;

use crate::props::core::*;
use crate::props::registry::PropArchetypeDef;
use crate::props::placement::make_strategy;
use crate::heightmap_data::{HeightSampler, SlopeSampler};

/// Input to placement evaluation
pub struct PlacementContext<'a> {
    pub chunk: ChunkArea,
    pub seed: WorldSeed,
    pub archetype_id: PropArchetypeId,
    pub def: &'a PropArchetypeDef,
    pub sampler: &'a dyn HeightSampler,
    pub slope: &'a dyn SlopeSampler,
}

/// Run placement, filters, and transform snapping for a single prop in a chunk
pub fn run_placement_for_chunk(ctx: PlacementContext) -> Vec<PlacementResult> {
    let strat = make_strategy(&ctx.def.placement, ctx.archetype_id);
    let probes = strat.place(ctx.seed, &ctx.chunk, ctx.archetype_id);
    let mut out = Vec::with_capacity(probes.len());

    let filters = &ctx.def.filters;

    for probe in probes {
        let y = ctx.sampler.sample_height(probe.x, probe.z);

        // --- Altitude Filter ---
        if let Some(min) = filters.altitude_min {
            if y < min {
                continue;
            }
        }
        if let Some(max) = filters.altitude_max {
            if y > max {
                continue;
            }
        }

        // --- Slope Filter ---
        if let Some(min) = filters.slope_min_deg {
            if let Some(s) = ctx.slope.slope_deg(probe.x, probe.z) {
                if s < min {
                    continue;
                }
            }
        }
        if let Some(max) = filters.slope_max_deg {
            if let Some(s) = ctx.slope.slope_deg(probe.x, probe.z) {
                if s > max {
                    continue;
                }
            }
        }

        let (pos, rot, scale) = finalize_transform(
            &probe,
            ctx.sampler,
            ctx.slope,
            ctx.def.height_snap,
        );

        out.push(PlacementResult {
            id: make_prop_id(ctx.chunk.coord, probe.local_index.0, ctx.archetype_id.0),
            translation: pos,
            rotation: rot,
            scale,
        });
    }

    debug!(
        "Chunk {:?} / Archetype {:?}: placed {} props",
        ctx.chunk.coord,
        ctx.archetype_id,
        out.len()
    );

    out
}
