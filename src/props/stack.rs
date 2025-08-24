// src/props/stack.rs
use bevy::prelude::*;
use crate::props::plugin::PropsPlugin;
use crate::props::streaming::StreamingPlugin;
use crate::props::vegetation::plugin::VegetationPlugin;

pub struct PropsStackPlugin;
impl Plugin for PropsStackPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PropsPlugin)        // infra: registry + events + seed
           .add_plugins(StreamingPlugin)    // LOD / despawn
           .add_plugins(VegetationPlugin);  // trees/grass spawner
    }
}
