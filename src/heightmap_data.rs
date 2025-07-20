// src/heightmap_data.rs

use bevy::prelude::*;
use bevy::math::{Vec2, UVec2};
use image::GrayImage;

/// Holds your heightmap image plus size/origin metadata.
#[derive(Resource)]
pub struct HeightmapData {
    /// The raw grayscale heightmap (0–255)
    pub image: GrayImage,
    /// Pixel resolution (width, height)
    pub resolution: UVec2,
    /// World‐space size covered (X extent, Z extent)
    pub size: Vec2,
    /// How tall a “1.0” pixel becomes in world‐Y
    pub height_scale: f32,
    /// World‐space (X,Z) of the heightmap’s (u=0,v=0) corner
    pub origin: Vec2,
}

impl HeightmapData {
    /// Sample the terrain height at world‐space (x, z) with bilinear filtering.
    pub fn sample_height(&self, world_x: f32, world_z: f32) -> f32 {
        // 1) Shift into local [0..size] by subtracting origin.
        let local_x = (world_x - self.origin.x).clamp(0.0, self.size.x);
        let local_z = (world_z - self.origin.y).clamp(0.0, self.size.y);

        // 2) Normalize to UV [0..1].
        let u = (local_x / self.size.x).clamp(0.0, 1.0);
        let v = (local_z / self.size.y).clamp(0.0, 1.0);

        // 3) Convert UV → floating‐point pixel coords.
        let max_x = (self.resolution.x - 1) as f32;
        let max_y = (self.resolution.y - 1) as f32;
        let px_f = u * max_x;
        let py_f = v * max_y;

        // 4) Find the four surrounding integer pixels.
        let x0 = px_f.floor() as u32;
        let y0 = py_f.floor() as u32;
        let x1 = (x0 + 1).min(self.resolution.x - 1);
        let y1 = (y0 + 1).min(self.resolution.y - 1);

        let dx = px_f - x0 as f32;
        let dy = py_f - y0 as f32;

        // 5) Fetch their heights (0..255).
        let h00 = self.image.get_pixel(x0, y0)[0] as f32;
        let h10 = self.image.get_pixel(x1, y0)[0] as f32;
        let h01 = self.image.get_pixel(x0, y1)[0] as f32;
        let h11 = self.image.get_pixel(x1, y1)[0] as f32;

        // 6) Bilinear interpolate.
        let h0 = h00 * (1.0 - dx) + h10 * dx;
        let h1 = h01 * (1.0 - dx) + h11 * dx;
        let h  = h0  * (1.0 - dy) + h1  * dy;

        // 7) Normalize (0..255→0.0..1.0) and apply height_scale.
        (h / 255.0) * self.height_scale
    }
}
