use bevy::prelude::*;
use bevy::input::mouse::MouseButton;
use bevy::input::ButtonInput;
use bevy::window::{Window, PrimaryWindow};

use crate::heightmap_data::{HeightmapData, HeightTileCache, sample_height};
use crate::unit::components::{Unit, MoveTo, Grounded, PreviousPosition};
use crate::terrain::{ChunkCoords, LocalOffset, world_to_chunk_and_local};

/// Spawns your pill-shaped unit, now chunked for seamless streaming
pub fn spawn_unit(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: ResMut<HeightTileCache>,
    heightmap: Res<HeightmapData>,
) {
    let desired_xz = Vec2::new(5.0, 5.0).round();
    let ground_y = sample_height(desired_xz.x, desired_xz.y, &heightmap, &mut cache).unwrap_or(0.0);

    let scale = Vec3::new(0.5, 1.0, 0.5);
    let half_h = scale.y * 0.5;
    let world_pos = Vec3::new(desired_xz.x, ground_y + half_h, desired_xz.y);

    // Use helper to derive chunk coords + local offset (x,z). Add y yourself.
    let (chunk, mut local) = world_to_chunk_and_local(world_pos.x, world_pos.z, &heightmap)
        .map(|(c, l)| (c, l))
        .unwrap_or((
            ChunkCoords { x: 0, z: 0 },
            LocalOffset { x: 0.0, y: 0.0, z: 0.0 },
        ));
    local.y = world_pos.y;

    let mesh = meshes.add(Sphere::new(0.5));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(124, 144, 255),
        ..default()
    });

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform {
            translation: world_pos,
            scale,
            ..default()
        },
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        chunk,      // ChunkCoords
        local,      // LocalOffset
        Unit {
            grounded_offset: half_h,
            max_slope: 0.9,
        },
        PreviousPosition(world_pos),
        MoveTo(world_pos),
        Grounded { offset: half_h },
    ));
}

/// Moves each `Unit` toward its `MoveTo.x/z` only.
pub fn move_units(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &MoveTo), With<Unit>>,
) {
    const SPEED: f32 = 50.0;
    let dt = time.delta_secs();

    for (mut tf, target) in query.iter_mut() {
        let current = Vec2::new(tf.translation.x, tf.translation.z);
        let goal = Vec2::new(target.0.x, target.0.z);
        let dir = (goal - current).normalize_or_zero();
        let step = dir * SPEED * dt;

        if current.distance(goal) > step.length() {
            tf.translation.x += step.x;
            tf.translation.z += step.y;
        } else {
            tf.translation.x = target.0.x;
            tf.translation.z = target.0.z;
        }
    }
}

/// Snaps any `Grounded` entity to the heightmap each frame.
pub fn grounding_system(
    mut cache: ResMut<HeightTileCache>,
    heightmap: Res<HeightmapData>,
    mut query: Query<(&Grounded, &mut Transform)>,
) {
    for (grounded, mut transform) in &mut query {
        let x = transform.translation.x;
        let z = transform.translation.z;
        if let Some(y) = sample_height(x, z, &heightmap, &mut cache) {
            transform.translation.y = y + grounded.offset;
        }
    }
}

/// Click to move
pub fn click_to_move(
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut cache: ResMut<HeightTileCache>,
    heightmap: Res<HeightmapData>,
    mut movers: Query<&mut MoveTo, With<Unit>>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let window = match windows.single() {
        Ok(w) => w,
        Err(_) => return,
    };
    let cursor_pos = match window.cursor_position() {
        Some(pos) => pos,
        None => return,
    };

    let (camera, cam_transform) = match cameras.single() {
        Ok(c) => c,
        Err(_) => return,
    };

    let ray = match camera.viewport_to_world(cam_transform, cursor_pos) {
        Ok(r) => r,
        Err(_) => return,
    };
    let origin = ray.origin;
    let dir = ray.direction;

    if dir.y.abs() < f32::EPSILON {
        return;
    }
    let t_plane = -origin.y / dir.y;
    if t_plane <= 0.0 {
        return;
    }

    let mut t_low = 0.0;
    let mut t_high = t_plane;
    let mut hit = origin + dir * t_plane;
    for _ in 0..6 {
        let t_mid = (t_low + t_high) * 0.5;
        let p = origin + dir * t_mid;
        let ground_h = sample_height(p.x, p.z, &heightmap, &mut cache).unwrap_or(0.0);
        if p.y > ground_h {
            t_low = t_mid;
        } else {
            t_high = t_mid;
            hit = p;
        }
    }

    for mut mv in movers.iter_mut() {
        mv.0.x = hit.x;
        mv.0.z = hit.z;
    }
}

/// Record previous unit position
pub fn record_previous_system(
    mut query: Query<(&mut PreviousPosition, &Transform), With<Unit>>,
) {
    for (mut prev, t) in &mut query {
        **prev = t.translation;
    }
}

/// Collision + slope checks
pub fn collision_system(
    mut cache: ResMut<HeightTileCache>,
    heightmap: Res<HeightmapData>,
    mut query: Query<(&Unit, &PreviousPosition, &mut Transform)>,
) {
    for (unit, prev, mut t) in &mut query {
        let pos = t.translation;
        let ground_y = sample_height(pos.x, pos.z, &heightmap, &mut cache).unwrap_or(0.0) + unit.grounded_offset;

        if pos.y < ground_y {
            t.translation = **prev;
            continue;
        }

        // Use actual tile resolution from cache (not a hardcoded const)
        let dx = heightmap.chunk_size.x / cache.tile_resolution.x as f32;
        let dz = heightmap.chunk_size.y / cache.tile_resolution.y as f32;

        let h_x0 = sample_height(pos.x - dx, pos.z, &heightmap, &mut cache).unwrap_or(ground_y);
        let h_x1 = sample_height(pos.x + dx, pos.z, &heightmap, &mut cache).unwrap_or(ground_y);
        let dhdx = (h_x1 - h_x0) / (2.0 * dx);

        let h_z0 = sample_height(pos.x, pos.z - dz, &heightmap, &mut cache).unwrap_or(ground_y);
        let h_z1 = sample_height(pos.x, pos.z + dz, &heightmap, &mut cache).unwrap_or(ground_y);
        let dhdz = (h_z1 - h_z0) / (2.0 * dz);

        let slope = (dhdx * dhdx + dhdz * dhdz).sqrt();
        let max_tan = unit.max_slope.tan();
        if slope > max_tan {
            t.translation = **prev;
            continue;
        }

        t.translation.y = ground_y;
    }
}
