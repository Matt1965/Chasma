use bevy::prelude::*;
use bevy::render::camera::Projection;
use bevy::render::camera::PerspectiveProjection;
use bevy::pbr::CascadeShadowConfig;
use crate::input::CameraOrbit;

#[derive(Component)]
pub struct MainCamera;

pub fn setup(
    mut commands: Commands,
) {
    // 1) Light
    let mut light = commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_rotation_x(-0.5)),
        GlobalTransform::default(),
    ));
    light.insert(CascadeShadowConfig {
        bounds: vec![100.0, 500.0, 2000.0, 5000.0], // 4 cascades, max distance 5000
        ..default()
    });


    // 2) Camera
    commands.spawn((
        Camera3d { ..default() },
        Projection::Perspective(PerspectiveProjection {
            near: 10.0,
            far: 20000.0,
            ..default()
        }),
        Transform::from_xyz(-2.5, 2200.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
        MainCamera,
    ))
    .insert(CameraOrbit {
        focus: Vec3::ZERO,
        radius: 10.0,
        yaw: 0.0,
        pitch: 10.0,
    });
}
