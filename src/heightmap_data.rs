use bevy::math::{UVec2, Vec2, Vec3};
use bevy::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Global terrain metadata
#[derive(Resource, Clone)]
pub struct HeightmapData {
    pub size: Vec2,
    pub origin: Vec2,
    pub height_scale: f32,
    pub chunk_size: Vec2,
    pub raw_minmax: (f32, f32),
}

impl Default for HeightmapData {
    fn default() -> Self {
        Self {
            size: Vec2::ZERO,
            origin: Vec2::ZERO,
            height_scale: 1.0,
            chunk_size: Vec2::splat(1.0),
            raw_minmax: (0.0, 65535.0),
        }
    }
}

/// A single 16-bit RAW tile in memory
#[derive(Clone)]
pub struct Tile16 {
    pub res: UVec2,
    pub data: Arc<Vec<u16>>,
}

impl Tile16 {
    #[inline]
    pub fn get_clamped(&self, x: i32, y: i32) -> u16 {
        let xi = x.clamp(0, self.res.x as i32 - 1) as u32;
        let yi = y.clamp(0, self.res.y as i32 - 1) as u32;
        self.data[(yi * self.res.x + xi) as usize]
    }
}

/// IO + in-memory cache for RAW tiles
#[derive(Resource, Clone)]
pub struct HeightTileCache {
    pub tiles: HashMap<(i32, i32), Tile16>,
    pub folder: PathBuf,
    pub tile_resolution: UVec2,
    pub filename_prefix: String,
    pub filename_ext: String,
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
        let expected_pixels = (self.tile_resolution.x * self.tile_resolution.y) as usize;
        let expected_bytes = expected_pixels * 2;

        let mut f = File::open(path).ok()?;
        if f.metadata().ok()?.len() < expected_bytes as u64 {
            return None;
        }

        let mut buf = vec![0u8; expected_bytes];
        f.seek(SeekFrom::Start(0)).ok()?;
        f.read_exact(&mut buf).ok()?;

        let data = buf
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect::<Vec<_>>();

        Some(Tile16 {
            res: self.tile_resolution,
            data: Arc::new(data),
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

    pub fn fetch_tile(&mut self, cx: i32, cz: i32) -> Option<Tile16> {
        self.get_or_load(cx, cz).cloned()
    }
}

/// Bilinear height sampling in world space
pub fn sample_height(
    world_x: f32,
    world_z: f32,
    data: &HeightmapData,
    cache: &mut HeightTileCache,
) -> Option<f32> {
    let lx = world_x - data.origin.x;
    let lz = world_z - data.origin.y;

    if lx < 0.0 || lz < 0.0 || lx >= data.size.x || lz >= data.size.y {
        return None;
    }

    let cx = (lx / data.chunk_size.x).floor() as i32;
    let cz = (lz / data.chunk_size.y).floor() as i32;

    let local_x = lx - (cx as f32 * data.chunk_size.x);
    let local_z = lz - (cz as f32 * data.chunk_size.y);

    let tile = cache.fetch_tile(cx, cz)?;
    let u = (local_x / data.chunk_size.x).clamp(0.0, 1.0);
    let v = (local_z / data.chunk_size.y).clamp(0.0, 1.0);

    let max_x = (tile.res.x - 1) as i32;
    let max_y = (tile.res.y - 1) as i32;

    let px_f = u * max_x as f32;
    let py_f = v * max_y as f32;

    let x0 = px_f.floor() as i32;
    let y0 = py_f.floor() as i32;
    let x1 = (x0 + 1).min(max_x);
    let y1 = (y0 + 1).min(max_y);

    let dx = px_f - x0 as f32;
    let dy = py_f - y0 as f32;

    let s00 = tile.get_clamped(x0, y0) as f32;
    let s10 = tile.get_clamped(x1, y0) as f32;
    let s01 = tile.get_clamped(x0, y1) as f32;
    let s11 = tile.get_clamped(x1, y1) as f32;

    let a = s00 * (1.0 - dx) + s10 * dx;
    let b = s01 * (1.0 - dx) + s11 * dx;
    let raw = a * (1.0 - dy) + b * dy;

    let (rmin, rmax) = data.raw_minmax;
    let norm = if rmax > rmin {
        ((raw - rmin) / (rmax - rmin)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    Some(norm * data.height_scale)
}

/// Adapter that satisfies HeightSampler and SlopeSampler traits
#[derive(Clone)]
pub struct TerrainSampleAdapter {
    pub data: HeightmapData,
    pub cache: HeightTileCache,
}

impl TerrainSampleAdapter {
    pub fn new(data: &HeightmapData, cache: &HeightTileCache) -> Self {
        Self {
            data: data.clone(),
            cache: cache.clone(),
        }
    }
}

impl HeightSampler for TerrainSampleAdapter {
    fn sample_height(&self, x: f32, z: f32) -> f32 {
        sample_height(x, z, &self.data, &mut self.cache.clone()).unwrap_or(0.0)
    }
}

impl SlopeSampler for TerrainSampleAdapter {
    fn sample_normal(&self, x: f32, z: f32) -> Option<Vec3> {
        let d = 0.25;

        let h = |x, z| sample_height(x, z, &self.data, &mut self.cache.clone()).unwrap_or(0.0);

        let hx1 = h(x + d, z);
        let hx0 = h(x - d, z);
        let hz1 = h(x, z + d);
        let hz0 = h(x, z - d);

        let dx = (hx1 - hx0) / (2.0 * d);
        let dz = (hz1 - hz0) / (2.0 * d);

        Some(Vec3::new(-dx, 1.0, -dz).normalize())
    }
}

/// Trait for height sampling used in placement
pub trait HeightSampler: Send + Sync + 'static {
    fn sample_height(&self, x: f32, z: f32) -> f32;
}

/// Trait for slope sampling (optional)
pub trait SlopeSampler: Send + Sync + 'static {
    fn sample_normal(&self, x: f32, z: f32) -> Option<Vec3>;

    fn slope_deg(&self, x: f32, z: f32) -> Option<f32> {
        self.sample_normal(x, z)
            .map(|n| n.angle_between(Vec3::Y).to_degrees())
    }
}
