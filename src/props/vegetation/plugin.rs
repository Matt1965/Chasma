use bevy::prelude::*;
use crate::heightmap_data::{HeightmapData, HeightTileCache, TerrainSampleAdapter};

#[derive(Resource, Default)]
pub struct FlatGround {
    pub y: f32,
}

impl crate::heightmap_data::HeightSampler for FlatGround {
    fn sample_height(&self, _x: f32, _z: f32) -> f32 {
        self.y
    }
}

impl crate::heightmap_data::SlopeSampler for FlatGround {
    fn sample_normal(&self, _x: f32, _z: f32) -> Option<Vec3> {
        Some(Vec3::Y)
    }
}

#[derive(Resource, Clone)]
pub struct VegSampler(pub TerrainSampleAdapter);

pub struct VegetationPlugin;
impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<FlatGround>()
            .add_systems(Startup, init_terrain_height_sampler);
            // NOTE: Removed sync vegetation system to avoid CPU freeze
    }
}

fn init_terrain_height_sampler(
    mut commands: Commands,
    data: Res<HeightmapData>,
    cache: Res<HeightTileCache>,
) {
    let sampler = TerrainSampleAdapter::new(&data, &cache);
    commands.insert_resource(VegSampler(sampler));
}
