#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{EulerRot, Mat4};
use newengine_ecs::{EntityId, World};
use slotmap::Key;

use crate::{GlobalTransform, Parent, Transform, TransformDirty, WorldPose};


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

    // 1) Collect all entities that have Transform, deterministically ordered.
    let mut ids: Vec<EntityId> = world.query::<Transform>().map(|(id, _)| id).collect();
    ids.sort_unstable_by_key(|e| e.data().as_ffi());
    ids.dedup();

    if ids.is_empty() {
        return;
    }

    // 2) Build index mapping (EntityId -> idx) without HashMap:
    // We can’t avoid a map completely unless we accept O(n^2) parent lookup.
    // But we can make it deterministic and allocate once per call: Vec + binary_search.
    //
    // We binary_search on sorted ids, so parent->idx is O(log n).
    let locals: Vec<Mat4> = ids
        .iter()
        .map(|&id| world.get::<Transform>(id).copied().unwrap_or_default().to_mat4())
        .collect();

    let parents: Vec<Option<EntityId>> = ids
        .iter()
        .map(|&id| world.get::<Parent>(id).map(|p| p.0))
        .collect();

    // 3) Iterative DFS with 3-state visitation.
    // 0 = Unvisited, 1 = Visiting (in stack), 2 = Done
    let mut vis: Vec<u8> = vec![0; ids.len()];
    let mut out: Vec<Mat4> = vec![Mat4::IDENTITY; ids.len()];

    // Stack frames: (node_idx, phase)
    // phase 0 = enter, phase 1 = exit (after children/parent handled)
    let mut stack: Vec<(usize, u8)> = Vec::with_capacity(ids.len() * 2);

    for start in 0..ids.len() {
        if vis[start] != 0 {
            continue;
        }

        stack.push((start, 0));

        while let Some((i, phase)) = stack.pop() {
            match phase {
                0 => {
                    match vis[i] {
                        2 => continue, // already done
                        1 => {
                            // Cycle edge reached: degrade to local as root.
                            out[i] = locals[i];
                            vis[i] = 2;
                            continue;
                        }
                        _ => {}
                    }

                    vis[i] = 1; // visiting
                    stack.push((i, 1)); // exit later

                    // Push parent first (so it computes before node).
                    if let Some(pid) = parents[i] {
                        if let Ok(pidx) = ids.binary_search(&pid) {
                            if vis[pidx] != 2 {
                                stack.push((pidx, 0));
                            }
                        }
                    }
                }
                _ => {
                    // exit: compute node from parent (if valid & computed) * local.
                    let local = locals[i];
                    let composed = if let Some(pid) = parents[i] {
                        if let Ok(pidx) = ids.binary_search(&pid) {
                            if vis[pidx] == 2 {
                                out[pidx] * local
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

                    out[i] = composed;
                    vis[i] = 2;
                }
            }
        }
    }

    // 4) Write-back.
    for (i, &id) in ids.iter().enumerate() {
        let m = out[i];

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