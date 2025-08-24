// src/props/placement/grid.rs
//! Jittered grid placement (deterministic per seed, chunk, archetype).

use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use super::{clamp_into_chunk, make_probe};
use crate::props::core::{PlacementProbe, PlacementStrategy, PropArchetypeId, WorldSeed, ChunkArea};

#[derive(Clone, Copy, Debug)]
pub struct GridParams {
    pub cell: f32,        // meters
    pub jitter: f32,      // 0..=0.5 (fraction of cell)
    pub cap: usize,       // absolute cap
}

pub struct GridPlacement {
    params: GridParams,
    arche: PropArchetypeId,
}

impl GridPlacement {
    pub fn new(cell: f32, jitter: f32, cap: usize, arche: PropArchetypeId) -> Self {
        let j = jitter.clamp(0.0, 0.5);
        Self { params: GridParams { cell, jitter: j, cap }, arche }
    }

    #[inline]
    fn rng_for(&self, seed: WorldSeed, chunk: &ChunkArea) -> ChaCha8Rng {
        // Stable per (seed, chunk, archetype)
        let mix = (seed.0)
            ^ ((chunk.coord.x as u64) << 16)
            ^ ((chunk.coord.z as u64) << 32)
            ^ ((self.arche.0 as u64) << 48)
            ^ 0xA5A5_5A5A_D3F0_1234u64;
        ChaCha8Rng::seed_from_u64(mix)
    }
}

impl PlacementStrategy for GridPlacement {
    fn place(&self, world_seed: WorldSeed, chunk: &ChunkArea, _archetype: PropArchetypeId) -> Vec<PlacementProbe> {
        let cell = self.params.cell.max(0.0001);
        let jitter = self.params.jitter;

        let size = chunk.max_xz - chunk.min_xz;
        let nx = (size.x / cell).floor().max(1.0) as i32;
        let nz = (size.y / cell).floor().max(1.0) as i32;

        let mut rng = self.rng_for(world_seed, chunk);
        let mut out = Vec::with_capacity((nx as usize) * (nz as usize));
        let mut local: u32 = 0;

        'outer: for j in 0..nz {
            for i in 0..nx {
                if out.len() >= self.params.cap { break 'outer; }

                // Cell center
                let cx = chunk.min_xz.x + (i as f32 + 0.5) * cell;
                let cz = chunk.min_xz.y + (j as f32 + 0.5) * cell;

                // Jitter
                let jx = (rng.random::<f32>() - 0.5) * 2.0 * (jitter * cell);
                let jz = (rng.random::<f32>() - 0.5) * 2.0 * (jitter * cell);

                let (x, z) = clamp_into_chunk(cx + jx, cz + jz, chunk);

                // Random yaw; keep scale = 1.0 (archetype can scale in spawn step)
                let rot_y = rng.random_range(0.0..std::f32::consts::TAU);

                out.push(make_probe(local, x, z, rot_y, 1.0));
                local = local.wrapping_add(1);
            }
        }
        out
    }
}
