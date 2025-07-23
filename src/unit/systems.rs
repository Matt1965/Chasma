// src/unit/systems.rs

use bevy::prelude::*;
use bevy::input::mouse::MouseButton;
use bevy::input::ButtonInput;
use bevy::window::{Window, PrimaryWindow};

use crate::heightmap_data::HeightmapData;
use crate::unit::components::{Unit, MoveTo, Grounded, PreviousPosition};
use crate::terrain::{ChunkCoords, LocalOffset};

/// Spawns your pill-shaped unit, now chunked for seamless streaming
pub fn spawn_unit(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap: Res<HeightmapData>,
) {
    // 1) Pick your X/Z on the heightmap
    let desired_xz = Vec2::new(5.0, 5.0).round();
    let ground_y   = heightmap.sample_height(desired_xz.x, desired_xz.y);

    // 2) Unit’s world-space position & scale
    let scale     = Vec3::new(0.5, 1.0, 0.5);
    let half_h    = scale.y * 0.5;
    let world_pos = Vec3::new(desired_xz.x, ground_y + half_h, desired_xz.y);

    // 3) Compute which chunk that falls in
    let chunk_x = (world_pos.x / heightmap.size.x).floor() as i32;
    let chunk_z = (world_pos.z / heightmap.size.y).floor() as i32;

    // 4) And the “local offset” inside that chunk
    let local_x = world_pos.x - (chunk_x as f32 * heightmap.size.x);
    let local_z = world_pos.z - (chunk_z as f32 * heightmap.size.y);

    // 5) Build mesh & material
    let mesh     = meshes.add(Sphere::new(0.5));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(124, 144, 255),
        ..default()
    });

    // 6) Spawn with ChunkCoords + LocalOffset (Transform.translation will be rebased)
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform {
            translation: world_pos, // initial, gets overwritten by apply_chunked_transform
            scale,
            ..default()
        },
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        ChunkCoords { x: chunk_x, z: chunk_z },
        LocalOffset { x: local_x, y: world_pos.y, z: local_z },
        Unit {
            grounded_offset: half_h,
            max_slope: 0.9, // ≈ 45°
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
    const SPEED: f32 = 5.0;
    let dt = time.delta_secs();

    for (mut tf, target) in query.iter_mut() {
        let current = Vec2::new(tf.translation.x, tf.translation.z);
        let goal    = Vec2::new(target.0.x,        target.0.z);
        let dir     = (goal - current).normalize_or_zero();
        let step    = dir * SPEED * dt;

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
    heightmap: Res<HeightmapData>,
    mut query: Query<(&Grounded, &mut Transform)>,
) {
    for (grounded, mut transform) in &mut query {
        let x = transform.translation.x;
        let z = transform.translation.z;
        let y = heightmap.sample_height(x, z);
        transform.translation.y = y + grounded.offset;
    }
}


/// On left-click, cast a ray into world-space, then bisect it against the
/// heightmap surface for exact X/Z intersection, and update each Unit’s MoveTo.
pub fn click_to_move(
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    heightmap: Res<HeightmapData>,
    mut movers: Query<&mut MoveTo, With<Unit>>,
) {
    // only on the exact frame the left button was pressed
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    // 1) get the primary window & cursor pos
    let window = match windows.single() {
        Ok(w) => w,
        Err(_) => return,
    };
    let cursor_pos = match window.cursor_position() {
        Some(pos) => pos,
        None      => return,
    };

    // 2) get the single 3D camera & its transform
    let (camera, cam_transform) = match cameras.single() {
        Ok(c) => c,
        Err(_) => return,
    };

    // 3) build a world-space ray from the cursor
    let ray = match camera.viewport_to_world(cam_transform, cursor_pos) {
        Ok(r)  => r,
        Err(_) => return,
    };
    let origin = ray.origin;
    let dir    = ray.direction;

    // 4) find the rough plane intersection at y=0
    if dir.y.abs() < f32::EPSILON {
        return;
    }
    let t_plane = -origin.y / dir.y;
    if t_plane <= 0.0 {
        return;
    }

    // 5) bisect between t=0 and t_plane to solve
    //    origin.y + t*dir.y == heightmap.sample_height(origin.x + t*dir.x, origin.z + t*dir.z)
    let mut t_low  = 0.0;
    let mut t_high = t_plane;
    let mut hit    = origin + dir * t_plane;
    for _ in 0..6 {
        let t_mid = (t_low + t_high) * 0.5;
        let p = origin + dir * t_mid;
        let ground_h = heightmap.sample_height(p.x, p.z);
        if p.y > ground_h {
            t_low = t_mid;
        } else {
            t_high = t_mid;
            hit = p;
        }
    }

    // 6) update each Unit’s target X/Z to this precise hit point
    for mut mv in movers.iter_mut() {
        mv.0.x = hit.x;
        mv.0.z = hit.z;
    }
}

/// (1) Snapshot each unit’s current position before it moves.
pub fn record_previous_system(
    mut query: Query<(&mut PreviousPosition, &Transform), With<Unit>>,
) {
    for (mut prev, t) in &mut query {
        **prev = t.translation;
    }
}

pub fn collision_system(
    heightmap: Res<HeightmapData>,
    mut query: Query<(&Unit, &PreviousPosition, &mut Transform)>,
) {
    for (unit, prev, mut t) in &mut query {
        let pos = t.translation;

        // 1) Sample ground height under feet
        let ground_y = heightmap.sample_height(pos.x, pos.z) + unit.grounded_offset;

        // 2) Reject falling below ground
        if pos.y < ground_y {
            t.translation = **prev;
            continue;
        }

        // 3) Compute local gradients via central differences
        let dx = heightmap.size.x / heightmap.resolution.x as f32;
        let dz = heightmap.size.y / heightmap.resolution.y as f32;

        let h_x0 = heightmap.sample_height(pos.x - dx, pos.z);
        let h_x1 = heightmap.sample_height(pos.x + dx, pos.z);
        let dhdx = (h_x1 - h_x0) / (2.0 * dx);

        let h_z0 = heightmap.sample_height(pos.x, pos.z - dz);
        let h_z1 = heightmap.sample_height(pos.x, pos.z + dz);
        let dhdz = (h_z1 - h_z0) / (2.0 * dz);

        // 4) Compute slope magnitude and compare to max
        let slope = (dhdx * dhdx + dhdz * dhdz).sqrt();
        let max_tan = unit.max_slope.tan();
        if slope > max_tan {
            t.translation = **prev;
            continue;
        }

        // 5) Clamp to ground
        t.translation.y = ground_y;
    }
}
