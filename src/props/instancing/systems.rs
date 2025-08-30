// src/props/instancing/systems.rs

use bevy::prelude::*;
use bevy::render::mesh::{Mesh, Indices, VertexAttributeValues};

use crate::props::core::{ChunkCoord, PropArchetypeId};
use crate::props::queue::{SpawnQueue, SpawnQueueConfig, SpawnRequest};
use crate::props::registry::RenderRef;
use crate::props::plugin::TerrainChunkUnloaded;

use super::components::{InstanceBatch, BatchStats};
use super::resources::{InstanceBatches, PropsInstancingConfig};

/// Drain queue -> append transforms to batches. No mesh building here.
pub fn drain_spawn_queue_into_batches(
    mut commands: Commands,
    mut queue: ResMut<SpawnQueue>,
    qcfg: Res<SpawnQueueConfig>,
    mut batches: ResMut<InstanceBatches>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    // ⬇ add this query to mutate batches directly
    mut q_batch: Query<(&mut InstanceBatch, &mut BatchStats)>,
) {
    if queue.items.is_empty() { return; }

    let drain_n = qcfg.max_per_frame.min(queue.items.len());
    let len = queue.items.len();
    if drain_n == 0 || len == 0 { return; }
    let split_at = len - drain_n;
    let mut drained: Vec<SpawnRequest> = queue.items.split_off(split_at);

    for req in drained.drain(..) {
        let (base_mesh, material) = match resolve_mesh_and_material(&req.render, &asset_server, &mut materials) {
            Some(mm) => mm,
            None => { warn!("Instancing: unsupported render type for {:?}; skipping.", req.id); continue; }
        };

        let key = (req.chunk, req.id.archetype);
        let batch_e = match batches.by_key.get(&key).copied() {
            Some(e) => e,
            None => {
                let e = commands
                    .spawn((
                        InstanceBatch {
                            chunk: req.chunk,
                            archetype: req.id.archetype,
                            base_mesh: base_mesh.clone(),
                            material: material.clone(),
                            instances: Vec::new(),
                            dirty: true,
                            last_built_count: 0,
                        },
                        BatchStats::default(),
                        Name::new(format!("Batch {:?} / {:?}", req.chunk, req.id.archetype)),
                        Transform::default(),
                        GlobalTransform::default(),
                        Visibility::Hidden,
                    ))
                    .id();
                batches.by_key.insert(key, e);
                e
            }
        };

        if let Ok((mut batch, mut stats)) = q_batch.get_mut(batch_e) {
            batch.instances.push(req.transform);
            batch.mark_dirty();
            stats.instance_count += 1;
        }
    }
}


/// Merge source mesh once into a combined mesh per batch (only when dirty), with a small budget.
pub fn rebuild_dirty_batches(
    mut commands: Commands,
    mut q_batches: Query<(Entity, &mut InstanceBatch)>,
    cfg: Res<PropsInstancingConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if q_batches.is_empty() { return; }
    let mut merged_this_frame = 0usize;

    for (e, mut batch) in q_batches.iter_mut() {
        if !batch.dirty { continue; }
        if merged_this_frame >= cfg.max_merges_per_frame { break; }

        let Some(src_mesh) = meshes.get(&batch.base_mesh).cloned() else {
            // Base mesh not ready yet
            continue;
        };

        // Build merged once.
        let Some(merged) = merge_mesh_instances_fast(&src_mesh, &batch.instances) else {
            continue;
        };
        let merged_handle = meshes.add(merged);

        // Force a **very cheap** material: unlit + alpha mask + double-sided, no shadows.
        let mut mat = materials.get_mut(&batch.material).cloned()
            .unwrap_or_else(|| StandardMaterial { base_color: Color::WHITE, ..Default::default() });
        mat.unlit = true;
        mat.alpha_mode = AlphaMode::Mask(0.5);
        mat.cull_mode = None;
        mat.double_sided = true;
        mat.perceptual_roughness = 1.0;
        mat.metallic = 0.0;
        let mat_handle = materials.add(mat);

        commands.entity(e).insert((
            bevy::render::mesh::Mesh3d(merged_handle),
            bevy::pbr::MeshMaterial3d(mat_handle),
            Visibility::Visible,
        ));

        batch.dirty = false;
        merged_this_frame += 1;
    }
}

/// Cull entire batch by camera distance (super cheap).
pub fn cull_batches_by_distance(
    cam_q: Query<&Transform, With<crate::setup::MainCamera>>,
    mut q: Query<(&mut Visibility, &InstanceBatch, &Transform)>,
    cfg: Res<PropsInstancingConfig>,
) {
    let Ok(cam_tf) = cam_q.single() else { return; };
    let cam = cam_tf.translation;

    for (mut vis, batch, tf) in q.iter_mut() {
        // simple center check; feel free to use your chunk aabb instead
        let center = tf.translation;
        let d2 = center.distance_squared(cam);
        *vis = if d2 <= cfg.max_visible_distance_sq { Visibility::Visible } else { Visibility::Hidden };
        // hide empty batches as well
        if batch.instances.is_empty() { *vis = Visibility::Hidden; }
    }
}

/// Clear batches when a chunk unloads.
pub fn cleanup_batches_on_chunk_unloaded(
    mut evr: EventReader<TerrainChunkUnloaded>,
    mut commands: Commands,
    mut batches: ResMut<InstanceBatches>,
    q_has: Query<(), With<InstanceBatch>>,
) {
    for ev in evr.read() {
        let chunk = ev.0;
        let keys: Vec<_> = batches.by_key
            .keys()
            .copied()
            .filter(|(c, _)| *c == chunk)
            .collect();
        for key in keys {
            if let Some(e) = batches.by_key.remove(&key) {
                if q_has.get(e).is_ok() {
                    commands.entity(e).despawn();
                }
            }
        }
    }
}

/// Resolve (mesh, material) only for MeshMaterial render kinds.
fn resolve_mesh_and_material(
    render: &RenderRef,
    assets: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> Option<(Handle<Mesh>, Handle<StandardMaterial>)> {
    match render {
        RenderRef::MeshMaterial { mesh, material } => {
            let mesh_h: Handle<Mesh> = assets.load(mesh.as_str());
            let mat_h: Handle<StandardMaterial> = if let Some(path) = material {
                assets.load(path.as_str())
            } else {
                materials.add(StandardMaterial { base_color: Color::WHITE, ..Default::default() })
            };
            Some((mesh_h, mat_h))
        }
        _ => None,
    }
}

/// Faster merge: transform positions only, optionally drop normals/tangents for grass cards.
/// Keep UVs; PBR is off (unlit), so normals won’t affect lighting anyway.
fn merge_mesh_instances_fast(src: &Mesh, instances: &[Transform]) -> Option<Mesh> {
    let positions: Vec<[f32; 3]> = match src.attribute(Mesh::ATTRIBUTE_POSITION)? {
        VertexAttributeValues::Float32x3(v) => v.clone(),
        _ => return None,
    };

    // Keep UVs if present; drop normals for speed/bandwidth
    let uvs: Option<Vec<[f32; 2]>> = src
        .attribute(Mesh::ATTRIBUTE_UV_0)
        .and_then(|vals| match vals {
            VertexAttributeValues::Float32x2(v) => Some(v.clone()),
            _ => None,
        });

    let src_indices: Option<Vec<u32>> = match src.indices() {
        Some(Indices::U32(v)) => Some(v.clone()),
        Some(Indices::U16(v)) => Some(v.iter().map(|&x| x as u32).collect()),
        None => None,
    };

    let src_vtx = positions.len() as u32;
    let inst_n = instances.len().max(1);

    let mut out_positions = Vec::with_capacity(src_vtx as usize * inst_n);
    let mut out_uvs: Option<Vec<[f32; 2]>> = uvs.as_ref().map(|_| Vec::with_capacity(src_vtx as usize * inst_n));
    let mut out_indices: Vec<u32> = Vec::with_capacity(src_indices.as_ref().map(|ix| ix.len()).unwrap_or(0) * inst_n);

    for (inst_id, t) in instances.iter().enumerate() {
        let trs = Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation);

        for (i, p) in positions.iter().enumerate() {
            let wp = trs * Vec4::new(p[0], p[1], p[2], 1.0);
            out_positions.push([wp.x, wp.y, wp.z]);

            if let (Some(src_uv), Some(dst_uv)) = (uvs.as_ref(), out_uvs.as_mut()) {
                dst_uv.push(src_uv[i]);
            }
        }

        if let Some(ix) = &src_indices {
            let base = (inst_id as u32) * src_vtx;
            out_indices.extend(ix.iter().map(|&i| i + base));
        }
    }

    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, out_positions);
    if let Some(uv) = out_uvs { mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv); }
    if let Some(ix) = src_indices { mesh.insert_indices(Indices::U32(ix)); }
    Some(mesh)
}
