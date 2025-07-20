use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerAction {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
}

#[derive(Default, Resource)]
pub struct ActionState {
    pressed: HashMap<PlayerAction, bool>,
}

impl ActionState {
    pub fn set(&mut self, action: PlayerAction, is_pressed: bool) {
        self.pressed.insert(action, is_pressed);
    }

    pub fn pressed(&self, action: PlayerAction) -> bool {
        *self.pressed.get(&action).unwrap_or(&false)
    }
}
