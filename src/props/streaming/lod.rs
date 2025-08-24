// src/props/streaming/lod.rs
use bevy::prelude::*;

#[derive(Component, Debug, Clone)]
pub struct LodGroup {
    pub start_distances: Vec<f32>,
    pub active_ix: usize,
}
impl Default for LodGroup {
    fn default() -> Self {
        Self { start_distances: Vec::new(), active_ix: usize::MAX }
    }
}

pub fn update_lod_groups(
    cam_q: Query<&GlobalTransform, With<Camera3d>>,
    mut groups: Query<(&GlobalTransform, &mut LodGroup, &Children)>,
    mut vis_q: Query<&mut Visibility>,
) {
    let Ok(cam_gt) = cam_q.single() else { return; };
    let cam_pos = cam_gt.translation();

    for (gt, mut group, children) in &mut groups {
        if group.start_distances.is_empty() || children.is_empty() { continue; }

        let dist = gt.translation().distance(cam_pos);

        // pick highest i with start_distances[i] <= dist
        let mut chosen = 0usize;
        for (i, d) in group.start_distances.iter().enumerate() {
            if dist >= *d { chosen = i; } else { break; }
        }
        if chosen == group.active_ix && group.active_ix != usize::MAX { continue; }
        group.active_ix = chosen;

        // toggle child vis
        for (i, child) in children.iter().enumerate() {
            if let Ok(mut vis) = vis_q.get_mut(child) {
                if i == chosen {
                    vis.set_if_neq(Visibility::Visible);
                } else {
                    vis.set_if_neq(Visibility::Hidden);
                }
            }
        }
    }
}
