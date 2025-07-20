// src/systems/terrain.rs

use bevy::prelude::*;
use bevy::math::{Vec2, UVec2};
use bevy::render::mesh::{Mesh, Mesh3d};
use image::open;
use crate::heightmap_data::HeightmapData;
use crate::terrain::components::{Terrain};


pub fn load_heightmap_data(mut commands: Commands) {
    // 1) Horizontal extents: width (X) and depth (Z)
    let terrain_size = Vec2::new(100.0, 100.0);
    // 2) Center of that quad in world-space
    let terrain_translation = Vec3::ZERO;

    // 3) Load your grayscale heightmap into a GrayImage
    let dyn_image = open("assets/Heightmaps/canyon_heightmap.hmp.png")
        .expect("Heightmap not found")
        .to_luma8();
    let resolution = UVec2::new(dyn_image.width(), dyn_image.height());

    // 4) Compute the (u=0, v=0) corner in *X/Z* world-space
    let half = terrain_size * 0.5;
    let origin = Vec2::new(
        terrain_translation.x - half.x, // world-min X
        terrain_translation.z - half.y, // world-min Z
    );

    // 5) Insert the HeightmapData resource
    commands.insert_resource(HeightmapData {
        image: dyn_image,
        resolution,
        size: terrain_size,
        height_scale: 10.0,
        origin,
    });
}


/// Spawns one terrain‐chunk at the given world‐min (X,Z) `origin`.
/// 
/// # Parameters
/// - `commands`:           &mut Commands to spawn the entity  
/// - `origin`:             Vec2 world‐min corner (X,Z) of this chunk  
/// - `heightmap`:          &Res<HeightmapData> for size & height_scale  
/// - `mesh_handle`:        &Handle<Mesh>  (preloaded once in TerrainAssets)  
/// - `materials`:          &mut Assets<StandardMaterial> to fetch the material  
/// - `material_handle`:    &Handle<StandardMaterial> (preloaded once in TerrainAssets)  
pub fn spawn_terrain_chunk(
    commands: &mut Commands,
    origin: Vec2,
    heightmap: &Res<HeightmapData>,
    mesh_handle: &Handle<Mesh>,
    material_handle: &Handle<StandardMaterial>,
) -> Entity {
    // 1) Rotate so plugin‐local Z → world Y
    let rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);

    // 2) Compute center = origin + half‐size
    let half = heightmap.size * 0.5;
    let translation = Vec3::new(
        origin.x + half.x,
        0.0,
        origin.y + half.y,
    );

    // 3) Scale so local (X,Y,Z) → world (X,Z,Y)
    let scale = Vec3::new(
        heightmap.size.x,         // X span
        heightmap.size.y,         // Z span (after rotation)
        heightmap.height_scale,   // Y span (after rotation)
    );

    // 4) Spawn with the preloaded mesh & material handles
    commands.spawn((
        Terrain,
        Transform { translation, rotation, scale },
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        Mesh3d(mesh_handle.clone()),
        MeshMaterial3d(material_handle.clone()),
    )).id()
}