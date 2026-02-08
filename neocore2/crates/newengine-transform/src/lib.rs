#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{Mat4, Quat, Vec3};
use hashbrown::HashMap;
use newengine_ecs::{EntityId, World};

/// Local transform relative to parent.
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    #[inline]
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    #[inline]
    pub fn to_mat4(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}

/// World-space transform (derived).
#[derive(Clone, Copy, Debug)]
pub struct GlobalTransform(pub Mat4);

impl Default for GlobalTransform {
    #[inline]
    fn default() -> Self {
        Self(Mat4::IDENTITY)
    }
}

/// Parent link (tree).
#[derive(Clone, Copy, Debug)]
pub struct Parent(pub EntityId);

/// Children list (maintained by editor/gameplay code).
#[derive(Clone, Debug, Default)]
pub struct Children(pub Vec<EntityId>);

/// Marks a node as needing recomputation (optional, can be used later for incremental updates).
#[derive(Clone, Copy, Debug, Default)]
pub struct TransformDirty;

/// Attaches `child` under `parent` and keeps `Children` lists consistent.
///
/// This does not attempt to prevent cycles automatically; cycle protection is handled in propagation.
#[inline]
pub fn set_parent(world: &mut World, child: EntityId, parent: Option<EntityId>) -> bool {
    if !world.exists(child) {
        return false;
    }

    let old_parent = world.get::<Parent>(child).map(|p| p.0);

    if let Some(op) = old_parent {
        if let Some(ch) = world.get_mut::<Children>(op) {
            ch.0.retain(|&e| e != child);
        }
    }

    match parent {
        Some(p) => {
            if !world.exists(p) {
                let _ = world.remove::<Parent>(child);
                return true;
            }
            let _ = world.insert(child, Parent(p));
            if world.get::<Children>(p).is_none() {
                let _ = world.insert(p, Children::default());
            }
            if let Some(ch) = world.get_mut::<Children>(p) {
                if !ch.0.iter().any(|&e| e == child) {
                    ch.0.push(child);
                }
            }
        }
        None => {
            let _ = world.remove::<Parent>(child);
        }
    }

    let _ = world.insert(child, TransformDirty);
    true
}

/// Propagates `Transform` + hierarchy into `GlobalTransform`.
///
/// Properties:
/// - deterministic and stable for a fixed World state
/// - cycle-safe (cycles are ignored; nodes in cycles fall back to local space)
/// - tolerant to broken parents (missing parent treated as root)
#[inline]
pub fn propagate_transforms(world: &mut World) {
    // Ensure GlobalTransform exists for all entities that have Transform.
    {
        let ids: Vec<EntityId> = world.query::<Transform>().map(|(id, _)| id).collect();
        for id in ids {
            if world.get::<GlobalTransform>(id).is_none() {
                let _ = world.insert(id, GlobalTransform::default());
            }
        }
    }

    // Snapshot phase (immutable reads).
    let mut locals: HashMap<EntityId, Mat4> = HashMap::new();
    let mut parents: HashMap<EntityId, EntityId> = HashMap::new();
    let mut order: Vec<EntityId> = Vec::new();

    for (id, t) in world.query::<Transform>() {
        order.push(id);
        locals.insert(id, t.to_mat4());
    }
    for (id, p) in world.query::<Parent>() {
        parents.insert(id, p.0);
    }

    // Compute phase.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum VisitState {
        Visiting,
        Done,
    }

    let mut state: HashMap<EntityId, VisitState> = HashMap::new();
    let mut computed: HashMap<EntityId, Mat4> = HashMap::new();

    fn eval(
        id: EntityId,
        locals: &HashMap<EntityId, Mat4>,
        parents: &HashMap<EntityId, EntityId>,
        state: &mut HashMap<EntityId, VisitState>,
        computed: &mut HashMap<EntityId, Mat4>,
    ) -> Mat4 {
        if let Some(m) = computed.get(&id) {
            return *m;
        }

        match state.get(&id).copied() {
            Some(VisitState::Visiting) => {
                // Cycle detected: fall back to local transform.
                let m = locals.get(&id).copied().unwrap_or(Mat4::IDENTITY);
                computed.insert(id, m);
                return m;
            }
            Some(VisitState::Done) => {
                return computed.get(&id).copied().unwrap_or(Mat4::IDENTITY);
            }
            None => {}
        }

        state.insert(id, VisitState::Visiting);

        let local = locals.get(&id).copied().unwrap_or(Mat4::IDENTITY);
        let out = if let Some(&pid) = parents.get(&id) {
            if locals.contains_key(&pid) {
                eval(pid, locals, parents, state, computed) * local
            } else {
                local
            }
        } else {
            local
        };

        state.insert(id, VisitState::Done);
        computed.insert(id, out);
        out
    }

    for &id in &order {
        let _ = eval(id, &locals, &parents, &mut state, &mut computed);
    }

    // Write-back phase (mutable writes).
    for id in order {
        if let Some(m) = computed.get(&id).copied() {
            if let Some(gt) = world.get_mut::<GlobalTransform>(id) {
                gt.0 = m;
            }
        }
        let _ = world.remove::<TransformDirty>(id);
    }
}