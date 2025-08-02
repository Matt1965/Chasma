use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::input::{mouse::MouseMotion, keyboard::KeyCode, ButtonInput};

use crate::actions::{PlayerAction, ActionState};
use crate::heightmap_data::HeightmapData;
use crate::setup::MainCamera;
use crate::state::GameState;

pub const MOVE_SPEED: f32 = 250.0;
pub const ROTATE_SPEED: f32 = 0.2;

#[derive(Component)]
pub struct CameraOrbit {
    pub focus: Vec3,
    pub radius: f32,
    pub yaw: f32,
    pub pitch: f32,
}



pub fn input_mapping_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut action_state: ResMut<ActionState>,
) {
    action_state.set(PlayerAction::MoveForward, keys.pressed(KeyCode::KeyW));
    action_state.set(PlayerAction::MoveBackward, keys.pressed(KeyCode::KeyS));
    action_state.set(PlayerAction::MoveLeft, keys.pressed(KeyCode::KeyA));
    action_state.set(PlayerAction::MoveRight, keys.pressed(KeyCode::KeyD));
}


pub fn pause_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
    current_state: Res<State<GameState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        if current_state.get() == &GameState::Running {
            next_state.set(GameState::Paused);
            info!("Paused game");
        } else if current_state.get() == &GameState::Paused {
            next_state.set(GameState::Running);
            info!("Resumed game");
        }
    }
}


pub fn camera_controller(
    time:           Res<Time>,
    mouse_buttons:  Res<ButtonInput<MouseButton>>,
    mut motion_evr: EventReader<MouseMotion>,
    mut scroll_evr:  EventReader<MouseWheel>,
    action_state:   Res<ActionState>,
    heightmap:      Res<HeightmapData>,
    mut query:      Query<(&mut Transform, &mut CameraOrbit), With<MainCamera>>,
) {
    // 1) Pull out our single camera’s Transform & orbit state
    let Ok((mut tf, mut orbit)) = query.single_mut() else { return; };

    // 2) Build camera-relative forward & right on XZ plane
    //    forward = direction camera is looking horizontally
    let forward = Vec2::new(-orbit.yaw.cos(), -orbit.yaw.sin());
    //    right = perpendicular to forward, to the camera’s right
    let right   = Vec2::new( -forward.y, forward.x);

    // 3) WASD → pan the focus in XZ relative to camera
    let mut dir = Vec2::ZERO;
    if action_state.pressed(PlayerAction::MoveForward)  { dir += forward; }
    if action_state.pressed(PlayerAction::MoveBackward) { dir -= forward; }
    if action_state.pressed(PlayerAction::MoveLeft)     { dir -= right;   }
    if action_state.pressed(PlayerAction::MoveRight)    { dir += right;   }
    if dir != Vec2::ZERO {
        let delta = dir.normalize() * MOVE_SPEED * time.delta_secs();
        orbit.focus.x += delta.x;
        orbit.focus.z += delta.y;
    }

    // 4) Ground the focus to terrain height
    orbit.focus.y = heightmap.sample_height(orbit.focus.x, orbit.focus.z);

    // 5) Scroll-wheel zoom → adjust orbit.radius
    for ev in scroll_evr.read() {
        let amount = match ev.unit {
            MouseScrollUnit::Line  => ev.y * 1.0,
            MouseScrollUnit::Pixel => ev.y * 0.02,
        };
        orbit.radius = (orbit.radius - amount).clamp(2.0, 2000.0);
    }

    // 6) Middle-mouse drag → yaw & pitch
    if mouse_buttons.pressed(MouseButton::Middle) {
        for ev in motion_evr.read() {
            orbit.yaw   += ev.delta.x * ROTATE_SPEED * time.delta_secs();
            orbit.pitch += ev.delta.y * ROTATE_SPEED * time.delta_secs();
        }
    }
    // clamp pitch so you can’t flip upside-down
    orbit.pitch = orbit.pitch.clamp(
        -std::f32::consts::FRAC_PI_2 + 0.01,
         std::f32::consts::FRAC_PI_2 - 0.01,
    );

    // 7) Compute the spherical offset from focus:
    //    • horizontal radius = radius * cos(pitch)
    //    • vertical    = radius * sin(pitch)
    let xz_radius = orbit.radius * orbit.pitch.cos();
    let offset = Vec3::new(
        xz_radius * orbit.yaw.cos(),
        orbit.radius * orbit.pitch.sin(),
        xz_radius * orbit.yaw.sin(),
    );

    // 8) Place camera at focus + offset, and look back at focus
    tf.translation = orbit.focus + offset;
    tf.look_at(orbit.focus, Vec3::Y);
}