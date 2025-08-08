// src/terrain/water.rs

use bevy::prelude::*;
use bevy::math::primitives::Cuboid;

use crate::heightmap_data::HeightmapData;

/// How high your water sits above Y=0.
#[derive(Resource)]
pub struct WaterLevel(pub f32);

/// Spawn a big, semi-transparent “water slab” across the heightmap.
pub fn spawn_water(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap: Res<HeightmapData>,
    water: Res<WaterLevel>,
) {
    // World extents in X/Z. Your map is centered at (0,0), so a cuboid of this size
    // will cover from -size/2..+size/2 in both axes, matching terrain bounds.
    let size_x = heightmap.size.x;
    let size_z = heightmap.size.y;

    // Very thin “slab” so it behaves like a plane but uses the new primitives API.
    let water_mesh = Mesh::from(Cuboid::new(size_x, 0.05, size_z));
    let mesh_h = meshes.add(water_mesh);

    // Semi-transparent, double-sided material. Tune to taste.
    let mat_h = materials.add(StandardMaterial {
        base_color: Color::linear_rgba(0.0, 0.35, 0.55, 0.6),
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        perceptual_roughness: 0.15,
        reflectance: 0.6,
        ..Default::default()
    });

    commands.spawn((
        Mesh3d(mesh_h),
        MeshMaterial3d(mat_h),
        Transform::from_translation(Vec3::new(0.0, water.0, 0.0)),
        GlobalTransform::default(),
        Name::new("Water"),
    ));
}
