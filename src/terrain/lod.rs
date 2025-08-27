use bevy::prelude::*;

/// LoD tiers
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LodLevel {
    Near,
    Mid,
    Far,
}

impl LodLevel {
    /// Vertex grid (odd counts so edges line up)
    pub fn grid_res(self) -> UVec2 {
        match self {
            LodLevel::Near => UVec2::new(129, 129),
            LodLevel::Mid  => UVec2::new(65, 65),
            LodLevel::Far  => UVec2::new(33, 33),
        }
    }

    /// Distance thresholds → LoD (tweak to taste)
    pub fn pick(distance: f32) -> Self {
        if distance < 600.0 { LodLevel::Near }
        else if distance < 1200.0 { LodLevel::Mid }
        else { LodLevel::Far }
    }
}

/// Tag the chunk entity with its LoD
#[derive(Component, Debug, Clone, Copy)]
pub struct ChunkLod(pub LodLevel);
