// src/props/core.rs
//! Core types/traits for deterministic, chunk-aware prop placement.
//! Keep this file dependency-light; it should compile before any placement/streaming impls.

use bevy::prelude::*; // Vec2, Vec3, Quat
use serde::{Deserialize, Serialize}; // for registry (de)serialization

// ---------- World, chunks, ids ----------

/// Global world seed; changing this reshuffles all procedural props.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorldSeed(pub u64);

/// Integer chunk coordinate in XZ. Matches your terrain tiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkCoord {
    pub x: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub const fn new(x: i32, z: i32) -> Self { Self { x, z } }
}

/// Axis-aligned world-space bounds used for placement within a chunk.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ChunkArea {
    pub coord: ChunkCoord,
    pub min_xz: Vec2,
    pub max_xz: Vec2,
}

impl ChunkArea {
    pub fn size(&self) -> Vec2 { self.max_xz - self.min_xz }
    pub fn contains_xz(&self, p: Vec2) -> bool {
        p.x >= self.min_xz.x && p.x <= self.max_xz.x &&
        p.y >= self.min_xz.y && p.y <= self.max_xz.y
    }
}

/// Index of an archetype in the registry (stable during a session).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PropArchetypeId(pub u32);

/// Local index for a placement inside a chunk (stable given same seed/strategy).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalSpawnIndex(pub u32);

/// Globally-unique stable identity for a spawned prop instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PropId {
    pub chunk: ChunkCoord,
    pub local: LocalSpawnIndex,
    pub archetype: PropArchetypeId,
}

impl PropId {
    pub const fn new(chunk: ChunkCoord, local: LocalSpawnIndex, archetype: PropArchetypeId) -> Self {
        Self { chunk, local, archetype }
    }
}

// ---------- Footprints & nav tags ----------

/// 2D ground footprint (XZ). Use for spacing, overlap checks, and nav blocking.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Footprint2D {
    /// Circle with radius (meters).
    Circle { r: f32 },
    /// Axis-aligned rectangle with half-extents (meters).
    Rect { half: Vec2 },
    /// Arbitrary polygon in local prop space (counter-clockwise).
    Poly { points: Vec<Vec2> },
}

/// Navigation impact of a prop.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum NavTag {
    None,
    /// Fully blocks navigation within `footprint`.
    Blocker,
    /// Raises traversal cost (>= 1.0 recommended).
    Cost(f32),
}

/// Height snapping policy when placing on terrain.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HeightSnap {
    /// Added after sampling ground height.
    pub y_offset: f32,
    /// If true, attempt to align local up with ground normal (when available).
    pub align_to_normal: bool,
    /// Optional clamps (useful to avoid underwater/too-high placements).
    pub clamp_min: Option<f32>,
    pub clamp_max: Option<f32>,
}

impl Default for HeightSnap {
    fn default() -> Self {
        Self { y_offset: 0.0, align_to_normal: false, clamp_min: None, clamp_max: None }
    }
}

// ---------- Biomes / tags (lightweight bitmask) ----------

/// Bitmask of biome tags (fast filter). Define your own constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BiomeMask(pub u32);

impl BiomeMask {
    pub const NONE: Self = Self(0);
    pub fn contains(self, other: Self) -> bool { (self.0 & other.0) == other.0 }
    pub fn any(self, other: Self) -> bool { (self.0 & other.0) != 0 }
}

// ---------- Placement I/O ----------

/// Raw placement sample before height snap (XZ only).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PlacementProbe {
    pub x: f32,
    pub z: f32,
    /// Yaw (radians) around +Y.
    pub rot_y: f32,
    /// Uniform scale.
    pub scale: f32,
    /// Stable local index for ID composition.
    pub local_index: LocalSpawnIndex,
}

/// Finalized placement (full transform + identity).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PlacementResult {
    pub id: PropId,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

// ---------- Traits: sampling, placement, nav export ----------

/// Terrain height sampler (required).
pub trait HeightSampler: Send + Sync + 'static {
    /// Returns ground height (Y) at world XZ.
    fn sample_height(&self, x: f32, z: f32) -> f32;
}

/// Optional terrain normal/slope sampling.
pub trait SlopeSampler: Send + Sync + 'static {
    /// Returns surface normal (unit) at world XZ, if available.
    fn sample_normal(&self, x: f32, z: f32) -> Option<Vec3> { let _ = (x, z); None }
    /// Returns slope in degrees at world XZ, if available.
    fn slope_deg(&self, x: f32, z: f32) -> Option<f32> { let _ = (x, z); None }
}

/// Strategy that deterministically produces candidate placements inside a chunk.
pub trait PlacementStrategy: Send + Sync + 'static {
    /// Generate probes; must be deterministic for identical inputs.
    fn place(
        &self,
        world_seed: WorldSeed,
        chunk: &ChunkArea,
        archetype: PropArchetypeId,
    ) -> Vec<PlacementProbe>;
}

/// Receives nav blockers/costs for export to your nav solution.
pub trait NavSink: Send + Sync + 'static {
    fn add_blocker(&mut self, _id: PropId, _footprint: &Footprint2D) {}
    fn add_cost(&mut self, _id: PropId, _footprint: &Footprint2D, _multiplier: f32) {}
}

// ---------- Filtering & finalize helpers ----------

/// Simple numeric filters commonly used by vegetation/debris.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct CommonFilters {
    pub altitude_min: Option<f32>,
    pub altitude_max: Option<f32>,
    pub slope_min_deg: Option<f32>,
    pub slope_max_deg: Option<f32>,
    pub biome_mask_any: Option<BiomeMask>, // pass if any bit overlaps
}

impl Default for CommonFilters {
    fn default() -> Self {
        Self {
            altitude_min: None,
            altitude_max: None,
            slope_min_deg: None,
            slope_max_deg: None,
            biome_mask_any: None,
        }
    }
}

/// Check basic filters using available samplers.
/// `biome_at` is optional; pass `None` to ignore biome filtering.
pub fn passes_common_filters(
    probe: &PlacementProbe,
    height: f32,
    samplers: (&dyn SlopeSampler,),
    filters: &CommonFilters,
    biome_at: Option<impl Fn(f32, f32) -> BiomeMask>,
) -> bool {
    // Altitude
    if let Some(min_h) = filters.altitude_min { if height < min_h { return false; } }
    if let Some(max_h) = filters.altitude_max { if height > max_h { return false; } }

    // Slope
    if filters.slope_min_deg.is_some() || filters.slope_max_deg.is_some() {
        if let Some(slope) = samplers.0.slope_deg(probe.x, probe.z) {
            if let Some(min_s) = filters.slope_min_deg { if slope < min_s { return false; } }
            if let Some(max_s) = filters.slope_max_deg { if slope > max_s { return false; } }
        }
    }

    // Biome
    if let Some(mask_req) = filters.biome_mask_any {
        if let Some(f) = biome_at {
            if !mask_req.any(f(probe.x, probe.z)) { return false; }
        }
    }

    true
}

/// Apply height snap + optional normal alignment to produce a final transform.
pub fn finalize_transform(
    probe: &PlacementProbe,
    hs: &dyn HeightSampler,
    sn: &dyn SlopeSampler,
    snap: HeightSnap,
) -> (Vec3, Quat, Vec3) {
    let y0 = hs.sample_height(probe.x, probe.z);
    let mut y = y0 + snap.y_offset;
    if let Some(min) = snap.clamp_min { if y < min { y = min; } }
    if let Some(max) = snap.clamp_max { if y > max { y = max; } }

    // Rotation: yaw + optional ground normal alignment
    let yaw = Quat::from_rotation_y(probe.rot_y);
    let rot = if snap.align_to_normal {
        if let Some(n) = sn.sample_normal(probe.x, probe.z) {
            // Align +Y to ground normal (approximate): shortest-arc rotation.
            let up = Vec3::Y;
            let axis = up.cross(n);
            let dot = up.dot(n).clamp(-1.0, 1.0);
            let angle = dot.acos();
            Quat::from_axis_angle(axis.normalize_or_zero(), angle) * yaw
        } else {
            yaw
        }
    } else {
        yaw
    };

    let translation = Vec3::new(probe.x, y, probe.z);
    let scale = Vec3::splat(probe.scale);
    (translation, rot, scale)
}

/// Compose a stable `PropId` from inputs.
pub fn make_prop_id(chunk: ChunkCoord, local_index: u32, archetype_index: u32) -> PropId {
    PropId::new(chunk, LocalSpawnIndex(local_index), PropArchetypeId(archetype_index))
}
