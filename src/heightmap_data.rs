// src/heightmap_data.rs

use bevy::prelude::*;
use bevy::math::{Vec2, UVec2};
use image::{GrayImage, RgbaImage};
use std::sync::Arc;

/// Holds your heightmap + color map + size/origin metadata.
/// Both images live entirely on the CPU; shipping to GPU happens
/// only via small per-chunk crops in async_chunk_loader.rs.
#[derive(Resource)]
pub struct HeightmapData {
    /// Grayscale heightmap (0–255 per pixel).
    pub height_image: Arc<GrayImage>,
    /// RGBA color map (full-world).
    pub color_image:  Arc<RgbaImage>,
    /// Pixel resolution of both images (width, height).
    pub resolution:   UVec2,
    /// World-space size covered by the map in X (width) and Z (depth).
    pub size:         Vec2,
    /// How tall a “1.0” gray value becomes in world-Y.
    pub height_scale: f32,
    /// World-space (X, Z) of the (u=0,v=0) corner of your images.
    pub origin:       Vec2,
}

impl HeightmapData {
    /// Bilinear-samples the height at (world_x, world_z).
    pub fn sample_height(&self, world_x: f32, world_z: f32) -> f32 {
        // identical to what you already had…
        let lx = (world_x - self.origin.x).clamp(0.0, self.size.x);
        let lz = (world_z - self.origin.y).clamp(0.0, self.size.y);
        let u  = (lx / self.size.x).clamp(0.0, 1.0);
        let v  = (lz / self.size.y).clamp(0.0, 1.0);
        let max_x = (self.resolution.x - 1) as f32;
        let max_z = (self.resolution.y - 1) as f32;
        let px_f = u * max_x;
        let pz_f = v * max_z;
        let x0 = px_f.floor() as u32;
        let z0 = pz_f.floor() as u32;
        let x1 = (x0 + 1).min(self.resolution.x - 1);
        let z1 = (z0 + 1).min(self.resolution.y - 1);
        let dx = px_f - x0 as f32;
        let dz = pz_f - z0 as f32;
        let h00 = self.height_image.get_pixel(x0, z0)[0] as f32;
        let h10 = self.height_image.get_pixel(x1, z0)[0] as f32;
        let h01 = self.height_image.get_pixel(x0, z1)[0] as f32;
        let h11 = self.height_image.get_pixel(x1, z1)[0] as f32;
        let h0  = h00 * (1.0 - dx) + h10 * dx;
        let h1  = h01 * (1.0 - dx) + h11 * dx;
        let h   = h0 * (1.0 - dz) + h1 * dz;
        (h / 255.0) * self.height_scale
    }
}
