use bevy::prelude::*;
use bevy::pbr::MeshMaterial3d;
use crate::props::registry::RenderRef;
use crate::props::streaming::lod::LodGroup;
use crate::props::plugin::TerrainChunkUnloaded;
use super::components::PropChunkTag;

/// Spawn a `RenderRef` into the world at `transform`.
/// Returns the root entity (which may be a LOD group).
pub fn spawn_render_ref(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Option<Entity>,
    render: &RenderRef,
    transform: Transform,
) -> Entity {
    // Common vis/transform components we want to add on all spawned roots.
    let vis_components = (
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
    );

    match render {
        RenderRef::Scene { path } => {
            let scene: Handle<Scene> = assets.load(path.as_str());

            let mut e = commands.spawn((
                // Transform & visibility (no bundles)
                transform,
                GlobalTransform::IDENTITY,
                vis_components,
                // Render as a scene root
                SceneRoot(scene),
            ));
            if let Some(p) = parent { e.insert(ChildOf(p)); }
            e.id()
        }

        RenderRef::MeshMaterial { mesh, material } => {
            let mesh_h: Handle<Mesh> = assets.load(mesh.as_str());
            let mat_h: Handle<StandardMaterial> = match material {
                Some(m) => assets.load(m.as_str()),
                None => assets.add(StandardMaterial::default()),
            };

            let mut e = commands.spawn((
                // Transform & visibility
                transform,
                GlobalTransform::IDENTITY,
                vis_components,
                // PBR drawables (no PbrBundle)
                Mesh3d(mesh_h),
                MeshMaterial3d(mat_h),
            ));
            if let Some(p) = parent { e.insert(ChildOf(p)); }
            e.id()
        }

        RenderRef::Lods { levels } => {
            // Parent node holding children as LOD levels.
            let parent_entity = commands
                .spawn((
                    transform,
                    GlobalTransform::IDENTITY,
                    vis_components,
                    LodGroup::default(),
                ))
                .id();

            for lvl in levels {
                let child = spawn_render_ref(
                    commands,
                    assets,
                    Some(parent_entity),
                    &lvl.repr,
                    Transform::IDENTITY,
                );
                // Ensure child is attached (spawn_render_ref already parents if Some(parent))
                commands.entity(parent_entity).add_child(child);
            }

            // Ensure ascending distances
            let mut distances: Vec<f32> = levels.iter().map(|l| l.distance).collect();
            distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            commands
                .entity(parent_entity)
                .insert(LodGroup { start_distances: distances, active_ix: usize::MAX });

            if let Some(p) = parent {
                commands.entity(parent_entity).insert(ChildOf(p));
            }
            parent_entity
        }
    }
}

/// System: despawn props when a chunk unloads.
pub fn despawn_on_chunk_unloaded(
    mut evr: EventReader<TerrainChunkUnloaded>,
    q: Query<(Entity, &PropChunkTag)>,
    mut commands: Commands,
) {
    for ev in evr.read() {
        let dead = ev.0;
        for (e, tag) in q.iter() {
            if tag.0 == dead {
                commands.entity(e).despawn(); // (recursive by default in Bevy 0.16)
            }
        }
    }
}