// src/props/placement/poisson.rs
//! Bridson Poisson-disc sampling inside a chunk (deterministic).

use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use super::{clamp_into_chunk, make_probe};
use crate::props::core::{
    PlacementProbe, PlacementStrategy, PropArchetypeId, WorldSeed, ChunkArea,
};

#[derive(Clone, Copy, Debug)]
pub struct PoissonParams {
    /// Minimum distance between points (meters)
    pub radius: f32,
    /// Attempts per active sample
    pub tries: u32,
    /// Absolute cap on generated samples
    pub cap: usize,
}

pub struct PoissonPlacement {
    params: PoissonParams,
    arche: PropArchetypeId,
}

impl PoissonPlacement {
    pub fn new(radius: f32, tries: u32, cap: usize, arche: PropArchetypeId) -> Self {
        let r = radius.max(0.001);
        let t = tries.max(1);
        Self { params: PoissonParams { radius: r, tries: t, cap }, arche }
    }

    #[inline]
    fn rng_for(&self, seed: WorldSeed, chunk: &ChunkArea) -> ChaCha8Rng {
        let mix = (seed.0)
            ^ ((chunk.coord.x as u64) << 16)
            ^ ((chunk.coord.z as u64) << 32)
            ^ ((self.arche.0 as u64) << 48)
            ^ 0x9E37_79B9_7F4A_7C15u64;
        ChaCha8Rng::seed_from_u64(mix)
    }
}

impl PlacementStrategy for PoissonPlacement {
    fn place(
        &self,
        world_seed: WorldSeed,
        chunk: &ChunkArea,
        _archetype: PropArchetypeId,
    ) -> Vec<PlacementProbe> {
        let mut rng = self.rng_for(world_seed, chunk);
        let r = self.params.radius;
        let r2 = r * r;
        let tries = self.params.tries;

        let min = chunk.min_xz;
        let max = chunk.max_xz;
        let size = max - min;

        // Grid to accelerate neighbor queries (cell side = r / sqrt(2))
        let cell = r * std::f32::consts::FRAC_1_SQRT_2;
        let gx = (size.x / cell).ceil().max(1.0) as i32;
        let gz = (size.y / cell).ceil().max(1.0) as i32;
        let mut grid: Vec<Option<usize>> = vec![None; (gx * gz) as usize];

        let mut samples: Vec<Vec2> = Vec::new();
        let mut active: Vec<usize> = Vec::new();

        // Seed with one random point
        {
            let x = rng.random_range(min.x..max.x);
            let z = rng.random_range(min.y..max.y);
            samples.push(Vec2::new(x, z));
            let ix = ((x - min.x) / cell).floor() as i32;
            let iz = ((z - min.y) / cell).floor() as i32;
            grid[(iz * gx + ix) as usize] = Some(0);
            active.push(0);
        }

        // Bridson loop (randomly pick from the active list without SliceRandom)
        while !active.is_empty() {
            let pick = rng.random_range(0..active.len());
            let idx = active[pick];
            let base = samples[idx];
            let mut found = false;

            for _ in 0..tries {
                // Candidate in annulus [r, 2r)
                let ang  = rng.random_range(0.0..std::f32::consts::TAU);
                let dist = r * (1.0 + rng.random::<f32>());
                let (sx, cz) = (ang.cos(), ang.sin());
                let (x, z) = clamp_into_chunk(base.x + dist * sx, base.y + dist * cz, chunk);

                // Grid cell
                let ix = ((x - min.x) / cell).floor() as i32;
                let iz = ((z - min.y) / cell).floor() as i32;

                // Neighborhood check (±2 cells is sufficient)
                let mut ok = true;
                for dz in -2..=2 {
                    for dx in -2..=2 {
                        let nx = ix + dx;
                        let nz = iz + dz;
                        if nx < 0 || nz < 0 || nx >= gx || nz >= gz { continue; }
                        if let Some(si) = grid[(nz * gx + nx) as usize] {
                            let p = samples[si];
                            let d2 = (p.x - x) * (p.x - x) + (p.y - z) * (p.y - z);
                            if d2 < r2 { ok = false; break; }
                        }
                    }
                    if !ok { break; }
                }

                if ok {
                    let next_i = samples.len();
                    samples.push(Vec2::new(x, z));
                    grid[(iz * gx + ix) as usize] = Some(next_i);
                    active.push(next_i);
                    found = true;
                    if samples.len() >= self.params.cap { break; }
                }
            }

            if !found {
                // Retire this active sample
                active.swap_remove(pick);
            }
            if samples.len() >= self.params.cap { break; }
        }

        // Build probes with stable local indices [0..N)
        let mut out = Vec::with_capacity(samples.len());
        let mut local: u32 = 0;
        for p in samples {
            let rot_y = rng.random_range(0.0..std::f32::consts::TAU); // deterministic given RNG state
            out.push(make_probe(local, p.x, p.y, rot_y, 1.0));
            local = local.wrapping_add(1);
        }
        out
    }
}
