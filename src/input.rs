use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::input::{mouse::MouseMotion, keyboard::KeyCode, ButtonInput};

use crate::actions::{PlayerAction, ActionState};
use crate::heightmap_data::{HeightmapData, HeightTileCache, sample_height};
use crate::setup::MainCamera;
use crate::state::GameState;

pub const MOVE_SPEED: f32 = 250.0;
pub const ROTATE_SPEED: f32 = 0.2;
pub const MAX_CAMERA_DT: f32 = 0.05; // never use a dt larger than 50ms

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
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut motion_evr: EventReader<MouseMotion>,
    mut scroll_evr: EventReader<MouseWheel>,
    action_state: Res<ActionState>,
    heightmap: Res<HeightmapData>,
    mut cache: ResMut<HeightTileCache>,
    mut query: Query<(&mut Transform, &mut CameraOrbit), With<MainCamera>>,
) {
    // 0) Clamp delta
    let mut dt = time.delta_secs();
    if dt > MAX_CAMERA_DT {
        dt = MAX_CAMERA_DT;
    }

    let Ok((mut tf, mut orbit)) = query.single_mut() else { return; };

    // 1) Camera-relative movement
    let forward = Vec2::new(-orbit.yaw.cos(), -orbit.yaw.sin());
    let right = Vec2::new(-forward.y, forward.x);

    let mut dir = Vec2::ZERO;
    if action_state.pressed(PlayerAction::MoveForward) { dir += forward; }
    if action_state.pressed(PlayerAction::MoveBackward) { dir -= forward; }
    if action_state.pressed(PlayerAction::MoveLeft) { dir -= right; }
    if action_state.pressed(PlayerAction::MoveRight) { dir += right; }

    if dir != Vec2::ZERO {
        let delta = dir.normalize() * MOVE_SPEED * dt;
        orbit.focus.x += delta.x;
        orbit.focus.z += delta.y;
    }

    // 2) Ground the focus Y
    orbit.focus.y = sample_height(orbit.focus.x, orbit.focus.z, &heightmap, &mut cache).unwrap_or(0.0);

    // 3) Zoom
    for ev in scroll_evr.read() {
        let amount = match ev.unit {
            MouseScrollUnit::Line => ev.y * 1.0,
            MouseScrollUnit::Pixel => ev.y * 0.02,
        };
        orbit.radius = (orbit.radius - amount).clamp(2.0, 2000.0);
    }

    // 4) Orbit
    if mouse_buttons.pressed(MouseButton::Middle) {
        for ev in motion_evr.read() {
            orbit.yaw += ev.delta.x * ROTATE_SPEED * dt;
            orbit.pitch += ev.delta.y * ROTATE_SPEED * dt;
        }
    }

    orbit.pitch = orbit.pitch.clamp(
        -std::f32::consts::FRAC_PI_2 + 0.01,
        std::f32::consts::FRAC_PI_2 - 0.01,
    );

    // 5) Position camera
    let xz_radius = orbit.radius * orbit.pitch.cos();
    let offset = Vec3::new(
        xz_radius * orbit.yaw.cos(),
        orbit.radius * orbit.pitch.sin(),
        xz_radius * orbit.yaw.sin(),
    );

    tf.translation = orbit.focus + offset;

    // 6) Prevent underground camera
    let terrain_y = sample_height(tf.translation.x, tf.translation.z, &heightmap, &mut cache).unwrap_or(0.0);
    if tf.translation.y < terrain_y + 2.5 {
        tf.translation.y = terrain_y + 2.5;
    }

    tf.look_at(orbit.focus, Vec3::Y);
}
