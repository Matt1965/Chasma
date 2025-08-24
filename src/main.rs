use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin};

mod setup;
mod input;
mod actions;
mod state;
mod ui;
mod heightmap_data;
mod terrain;
mod unit;
mod props;

// re-export the bits we actually need in main
use actions::ActionState;
use input::{camera_controller, input_mapping_system, pause_toggle_system};
use state::GameState;
use ui::{spawn_pause_overlay, despawn_pause_overlay};
use terrain::TerrainPlugin;
use unit::UnitPlugin;
use props::PropsStackPlugin;
use bevy::render::{RenderPlugin, settings::WgpuSettings};

fn main() {
        // Start with Bevy’s default settings…
    let mut wgpu_settings = WgpuSettings::default();
    // …but raise the max 2D texture size to 16K:
    wgpu_settings.limits.max_texture_dimension_2d = 16_384;

    App::new()
        .add_plugins(
            DefaultPlugins
                // → override the RenderPlugin to inject our WgpuSettings
                .set(RenderPlugin {
                    render_creation: wgpu_settings.into(),
                    ..Default::default()
                })
                // → override the WindowPlugin to set our window title & size
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Chasma".to_string(),
                        resolution: (1920., 1080.).into(),
                        ..default() // fill in the rest from `Window::default()`
                    }),
                    ..default()    // fill in exit_condition, close_when_requested, etc.
                })
        )
        // core engine plugins
        // your domain plugins
        .add_plugins(TerrainPlugin)   // loads + spawns the heightmap terrain
        .add_plugins(UnitPlugin)      // spawns & moves your pill‐units
        .add_plugins(PropsStackPlugin)
        // legacy input/tracking systems left in place:
        //
        // init resources & game-state
        .init_resource::<ActionState>()
        .init_state::<GameState>()
        // camera, lights, whatever your setup.rs does
        .add_systems(Startup, setup::setup)
        // pause‐menu UI
        .add_systems(OnEnter(GameState::Paused), spawn_pause_overlay)
        .add_systems(OnExit(GameState::Paused), despawn_pause_overlay)
        // input + camera + pause toggle each frame
        .add_systems(Update, pause_toggle_system)
        .add_systems(
            Update,
            (input_mapping_system, camera_controller).run_if(in_state(GameState::Running))
        )
        .run();
}
