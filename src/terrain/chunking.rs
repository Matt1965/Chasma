// src/terrain/chunking.rs

use std::collections::HashMap;
use bevy::prelude::*;

/// How many chunks (in each direction) around the camera to keep loaded.
pub const CHUNK_RADIUS: i32 = 2;

/// Tracks which chunk coords are currently spawned,
/// and what the camera’s last‐chunk was so we only schedule on boundary‐cross.
#[derive(Resource, Default)]
pub struct ChunkManager {
    pub loaded:          HashMap<(i32, i32), Entity>,
    pub last_cam_chunk:  Option<(i32, i32)>,
}

/// Holds your terrain material (now textured!) so the chunk‐spawner can clone it.
#[derive(Resource)]
pub struct TerrainAssets {
    pub material: Handle<StandardMaterial>,
}

/// (Startup) Preload the terrain material once, this time with your mountain texture.
pub fn preload_terrain_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // 1) Load the image from assets/Textures/mountain.png
    let texture_handle: Handle<Image> = asset_server.load("Textures/mountain.png");

    // 2) Create a StandardMaterial that uses it as the base_color_texture
    let terrain_mat = StandardMaterial {
        base_color_texture: Some(texture_handle),
        // you can tweak these if you want more or less specular/roughness:
        perceptual_roughness: 1.0,
        ..default()
    };

    // 3) Add to the Assets<StandardMaterial> and stash the handle
    let mat_handle = materials.add(terrain_mat);
    commands.insert_resource(TerrainAssets { material: mat_handle });
}
