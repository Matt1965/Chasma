use bevy::prelude::*;
use crate::props::core::{HeightSampler, SlopeSampler};
use crate::props::vegetation::systems::on_chunk_loaded_spawn_vegetation;

#[derive(Resource, Default)]
pub struct FlatGround { pub y: f32 }
impl HeightSampler for FlatGround { fn sample_height(&self, _x:f32,_z:f32)->f32 { self.y } }
impl SlopeSampler for FlatGround {}

pub struct VegetationPlugin;
impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FlatGround>()
           .add_systems(Update, on_chunk_loaded_spawn_vegetation);
    }
}