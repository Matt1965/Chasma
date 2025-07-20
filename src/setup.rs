use bevy::prelude::*;
use crate::input::CameraOrbit;

#[derive(Component)]
pub struct MainCamera;

pub fn setup(
    mut commands: Commands,
) {
    // 1) Light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    // 2) Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
        MainCamera,
        CameraOrbit {
            focus: Vec3::ZERO,
            radius: 10.0,
            yaw: 0.0,
            pitch: 0.0,
        },
    ));
}
