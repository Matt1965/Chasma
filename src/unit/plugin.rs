use bevy::prelude::*;
use crate::terrain::load_heightmap_data;
use crate::state::GameState;
use crate::unit::systems::{
    spawn_unit,
    click_to_move,
    move_units,
    grounding_system,
    record_previous_system,   // ← NEW
    collision_system,         // ← NEW
};

/// Groups all your unit logic in one plugin.
pub struct UnitPlugin;

impl Plugin for UnitPlugin {
    fn build(&self, app: &mut App) {
        app
            // must wait until the heightmap resource exists
            .add_systems(
                Startup,
                spawn_unit.after(load_heightmap_data),
            )
            // on left-click set the new MoveTo
            .add_systems(
                Update,
                click_to_move
                    .run_if(in_state(GameState::Running)),
            )
            // record before any movement happens
            .add_systems(
                Update,
                record_previous_system
                    .before(move_units)
                    .run_if(in_state(GameState::Running)),
            )
            // move in X/Z next
            .add_systems(
                Update,
                move_units
                    .after(click_to_move)
                    .run_if(in_state(GameState::Running)),
            )
            // then re-ground in Y
            .add_systems(
                Update,
                grounding_system
                    .after(move_units)
                    .run_if(in_state(GameState::Running)),
            )
            // finally clamp & rollback bad moves
            .add_systems(
                Update,
                collision_system
                    .after(grounding_system)
                    .run_if(in_state(GameState::Running)),
            );
    }
}
