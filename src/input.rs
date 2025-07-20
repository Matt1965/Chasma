use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::input::{mouse::MouseMotion, keyboard::KeyCode, ButtonInput};

use crate::actions::{PlayerAction, ActionState};

pub const MOVE_SPEED: f32 = 5.0;
pub const ROTATE_SPEED: f32 = 0.2;

use crate::setup::MainCamera;
use crate::state::GameState;

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
    action_state.set(PlayerAction::MoveForward, keys.pressed(KeyCode::KeyS));
    action_state.set(PlayerAction::MoveBackward, keys.pressed(KeyCode::KeyW));
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
    mut query: Query<(&mut Transform, &mut CameraOrbit), With<MainCamera>>,
) {
    let Ok((mut transform, mut orbit)) = query.single_mut() else { return; };

    // === Zoom ===
    for ev in scroll_evr.read() {
        let scroll_amount = match ev.unit {
            MouseScrollUnit::Line => ev.y * 0.5,
            MouseScrollUnit::Pixel => ev.y * 0.01,
        };
        orbit.radius = (orbit.radius - scroll_amount).clamp(2.0, 100.0);
    }

    // === WASD movement ===
    let forward = transform.rotation.mul_vec3(Vec3::Z).xz().normalize_or_zero();
    let right = transform.rotation.mul_vec3(Vec3::X).xz().normalize_or_zero();
    let mut direction = Vec2::ZERO;

    if action_state.pressed(PlayerAction::MoveForward) {
        direction += forward;
    }
    if action_state.pressed(PlayerAction::MoveBackward) {
        direction -= forward;
    }
    if action_state.pressed(PlayerAction::MoveLeft) {
        direction -= right;
    }
    if action_state.pressed(PlayerAction::MoveRight) {
        direction += right;
    }

    let movement = Vec3::new(direction.x, 0.0, direction.y) * MOVE_SPEED * time.delta_secs();
    orbit.focus += movement;

    // === Orbit ===
    if mouse_buttons.pressed(MouseButton::Middle) {
        for ev in motion_evr.read() {
            orbit.yaw += ev.delta.x * ROTATE_SPEED * time.delta_secs();
            orbit.pitch += ev.delta.y * ROTATE_SPEED * time.delta_secs();
        }
    }

    // Clamp pitch to avoid gimbal lock
    orbit.pitch = orbit.pitch.clamp(-std::f32::consts::FRAC_PI_2 + 0.01, std::f32::consts::FRAC_PI_2 - 0.01);

    // Convert spherical coordinates (yaw, pitch) to cartesian
    let direction = Vec3::new(
        orbit.yaw.cos() * orbit.pitch.cos(),
        orbit.pitch.sin(),
        orbit.yaw.sin() * orbit.pitch.cos(),
    );

    transform.translation = orbit.focus + direction * orbit.radius;
    transform.look_at(orbit.focus, Vec3::Y);
}

