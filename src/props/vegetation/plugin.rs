// src/props/vegetation/plugin.rs
use bevy::prelude::*;
use crate::props::core::{HeightSampler, SlopeSampler};
use super::systems::spawn_veg_on_chunk_loaded;
use crate::heightmap_data::{HeightmapData, HeightTileCache};
use super::sampler::TerrainHeightSampler;

#[derive(Resource, Default)]
pub struct FlatGround { pub y: f32 }
impl HeightSampler for FlatGround { fn sample_height(&self, _x:f32,_z:f32)->f32 { self.y } }
impl SlopeSampler for FlatGround {}

fn registry_ready(
    handle: Res<crate::props::plugin::PropsRegistryHandle>,
    regs: Res<Assets<crate::props::registry::PropsRegistry>>,
) -> bool {
    regs.get(&handle.0).is_some()
}

pub struct VegetationPlugin;
impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FlatGround>()
            .add_systems(Startup, init_terrain_height_sampler)
            .add_systems(Update, spawn_veg_on_chunk_loaded.run_if(registry_ready));
    }
}

fn init_terrain_height_sampler(
    mut commands: Commands,
    data: Res<HeightmapData>,
    cache: Res<HeightTileCache>,
) {
    // Make a `'static` sampler from the current terrain config
    let sampler = TerrainHeightSampler::new_from(&data, &cache);
    commands.insert_resource(sampler);
}
