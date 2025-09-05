use bevy::prelude::*;
use bevy::render::mesh::{Mesh, Indices, VertexAttributeValues};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future::{block_on, poll_once};

use crate::props::core::{ChunkCoord, PropArchetypeId};
use crate::props::plugin::{PropsRegistryHandle, TerrainChunkUnloaded};
use crate::props::queue::{SpawnQueue, SpawnQueueConfig, SpawnRequest};
use crate::props::registry::{PropsRegistry, RenderRef};
use super::components::{InstanceBatch, BatchStats};
use super::resources::{InstanceBatches, PropsInstancingConfig, MergeIntegrationQueue};

/// -------- queue draining (routes per archetype draw mode) --------

pub fn drain_spawn_queue_into_batches(
    mut commands: Commands,
    mut queue: ResMut<SpawnQueue>,
    qcfg: Res<SpawnQueueConfig>,
    mut batches: ResMut<InstanceBatches>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _meshes: ResMut<Assets<Mesh>>, // keep if you later need a placeholder mesh
) {
    info!("Draining {} spawn requests into instance batches...", queue.items.len());

    if queue.items.is_empty() { return; }

    let drain_n = qcfg.max_per_frame.min(queue.items.len());
    let start = queue.items.len() - drain_n;

    // Move items out in one shot to avoid E0502
    let drained: Vec<SpawnRequest> = queue.items.drain(start..).collect();

    for req in drained {
        // Only batch Mesh+Material entries; route others to streaming elsewhere
        let (base_mesh, material) = match resolve_mesh_and_material(&req.render, &assets, &mut materials) {
            Some(mm) => mm,
            None => {
                warn!("Instancing: unsupported render type for {:?}; skipping (use streaming path).", req.id);
                continue;
            }
        };

        let key = (req.chunk, req.id.archetype);

        // Find or create batch entity for (chunk, archetype)
        let batch_e = if let Some(list) = batches.by_key.get(&key) {
            list[0] // or handle all of them if you want multi-batching
        } else {
            let e = commands.spawn((
                InstanceBatch {
                    chunk: req.chunk,
                    archetype: req.id.archetype,
                    base_mesh: base_mesh.clone(),
                    material: material.clone(),
                    instances: Vec::new(),
                    dirty: false,
                    last_built_count: 0,
                    building: false,
                },
                BatchStats::default(),
                Name::new(format!("Batch {:?} / {:?}", req.chunk, req.id.archetype)),
                Transform::default(),
                GlobalTransform::default(),
                Visibility::Hidden,
            )).id();
        
            batches.by_key.insert(key, vec![e]);  // ✅ FIX: wrap `e` in `vec![]`
            e
        };

        // Mutate the batch using the command-queue world closure (no Commands::add in 0.16)
        commands.queue(move |world: &mut World| {
            if let Some(mut batch) = world.get_mut::<InstanceBatch>(batch_e) {
                batch.instances.push(req.transform);
                batch.dirty = true;
            }
            if let Some(mut stats) = world.get_mut::<BatchStats>(batch_e) {
                stats.instance_count += 1;
            }
        });
    }
}

/// -------- async merge & integrate (budgeted) --------

/// Schedule/perform merges for batches marked dirty; integration is deferred to `integrate_finished_merges`.
pub fn rebuild_dirty_batches(
    mut commands: Commands,
    cfg: Res<PropsInstancingConfig>,
    regs: Res<Assets<PropsRegistry>>,
    handle: Res<PropsRegistryHandle>,
    mut q_batches: Query<(Entity, &mut InstanceBatch)>,
    meshes: Res<Assets<Mesh>>,
    mut integration: ResMut<MergeIntegrationQueue>,
) {
    let Some(reg) = regs.get(&handle.0) else { return; };

    // We’ll launch at most cfg.max_merges_per_frame tasks.
    let mut launched = 0usize;

    for (e, mut batch) in q_batches.iter_mut() {
        if launched >= cfg.max_merges_per_frame { break; }
        if batch.building || !batch.dirty { continue; }

        // Ensure archetype exists (for thresholds/caps if you want)
        let _arch = reg.archetypes.get(batch.archetype.0 as usize);

        // Source mesh must be available
        let Some(src) = meshes.get(&batch.base_mesh).cloned() else { continue; };

        // Snapshot transforms now
        let transforms = batch.instances.clone();
        batch.building = true; // prevent relaunch
        launched += 1;

        // Async compute
        let task = AsyncComputeTaskPool::get().spawn(async move {
            merge_mesh_instances(&src, &transforms)
        });

        // We can poll and immediately push into integration when done.
        // Simpler: block on task.join(). But we want async; so we detach a tiny system-closure here:
        commands.spawn(DeferredMergeResult { target: e, task });
    }
}

/// A tiny helper component to poll a single merge task next frame
#[derive(Component)]
pub struct DeferredMergeResult {
    target: Entity,
    task: Task<Option<Mesh>>,
}

/// Polls deferred merge tasks and pushes finished meshes to the integration queue.
pub fn poll_merge_tasks(
    mut commands: Commands,
    mut q: Query<(Entity, &mut DeferredMergeResult)>,
    mut integration: ResMut<MergeIntegrationQueue>,
) {
    for (e, mut d) in q.iter_mut() {
        if let Some(out) = block_on(poll_once(&mut d.task)) {
            if let Some(mesh) = out {
                integration.finished.push((d.target, mesh));
            }
            commands.entity(e).despawn();
        }
    }
}


/// Integrate up to budget finished merges into the world (attach Mesh3d)
pub fn integrate_finished_merges(
    mut commands: Commands,
    mut integration: ResMut<MergeIntegrationQueue>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut q: Query<&mut InstanceBatch>,
) {
    let mut i = 0usize;
    while i < integration.finished.len() {
        let (target, mesh) = integration.finished.remove(i);

        let handle = meshes.add(mesh);
        if let Ok(mut b) = q.get_mut(target) {
            // capture count BEFORE calling a &mut self method
            let built_count = b.instances.len();

            commands.entity(target).insert((
                bevy::render::mesh::Mesh3d(handle),
                bevy::pbr::MeshMaterial3d(b.material.clone()),
                Visibility::Visible,
            ));
            b.clear_build_flags(built_count);
        }
    }
}


/// -------- chunk unload cleanup --------

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
            if let Some(list) = batches.by_key.remove(&key) {
                for e in list {
                    if q_has.get(e).is_ok() {
                        commands.entity(e).despawn();
                    }
                }
            }
        }
    }
}

/// -------- helpers --------

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
                materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 1.0,
                    metallic: 0.0,
                    ..Default::default()
                })
            };
            Some((mesh_h, mat_h))
        }
        _ => None,
    }
}

/// Merge one source mesh into many instances.
/// (Identical to your previous function, just returns Option<Mesh>)
fn merge_mesh_instances(src: &Mesh, instances: &[Transform]) -> Option<Mesh> {
    let positions: Vec<[f32; 3]> = match src.attribute(Mesh::ATTRIBUTE_POSITION)? {
        VertexAttributeValues::Float32x3(v) => v.clone(),
        _ => return None,
    };
    let normals: Option<Vec<[f32; 3]>> = src
        .attribute(Mesh::ATTRIBUTE_NORMAL)
        .and_then(|vals| match vals {
            VertexAttributeValues::Float32x3(v) => Some(v.clone()),
            _ => None,
        });
    let uvs: Option<Vec<[f32; 2]>> = src
        .attribute(Mesh::ATTRIBUTE_UV_0)
        .and_then(|vals| match vals {
            VertexAttributeValues::Float32x2(v) => Some(v.clone()),
            _ => None,
        });

    let (src_indices, tri_list): (Option<Vec<u32>>, bool) = match src.indices() {
        Some(Indices::U32(v)) => (Some(v.clone()), true),
        Some(Indices::U16(v)) => (Some(v.iter().map(|&x| x as u32).collect()), true),
        None => (None, false),
    };

    let src_count = positions.len() as u32;
    let inst_n = instances.len().max(1);

    let mut out_positions = Vec::with_capacity(src_count as usize * inst_n);
    let mut out_normals: Option<Vec<[f32; 3]>> = normals.as_ref().map(|_| Vec::with_capacity(src_count as usize * inst_n));
    let mut out_uvs: Option<Vec<[f32; 2]>> = uvs.as_ref().map(|_| Vec::with_capacity(src_count as usize * inst_n));
    let mut out_indices: Vec<u32> = Vec::with_capacity(src_indices.as_ref().map(|ix| ix.len()).unwrap_or(0) * inst_n);

    for (inst_id, t) in instances.iter().enumerate() {
        let trs = Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation);
        let nrm_m = Mat3::from_mat4(trs).inverse().transpose();

        for (i, p) in positions.iter().enumerate() {
            let wp = trs * Vec4::new(p[0], p[1], p[2], 1.0);
            out_positions.push([wp.x, wp.y, wp.z]);

            if let (Some(src_ns), Some(dst_ns)) = (normals.as_ref(), out_normals.as_mut()) {
                let n = src_ns[i];
                let wn = nrm_m * Vec3::new(n[0], n[1], n[2]);
                let wn = wn.normalize_or_zero();
                dst_ns.push([wn.x, wn.y, wn.z]);
            }
            if let (Some(src_uv), Some(dst_uv)) = (uvs.as_ref(), out_uvs.as_mut()) {
                dst_uv.push(src_uv[i]);
            }
        }

        if let Some(ix) = &src_indices {
            let base = (inst_id as u32) * src_count;
            out_indices.extend(ix.iter().map(|&i| i + base));
        }
    }

    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, out_positions);
    if let Some(ns) = out_normals { mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, ns); }
    if let Some(uv) = out_uvs { mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv); }
    if tri_list { mesh.insert_indices(Indices::U32(out_indices)); }
    Some(mesh)
}
