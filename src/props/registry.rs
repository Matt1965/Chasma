// src/props/registry.rs
//! Data-driven prop archetypes + loader.

use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::core::{
    BiomeMask, CommonFilters, Footprint2D, HeightSnap, NavTag, PropArchetypeId,
};

// ---------- Public plugin to register asset+loader ----------

pub struct PropsRegistryAssetPlugin;

impl Plugin for PropsRegistryAssetPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<PropsRegistry>()
            .register_asset_loader(PropsRegistryLoader);
    }
}

// ---------- Placement strategy (data form) ----------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlacementStrategyDef {
    Grid {
        cell: f32,
        #[serde(default = "default_jitter")]
        jitter: f32,
        #[serde(default)]
        cap: Option<usize>,
    },
    Poisson {
        radius: f32,
        #[serde(default = "default_poisson_tries")]
        tries: u32,
        #[serde(default)]
        cap: Option<usize>,
    },
}

fn default_jitter() -> f32 {
    0.35
}
fn default_poisson_tries() -> u32 {
    16
}

// ---------- Render refs (data form) ----------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RenderRef {
    Scene { path: String },
    MeshMaterial { mesh: String, material: Option<String> },
    Lods { levels: Vec<LodLevelRef> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LodLevelRef {
    pub distance: f32, // start distance in meters
    pub repr: RenderRef,
}

// ---------- Archetype definition (data form) ----------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PropArchetypeDef {
    /// Unique human-readable name (used for lookup).
    pub name: String,

    /// Optional category hint (e.g., "vegetation", "building", "debris").
    #[serde(default)]
    pub category: Option<String>,

    /// Visual representation.
    pub render: RenderRef,

    /// Ground interaction & nav.
    #[serde(default)]
    pub footprint: Option<Footprint2D>,

    #[serde(default = "default_nav")]
    pub nav: NavTag,

    #[serde(default)]
    pub height_snap: HeightSnap,

    /// Common numeric/biome filters.
    #[serde(default)]
    pub filters: CommonFilters,

    /// Placement strategy parameters.
    pub placement: PlacementStrategyDef,

    /// Density multiplier (0..1 typical). Applied by strategies that use densities.
    #[serde(default = "default_density")]
    pub density: f32,

    /// Optional bitmask tags for fast inclusion/exclusion at query time.
    #[serde(default = "default_biome_mask")]
    pub biome_mask: BiomeMask,
}

fn default_nav() -> NavTag {
    NavTag::None
}
fn default_density() -> f32 {
    1.0
}
fn default_biome_mask() -> BiomeMask {
    BiomeMask(0)
}

// ---------- Runtime registry asset ----------

#[derive(Asset, TypePath, Clone)]
pub struct PropsRegistry {
    /// Ordered list; index in this vector is the `PropArchetypeId.0`.
    pub archetypes: Vec<PropArchetypeDef>,
    /// Name → index for quick lookups.
    pub name_to_index: HashMap<String, u32>,
}

impl PropsRegistry {
    pub fn index_of(&self, name: &str) -> Option<PropArchetypeId> {
        self.name_to_index.get(name).map(|&i| PropArchetypeId(i))
    }

    pub fn get(&self, id: PropArchetypeId) -> Option<&PropArchetypeDef> {
        self.archetypes.get(id.0 as usize)
    }
}

// ---------- Asset loader for `.props.ron` ----------

#[derive(Default)]
pub struct PropsRegistryLoader;

impl AssetLoader for PropsRegistryLoader {
    type Asset = PropsRegistry;
    type Settings = ();
    type Error = PropsRegistryLoadError;

    fn extensions(&self) -> &[&str] {
        &["props.ron"]
    }

    // NOTE: match the trait exactly: no explicit lifetimes, no LoadContext<'a>
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let defs: Vec<PropArchetypeDef> =
            ron::de::from_bytes(&bytes).map_err(|e| PropsRegistryLoadError::Ron(e.to_string()))?;

        let mut name_to_index = HashMap::with_capacity(defs.len());
        for (i, def) in defs.iter().enumerate() {
            if let Some(prev) = name_to_index.insert(def.name.clone(), i as u32) {
                return Err(PropsRegistryLoadError::DuplicateName {
                    name: def.name.clone(),
                    first: prev,
                    second: i as u32,
                });
            }
        }

        Ok(PropsRegistry { archetypes: defs, name_to_index })
    }
}


// ---------- Loader errors ----------

#[derive(thiserror::Error, Debug)]
pub enum PropsRegistryLoadError {
    #[error("I/O while reading registry: {0}")]
    Io(#[from] std::io::Error),
    #[error("RON parse error: {0}")]
    Ron(String),
    #[error("Duplicate archetype name '{name}' (first idx {first}, second idx {second})")]
    DuplicateName { name: String, first: u32, second: u32 },
}
