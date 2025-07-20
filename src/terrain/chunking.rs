// src/terrain/chunking.rs

use std::collections::{HashMap, HashSet};
use bevy::prelude::*;
use crate::terrain::systems::spawn_terrain_chunk;
use crate::terrain::components::{ChunkCoords, LocalOffset};
use crate::heightmap_data::HeightmapData;
use crate::setup::MainCamera;

/// How many chunks in each direction we keep around the camera.
const CHUNK_RADIUS: i32 = 2;

/// Cached mesh + material handles so we never re-load or re-create.
#[derive(Resource)]
pub struct TerrainAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

/// Tracks which chunk coords → entity are in use,
/// plus the last camera-chunk so we only run on actual moves.
#[derive(Resource)]
pub struct ChunkManager {
    pub loaded: HashMap<(i32, i32), Entity>,
    pub last_cam_chunk: Option<(i32, i32)>,
}

impl Default for ChunkManager {
    fn default() -> Self {
        ChunkManager {
            loaded: HashMap::new(),
            last_cam_chunk: None,
        }
    }
}

impl ChunkManager {
    /// Convert a world-space (x,z) into integer chunk coords.
    fn world_to_chunk(pos: Vec3, size: Vec2) -> (i32, i32) {
        (
            (pos.x / size.x).floor() as i32,
            (pos.z / size.y).floor() as i32,
        )
    }
}

/// (Startup) Preload the mesh & material once.
pub fn preload_terrain_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = asset_server.load("Heightmaps/canyon_heightmap.hmp.png");
    let material = materials.add(StandardMaterial {
        base_color: Color::linear_rgb(0.1, 0.9, 0.1),
        ..default()
    });
    commands.insert_resource(TerrainAssets { mesh, material });
}

/// (Update) Only when the camera actually *changes* chunk do we recycle entities.
pub fn chunk_streaming_system(
    mut commands: Commands,
    cam_q: Query<&Transform, With<MainCamera>>,
    mut manager: ResMut<ChunkManager>,
    heightmap: Res<HeightmapData>,
    terrain_assets: Res<TerrainAssets>,
) {
    // 1) Grab camera transform
    let cam_tf = match cam_q.single() {
        Ok(tf) => tf,
        Err(_) => return,
    };

    // 2) Which chunk is the camera in now?
    let size = heightmap.size;
    let cam_chunk = ChunkManager::world_to_chunk(cam_tf.translation, size);

    // 3) Early-out if still in the same chunk
    if manager.last_cam_chunk == Some(cam_chunk) {
        return;
    }
    manager.last_cam_chunk = Some(cam_chunk);

    // 4) Build the desired set of chunk coords around camera
    let mut want = HashSet::new();
    for dx in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for dz in -CHUNK_RADIUS..=CHUNK_RADIUS {
            want.insert((cam_chunk.0 + dx, cam_chunk.1 + dz));
        }
    }

    // --- INITIAL SPAWN: if we have no chunks loaded yet, just spawn them all ---
    if manager.loaded.is_empty() {
        for &coord in &want {
            let origin = Vec2::new(
                coord.0 as f32 * size.x,
                coord.1 as f32 * size.y,
            );
            let ent = spawn_terrain_chunk(
                &mut commands,
                origin,
                &heightmap,
                &terrain_assets.mesh,
                &terrain_assets.material,
            );
            // attach chunk coords + local offset for that entity
            let half = size * 0.5;
            commands.entity(ent)
                .insert(ChunkCoords { x: coord.0, z: coord.1 })
                .insert(LocalOffset {
                    x: origin.x + half.x,
                    y: 0.0,
                    z: origin.y + half.y,
                });
            manager.loaded.insert(coord, ent);
        }
        return;
    }

    // --- RECYCLING PASS: reuse entities from chunks we no longer want ---

    // a) who’s currently loaded?
    let current_keys: Vec<_> = manager.loaded.keys().cloned().collect();

    // b) which loaded coords are *not* in `want`? → these get recycled
    let removed_coords: Vec<_> = current_keys
        .iter()
        .filter(|c| !want.contains(c))
        .cloned()
        .collect();

    // c) which coords are *new*? (in want but not in current)
    let new_coords: Vec<_> = want
        .difference(&current_keys.iter().cloned().collect())
        .cloned()
        .collect();

    // d) pull out the entities for the removed coords
    let mut free_entities = Vec::new();
    for coord in &removed_coords {
        if let Some(ent) = manager.loaded.remove(coord) {
            free_entities.push(ent);
        }
    }

    // e) zip free_entities with new_coords to re-assign
    for (ent, coord) in free_entities.into_iter().zip(new_coords.into_iter()) {
        let origin = Vec2::new(
            coord.0 as f32 * size.x,
            coord.1 as f32 * size.y,
        );
        let half = size * 0.5;

        commands.entity(ent)
            .insert(ChunkCoords { x: coord.0, z: coord.1 })
            .insert(LocalOffset {
                x: origin.x + half.x,
                y: 0.0,
                z: origin.y + half.y,
            });

        manager.loaded.insert(coord, ent);
    }
}

/// (Update) Wrap any chunked entity that drifts outside its ±half‐size back into chunk coords.
pub fn wrap_chunks(
    mut query: Query<(&mut ChunkCoords, &mut LocalOffset)>,
    heightmap: Res<HeightmapData>,
) {
    let half = heightmap.size * 0.5;
    for (mut cc, mut off) in &mut query {
        if off.x > half.x {
            off.x -= heightmap.size.x;
            cc.x += 1;
        } else if off.x < -half.x {
            off.x += heightmap.size.x;
            cc.x -= 1;
        }
        if off.z > half.y {
            off.z -= heightmap.size.y;
            cc.z += 1;
        } else if off.z < -half.y {
            off.z += heightmap.size.y;
            cc.z -= 1;
        }
    }
}

/// (Update) Rebase all chunked entities around the camera’s real (unmoved) Transform.
pub fn apply_chunked_transform(
    camera_q: Query<(&ChunkCoords, &LocalOffset), With<MainCamera>>,
    mut query: Query<(&ChunkCoords, &LocalOffset, &mut Transform), Without<MainCamera>>,
    heightmap: Res<HeightmapData>,
) {
    let (cam_cc, cam_off) = match camera_q.single() {
        Ok(pair) => pair,
        Err(_) => return,
    };
    let cam_world = Vec3::new(
        cam_cc.x as f32 * heightmap.size.x + cam_off.x,
        cam_off.y,
        cam_cc.z as f32 * heightmap.size.y + cam_off.z,
    );
    for (cc, off, mut tf) in &mut query {
        let world_pos = Vec3::new(
            cc.x as f32 * heightmap.size.x + off.x,
            off.y,
            cc.z as f32 * heightmap.size.y + off.z,
        );
        tf.translation = world_pos - cam_world;
    }
}
