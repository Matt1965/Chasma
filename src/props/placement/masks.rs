// src/props/placement/masks.rs
//! World-space mask sampling helpers (density/biome/etc.) from Bevy `Image`s.
//! These are utilities; integrate from streaming to bias/accept placements.

use bevy::image::Image;
use bevy::prelude::*;

/// Defines the world-space rectangle a mask covers.
#[derive(Clone, Copy, Debug)]
pub struct MaskWorld {
    pub min_xz: Vec2,
    pub max_xz: Vec2,
}

impl MaskWorld {
    #[inline]
    pub fn size(&self) -> Vec2 { self.max_xz - self.min_xz }

    #[inline]
    pub fn to_uv(&self, x: f32, z: f32) -> Vec2 {
        let s = self.size();
        Vec2::new(
            ((x - self.min_xz.x) / s.x).clamp(0.0, 1.0),
            ((z - self.min_xz.y) / s.y).clamp(0.0, 1.0),
        )
    }
}

/// Sample a single-channel (grayscale) `Image` as 0..1 using nearest or bilinear.
/// Assumes the channel of interest is R (luminance-like).
#[derive(Clone, Copy, Debug)]
pub enum SampleMode { Nearest, Linear }

pub fn sample_mask_01(img: &Image, world: MaskWorld, x: f32, z: f32, mode: SampleMode) -> f32 {
    let uv = world.to_uv(x, z);

    let w = img.size().x as usize;
    let h = img.size().y as usize;

    // CPU-side bytes
    let data: &[u8] = img
        .data
        .as_ref()
        .expect("Image has no CPU-side data; ensure it was created/loaded with CPU readback")
        .as_slice();

    // Bytes per texel (e.g., R8=1, Rgba8=4). Fall back to 4 if unknown.
    let pitch: usize = img
        .texture_descriptor
        .format
        .block_copy_size(None)
        .unwrap_or(4) as usize;

    debug_assert!(pitch >= 1);

    match mode {
        SampleMode::Nearest => {
            let u = (uv.x * (w as f32 - 1.0)).round() as usize;
            let v = (uv.y * (h as f32 - 1.0)).round() as usize;
            let idx = (v * w + u) * pitch;
            (data[idx] as f32) / 255.0
        }
        SampleMode::Linear => {
            // Bilinear
            let fx = uv.x * (w as f32 - 1.0);
            let fy = uv.y * (h as f32 - 1.0);
            let x0 = fx.floor() as i32;
            let y0 = fy.floor() as i32;
            let x1 = (x0 + 1).clamp(0, w as i32 - 1);
            let y1 = (y0 + 1).clamp(0, h as i32 - 1);
            let tx = fx - x0 as f32;
            let ty = fy - y0 as f32;

            let idx = |ix: i32, iy: i32| -> usize {
                ((iy as usize) * w + (ix as usize)) * pitch
            };

            let s00 = data[idx(x0, y0)] as f32 / 255.0;
            let s10 = data[idx(x1, y0)] as f32 / 255.0;
            let s01 = data[idx(x0, y1)] as f32 / 255.0;
            let s11 = data[idx(x1, y1)] as f32 / 255.0;

            let a = s00 * (1.0 - tx) + s10 * tx;
            let b = s01 * (1.0 - tx) + s11 * tx;
            a * (1.0 - ty) + b * ty
        }
    }
}

/// Stochastic accept/reject using a [0,1] density value and a deterministic random `r` in [0,1).
#[inline]
pub fn accept_by_density(density_01: f32, r01: f32) -> bool {
    r01 < density_01.clamp(0.0, 1.0)
}
