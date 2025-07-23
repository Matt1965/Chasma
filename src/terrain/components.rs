use bevy::prelude::*;

#[allow(dead_code)]
#[derive(Component)]
pub struct ChunkCoords {
    pub x: i32,
    pub z: i32,
}

#[allow(dead_code)]
#[derive(Component)]
pub struct LocalOffset {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Component)]
pub struct Terrain;