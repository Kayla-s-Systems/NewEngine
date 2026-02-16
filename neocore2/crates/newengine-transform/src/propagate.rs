#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{EulerRot, Mat4};
use newengine_ecs::{EntityId, World};
use slotmap::Key;

use crate::{GlobalTransform, Parent, Transform, TransformDirty, WorldPose};

/// Reusable buffers for transform propagation.
///
/// Stored as a `World` resource to avoid per-frame heap churn in editor/runtime.
///
/// Notes:
/// - This is intentionally *not* `pub(crate)` because the engine may want to pre-warm capacities
///   or inspect stats in tooling.
/// - Contents are scratch-only; never rely on values persisting across calls.
#[derive(Default)]
pub struct TransformPropagationScratch {
    ids: Vec<EntityId>,
    locals: Vec<Mat4>,
    parents: Vec<Option<EntityId>>,
    vis: Vec<u8>,
    out: Vec<Mat4>,
    stack: Vec<(usize, u8)>,
}


/// Ensures derived outputs (`GlobalTransform`, `WorldPose`) exist for all entities with `Transform`.
///
/// Call once per frame before propagation (or on-demand when authoring).
#[inline]
pub fn ensure_transform_outputs(world: &mut World) {
    let ids: Vec<EntityId> = world.query::<Transform>().map(|(id, _)| id).collect();
    for id in ids {
        if world.get::<GlobalTransform>(id).is_none() {
            let _ = world.insert(id, GlobalTransform::default());
        }
        if world.get::<WorldPose>(id).is_none() {
            let _ = world.insert(id, WorldPose::default());
        }
    }
}

/// Propagates `Transform` + hierarchy into `GlobalTransform` and `WorldPose`.
///
/// Properties:
/// - deterministic for a fixed World state (stable ordering by EntityId)
/// - cycle-safe (cycles degrade to local-space roots)
/// - tolerant to broken parents (missing parent treated as root)
/// - no per-entity HashMap allocations; uses dense vectors
#[inline]
pub fn propagate_transforms(world: &mut World) {
    ensure_transform_outputs(world);

    // Allocate once, reuse forever.
    if world.resource::<TransformPropagationScratch>().is_none() {
        world.insert_resource(TransformPropagationScratch::default());
    }

    let scratch = world
        .resource_mut::<TransformPropagationScratch>()
        .expect("TransformPropagationScratch must exist");

    // 1) Collect all entities that have Transform, deterministically ordered.
    scratch.ids.clear();
    scratch.ids.extend(world.query::<Transform>().map(|(id, _)| id));
    scratch.ids.sort_unstable_by_key(|e| e.data().as_ffi());
    scratch.ids.dedup();

    if scratch.ids.is_empty() {
        return;
    }

    // 2) Build index mapping (EntityId -> idx) without HashMap:
    // We can’t avoid a map completely unless we accept O(n^2) parent lookup.
    // But we can make it deterministic and allocate once per call: Vec + binary_search.
    //
    // We binary_search on sorted ids, so parent->idx is O(log n).
    scratch.locals.clear();
    scratch.locals.reserve(scratch.ids.len());
    for &id in scratch.ids.iter() {
        let local = world
            .get::<Transform>(id)
            .copied()
            .unwrap_or_default()
            .to_mat4();
        scratch.locals.push(local);
    }

    scratch.parents.clear();
    scratch.parents.reserve(scratch.ids.len());
    for &id in scratch.ids.iter() {
        scratch.parents.push(world.get::<Parent>(id).map(|p| p.0));
    }

    // 3) Iterative DFS with 3-state visitation.
    // 0 = Unvisited, 1 = Visiting (in stack), 2 = Done
    scratch.vis.clear();
    scratch.vis.resize(scratch.ids.len(), 0);

    scratch.out.clear();
    scratch.out.resize(scratch.ids.len(), Mat4::IDENTITY);

    // Stack frames: (node_idx, phase)
    // phase 0 = enter, phase 1 = exit (after children/parent handled)
    scratch.stack.clear();
    scratch.stack.reserve(scratch.ids.len() * 2);

    for start in 0..scratch.ids.len() {
        if scratch.vis[start] != 0 {
            continue;
        }

        scratch.stack.push((start, 0));

        while let Some((i, phase)) = scratch.stack.pop() {
            match phase {
                0 => {
                    match scratch.vis[i] {
                        2 => continue, // already done
                        1 => {
                            // Cycle edge reached: degrade to local as root.
                            scratch.out[i] = scratch.locals[i];
                            scratch.vis[i] = 2;
                            continue;
                        }
                        _ => {}
                    }

                    scratch.vis[i] = 1; // visiting
                    scratch.stack.push((i, 1)); // exit later

                    // Push parent first (so it computes before node).
                    if let Some(pid) = scratch.parents[i] {
                        if let Ok(pidx) = scratch.ids.binary_search(&pid) {
                            if scratch.vis[pidx] != 2 {
                                scratch.stack.push((pidx, 0));
                            }
                        }
                    }
                }
                _ => {
                    // exit: compute node from parent (if valid & computed) * local.
                    let local = scratch.locals[i];
                    let composed = if let Some(pid) = scratch.parents[i] {
                        if let Ok(pidx) = scratch.ids.binary_search(&pid) {
                            if scratch.vis[pidx] == 2 {
                                scratch.out[pidx] * local
                            } else {
                                // Parent in cycle or unresolved -> treat as root.
                                local
                            }
                        } else {
                            // Parent missing Transform -> root.
                            local
                        }
                    } else {
                        local
                    };

                    scratch.out[i] = composed;
                    scratch.vis[i] = 2;
                }
            }
        }
    }

    // 4) Write-back.
    for (i, &id) in scratch.ids.iter().enumerate() {
        let m = scratch.out[i];

        if let Some(gt) = world.get_mut_tracked::<GlobalTransform>(id) {
            gt.0 = m;
        }

        if let Some(wp) = world.get_mut_tracked::<WorldPose>(id) {
            let (scale, rot, trans) = m.to_scale_rotation_translation();
            let (yaw, pitch, roll) = rot.to_euler(EulerRot::YXZ);

            wp.world_pos = trans;
            wp.yaw = yaw;
            wp.pitch = pitch;
            wp.roll = roll;
            wp.world_scale = scale;
        }

        let _ = world.remove::<TransformDirty>(id);
    }
}