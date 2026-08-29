#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_math::collections::{FxHashMap, FxHashSet};
use newengine_transform_api::EntityHandle;
use newengine_transform_api::{Children, Parent, Transform, TransformDirty};

#[inline]
fn handle(id: EntityId) -> EntityHandle {
    EntityHandle::from(id)
}

#[inline]
fn resolve_transform_entity(world: &World, entity: EntityHandle) -> Option<EntityId> {
    world
        .query::<Transform>()
        .find(|(id, _)| id.stable_u64() == entity.stable_id)
        .map(|(id, _)| id)
}

/// Attaches `child` under `parent` and keeps `Children` lists consistent.
///
/// Notes:
/// - Does not automatically prevent cycles. Cycle handling is done in propagation.
/// - If `parent` doesn't exist, parent link is removed (child becomes root).
#[inline]
pub fn set_parent(world: &mut World, child: EntityId, parent: Option<EntityId>) -> bool {
    if !world.exists(child) {
        return false;
    }

    if parent == Some(child) {
        return false;
    }

    let child_handle = handle(child);

    // Remove from previous parent's children list.
    let prev_parent = world.get::<Parent>(child).map(|p| p.0);
    if let Some(pp) = prev_parent.and_then(|h| resolve_transform_entity(world, h)) {
        if let Some(ch) = world.get_mut::<Children>(pp) {
            if let Some(pos) = ch.0.iter().position(|&e| e == child_handle) {
                ch.0.swap_remove(pos);
                world.mark_changed::<Children>(pp);
            }
        }
    }

    // Update parent component.
    match parent {
        Some(p) if world.exists(p) => {
            let parent_handle = handle(p);
            let _ = world.insert(child, Parent(parent_handle));

            // Ensure parent has children list.
            if world.get::<Children>(p).is_none() {
                let _ = world.insert(p, Children::default());
            }

            if let Some(ch) = world.get_mut::<Children>(p) {
                if !ch.0.contains(&child_handle) {
                    ch.0.push(child_handle);
                    world.mark_changed::<Children>(p);
                }
            }
        }
        _ => {
            let _ = world.remove::<Parent>(child);
        }
    }

    // Parent topology affects world-space evaluation.
    // We mark the child dirty so runtimes can cheaply gate derived updates.
    let _ = world.insert(child, TransformDirty);

    true
}

/// Despawn one transform hierarchy, children first, while keeping external parent lists valid.
///
/// For unloading several world-partition cells in one tick prefer [`despawn_hierarchies`], which
/// builds the service-handle resolver only once for the whole batch.
#[inline]
pub fn despawn_hierarchy(world: &mut World, root: EntityId) -> usize {
    despawn_hierarchies(world, [root])
}

/// Despawn multiple hierarchy roots as one structural batch.
///
/// `Children` stores service-safe [`EntityHandle`] values rather than native generational keys.
/// Reversing those handles by scanning the ECS once per root would make cell eviction scale as
/// O(world * unloaded_cells). This routine creates one stable-id resolver for the batch, traverses
/// only descendants, unlinks boundaries from surviving parents, then despawns children first.
pub fn despawn_hierarchies(world: &mut World, roots: impl IntoIterator<Item = EntityId>) -> usize {
    let roots = roots
        .into_iter()
        .filter(|entity| world.exists(*entity))
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return 0;
    }

    let mut resolver = FxHashMap::<u64, EntityId>::default();
    resolver.reserve(world.entity_count());
    for entity in world.iter_entities() {
        resolver.insert(entity.stable_u64(), entity);
    }

    let mut visited = FxHashSet::<EntityId>::default();
    let mut stack = roots;
    let mut delete_order = Vec::<EntityId>::new();
    while let Some(entity) = stack.pop() {
        if !world.exists(entity) || !visited.insert(entity) {
            continue;
        }
        delete_order.push(entity);
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(
                children
                    .0
                    .iter()
                    .filter_map(|child| resolver.get(&child.stable_id).copied()),
            );
        }
    }

    // Remove only hierarchy edges that cross from the deletion set into surviving parents.
    // Internal Parent/Children components disappear with their entities below.
    for &entity in &delete_order {
        let Some(parent_handle) = world.get::<Parent>(entity).map(|parent| parent.0) else {
            continue;
        };
        let Some(parent) = resolver.get(&parent_handle.stable_id).copied() else {
            continue;
        };
        if visited.contains(&parent) || !world.exists(parent) {
            continue;
        }
        let entity_handle = handle(entity);
        if let Some(children) = world.get_mut::<Children>(parent) {
            let before = children.0.len();
            children.0.retain(|child| *child != entity_handle);
            if children.0.len() != before {
                world.mark_changed::<Children>(parent);
            }
        }
    }

    let mut removed = 0usize;
    for entity in delete_order.into_iter().rev() {
        removed += usize::from(world.despawn(entity));
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform() -> Transform {
        Transform::default()
    }

    #[test]
    fn hierarchy_despawn_removes_descendants_and_unlinks_surviving_parent() {
        let mut world = World::new();
        let outside = world.spawn();
        let root = world.spawn();
        let child = world.spawn();
        let sibling = world.spawn();
        let grandchild = world.spawn();
        for entity in [outside, root, child, sibling, grandchild] {
            let _ = world.insert(entity, transform());
        }
        assert!(set_parent(&mut world, root, Some(outside)));
        assert!(set_parent(&mut world, child, Some(root)));
        assert!(set_parent(&mut world, sibling, Some(root)));
        assert!(set_parent(&mut world, grandchild, Some(child)));

        assert_eq!(despawn_hierarchy(&mut world, root), 4);
        assert!(world.exists(outside));
        assert!(!world.exists(root));
        assert!(!world.exists(child));
        assert!(!world.exists(sibling));
        assert!(!world.exists(grandchild));
        assert!(world
            .get::<Children>(outside)
            .is_none_or(|children| children.0.is_empty()));
    }

    #[test]
    fn hierarchy_batch_despawn_handles_cycles_without_revisiting_entities() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        for entity in [a, b] {
            let _ = world.insert(entity, transform());
        }
        assert!(set_parent(&mut world, b, Some(a)));
        assert!(set_parent(&mut world, a, Some(b)));

        assert_eq!(despawn_hierarchies(&mut world, [a, b]), 2);
        assert_eq!(world.entity_count(), 0);
    }
}
