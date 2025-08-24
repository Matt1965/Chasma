// src/props/vegetation/rules.rs
//! Vegetation placement rules. Currently: simple **height (altitude) bounds** for trees.

use crate::props::core::CommonFilters;

/// Inclusive altitude gate.
#[derive(Clone, Copy, Debug)]
pub struct HeightRule {
    pub min: Option<f32>,
    pub max: Option<f32>,
}
impl HeightRule {
    #[inline]
    pub fn contains(&self, h: f32) -> bool {
        if let Some(mn) = self.min { if h < mn { return false; } }
        if let Some(mx) = self.max { if h > mx { return false; } }
        true
    }
}

/// Build a HeightRule from CommonFilters (uses altitude_min / altitude_max).
#[inline]
pub fn height_rule_from_filters(filters: &CommonFilters) -> HeightRule {
    HeightRule {
        min: filters.altitude_min,
        max: filters.altitude_max,
    }
}
