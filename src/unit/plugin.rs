use bevy::prelude::*;
use bevy::ecs::schedule::common_conditions::resource_exists;
use crate::heightmap_data::{HeightmapData, HeightTileCache};
use crate::unit::systems::{
    spawn_unit, click_to_move, move_units, grounding_system, record_previous_system, collision_system,
};
use crate::state::GameState;

pub struct UnitPlugin;

impl Plugin for UnitPlugin {
    fn build(&self, app: &mut App) {
        app
            // Run once when both resources are present (inserted by TerrainPlugin)
            .add_systems(
                Startup,
                spawn_unit
                    .run_if(resource_exists::<HeightmapData>)
                    .run_if(resource_exists::<HeightTileCache>),
            )
            .add_systems(
                Update,
                (
                    record_previous_system.before(move_units).run_if(in_state(GameState::Running)),
                    click_to_move.run_if(in_state(GameState::Running)),
                    move_units.after(click_to_move).run_if(in_state(GameState::Running)),
                    grounding_system.after(move_units).run_if(in_state(GameState::Running)),
                    collision_system.after(grounding_system).run_if(in_state(GameState::Running)),
                ),
            );
    }
}
