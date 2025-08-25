use bevy::prelude::*;
use super::lod::update_lod_groups;
use super::chunk_bind::despawn_chunk_props;

/// Small plugin that wires despawn + LOD.
pub struct StreamingPlugin;
impl Plugin for StreamingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (update_lod_groups, despawn_chunk_props));
    }
}