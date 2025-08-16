// src/heightmap_data.rs
use bevy::math::{UVec2, Vec2};
use bevy::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Global terrain metadata
#[derive(Resource, Clone)]
pub struct HeightmapData {
    /// Size of entire terrain in world units (X,Z)
    pub size: Vec2,
    /// World-space origin of the terrain (bottom-left corner)
    pub origin: Vec2,
    /// World meters per full normalized height (1.0)
    pub height_scale: f32,
    /// Size of a single chunk in world units
    pub chunk_size: Vec2,
    /// Normalization range for RAW values (before scaling)
    /// If you exported "Use Full Range", leave as (0.0, 65535.0).
    pub raw_minmax: (f32, f32),
    /// If true, flip V so that Gaea's top-left origin maps to world bottom-left.
    pub flip_v: bool,
}

impl Default for HeightmapData {
    fn default() -> Self {
        Self {
            size: Vec2::ZERO,
            origin: Vec2::ZERO,
            height_scale: 1.0,
            chunk_size: Vec2::splat(1.0),
            raw_minmax: (0.0, 65535.0),
            flip_v: true,
        }
    }
}

/// A single 16-bit RAW tile in memory
#[derive(Clone)]
pub struct Tile16 {
    pub res: UVec2,
    pub data: Arc<Vec<u16>>, // row-major, little-endian source
}

impl Tile16 {
    #[inline]
    pub fn get_clamped(&self, x: i32, y: i32) -> u16 {
        let xi = x.clamp(0, self.res.x as i32 - 1) as u32;
        let yi = y.clamp(0, self.res.y as i32 - 1) as u32;
        // Row-major
        self.data[(yi * self.res.x + xi) as usize]
    }
}

/// Cache & IO config for RAW tiles
#[derive(Resource)]
pub struct HeightTileCache {
    /// Loaded tiles by (chunk_x, chunk_z)
    pub tiles: HashMap<(i32, i32), Tile16>,
    /// Folder where RAW tiles live
    pub folder: PathBuf,
    /// Per-tile pixel resolution (e.g. 1024x1024)
    pub tile_resolution: UVec2,
    /// Filename pieces: `{prefix}_y{z}_x{x}{ext}`
    pub filename_prefix: String, // e.g. "Heightmap"
    pub filename_ext: String,    // e.g. ".raw16"
}

impl HeightTileCache {
    pub fn new(folder: impl AsRef<Path>, tile_resolution: UVec2) -> Self {
        Self {
            tiles: HashMap::new(),
            folder: folder.as_ref().to_path_buf(),
            tile_resolution,
            filename_prefix: "Heightmap".to_string(),
            filename_ext: ".raw16".to_string(),
        }
    }

    fn tile_path(&self, cx: i32, cz: i32) -> PathBuf {
        let name = format!(
            "{}_y{}_x{}{}",
            self.filename_prefix, cz, cx, self.filename_ext
        );
        self.folder.join(name)
    }

    fn load_raw16(&self, path: &Path) -> Option<Tile16> {
        let expected_pixels = (self.tile_resolution.x as usize) * (self.tile_resolution.y as usize);
        let expected_bytes = expected_pixels * 2;

        let mut f = File::open(path).ok()?;
        let len = f.metadata().ok()?.len() as usize;
        if len < expected_bytes {
            return None;
        }

        let mut buf = vec![0u8; expected_bytes];
        f.seek(SeekFrom::Start(0)).ok()?;
        f.read_exact(&mut buf).ok()?;

        // Convert little-endian bytes -> u16
        let mut out: Vec<u16> = Vec::with_capacity(expected_pixels);
        for i in (0..expected_bytes).step_by(2) {
            let lo = buf[i];
            let hi = buf[i + 1];
            out.push(u16::from_le_bytes([lo, hi]));
        }

        Some(Tile16 {
            res: self.tile_resolution,
            data: Arc::new(out),
        })
    }

    fn get_or_load(&mut self, cx: i32, cz: i32) -> Option<&Tile16> {
        if !self.tiles.contains_key(&(cx, cz)) {
            let path = self.tile_path(cx, cz);
            let tile = self.load_raw16(&path)?;
            self.tiles.insert((cx, cz), tile);
        }
        self.tiles.get(&(cx, cz))
    }

    /// Public API: return a cloned snapshot (Arc-backed) of the requested tile.
    pub fn fetch_tile(&mut self, cx: i32, cz: i32) -> Option<Tile16> {
        self.get_or_load(cx, cz).cloned()
    }
}

/// Bilinear-sample the terrain height (world units) at (world_x, world_z).
/// Returns None if outside the global terrain or the needed tile is missing.
/// Assumes 1 tile == 1 chunk in world span (CHUNK_SIZE).
pub fn sample_height(
    world_x: f32,
    world_z: f32,
    data: &HeightmapData,
    cache: &mut HeightTileCache,
) -> Option<f32> {
    // Convert to local terrain coords
    let lx = world_x - data.origin.x;
    let mut lz = world_z - data.origin.y;

    // Out of bounds -> None
    if lx < 0.0 || lz < 0.0 || lx >= data.size.x || lz >= data.size.y {
        return None;
    }

    // Handle Gaea top-left origin if requested
    if data.flip_v {
        lz = data.size.y - lz;
    }

    // Which tile/chunk?
    let cx = (lx / data.chunk_size.x).floor() as i32;
    let cz = (lz / data.chunk_size.y).floor() as i32;

    // Local position inside this tile in world units
    let local_x = lx - (cx as f32 * data.chunk_size.x);
    let local_z = lz - (cz as f32 * data.chunk_size.y);

    // Get the RAW16 tile snapshot (Arc-backed)
    let tile = cache.fetch_tile(cx, cz)?;

    // Normalized UV in [0,1] inside this tile
    let u = (local_x / data.chunk_size.x).clamp(0.0, 1.0);
    let v = (local_z / data.chunk_size.y).clamp(0.0, 1.0);

    // Convert to pixel space for bilinear
    let max_x = (tile.res.x.saturating_sub(1)) as i32;
    let max_y = (tile.res.y.saturating_sub(1)) as i32;

    let px_f = u * max_x as f32;
    let py_f = v * max_y as f32;

    let x0 = px_f.floor() as i32;
    let y0 = py_f.floor() as i32;
    let x1 = (x0 + 1).min(max_x);
    let y1 = (y0 + 1).min(max_y);

    let dx = px_f - x0 as f32;
    let dy = py_f - y0 as f32;

    // RAW16 samples (clamped)
    let s00 = tile.get_clamped(x0, y0) as f32;
    let s10 = tile.get_clamped(x1, y0) as f32;
    let s01 = tile.get_clamped(x0, y1) as f32;
    let s11 = tile.get_clamped(x1, y1) as f32;

    // Bilinear in RAW space
    let a = s00 * (1.0 - dx) + s10 * dx;
    let b = s01 * (1.0 - dx) + s11 * dx;
    let raw = a * (1.0 - dy) + b * dy;

    // Normalize and scale to world height
    let (rmin, rmax) = data.raw_minmax;
    let norm = if rmax > rmin {
        ((raw - rmin) / (rmax - rmin)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    Some(norm * data.height_scale)
}
