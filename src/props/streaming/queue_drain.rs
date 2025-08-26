// src/props/streaming/queue_drain.rs  (or put at end of streaming/mod.rs if you prefer)
use bevy::prelude::*;
use crate::props::queue::{SpawnQueue, SpawnQueueConfig};
use crate::props::streaming::spawn_prop_instance;

pub fn drain_spawn_queue(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut queue: ResMut<SpawnQueue>,
    cfg: Res<SpawnQueueConfig>,
) {
    // take up to N items this frame
    let take = cfg.max_per_frame.min(queue.items.len());
    if take == 0 { return; }

    // pop from the end (cheap); order doesn’t matter for visual parity
    for _ in 0..take {
        if let Some(req) = queue.items.pop() {
            spawn_prop_instance(
                &mut commands,
                &assets,
                req.id,
                req.chunk,
                &req.render,
                req.transform,
            );
        }
    }
}
