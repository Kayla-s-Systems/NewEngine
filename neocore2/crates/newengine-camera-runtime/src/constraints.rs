#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_math::{Quat, Vec3};
use std::collections::HashMap;
use std::sync::Arc;

/// Declarative spring-arm constraint configuration for third-person cameras.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSpringArmConfig {
    pub enabled: bool,
    pub probe_radius: f32,
    pub collision_padding: f32,
    pub min_distance: f32,
}

impl Default for CameraSpringArmConfig {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: true,
            probe_radius: 0.18,
            collision_padding: 0.08,
            min_distance: 0.75,
        }
    }
}

impl CameraSpringArmConfig {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            enabled: self.enabled,
            probe_radius: sanitize_non_negative(self.probe_radius, 0.18).min(4.0),
            collision_padding: sanitize_non_negative(self.collision_padding, 0.08).min(2.0),
            min_distance: sanitize_non_negative(self.min_distance, 0.75).min(32.0),
        }
    }
}

/// Engine-neutral spherical collider proxy used by spring-arm constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSpringArmCollider {
    pub entity: EntityId,
    pub center_ws: Vec3,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSpringArmAabbCollider {
    pub entity: EntityId,
    pub min_ws: Vec3,
    pub max_ws: Vec3,
}

/// Engine-neutral static triangle mesh used by third-person spring-arm collision.
/// Geometry stays in entity-local space so the engine can share the authored arrays without
/// rebuilding world-space triangle proxies every render frame.
#[derive(Clone, Debug)]
pub struct CameraSpringArmMeshCollider {
    pub entity: EntityId,
    pub revision: u64,
    pub position_ws: Vec3,
    pub rotation_ws: Quat,
    pub min_ls: Vec3,
    pub max_ls: Vec3,
    pub vertices: Arc<[[f32; 3]]>,
    pub triangles: Arc<[[u32; 3]]>,
}

#[derive(Clone, Copy, Debug)]
struct CameraSpringArmMeshAccelNode {
    min_ls: Vec3,
    max_ls: Vec3,
    left: Option<usize>,
    right: Option<usize>,
    first_triangle: usize,
    triangle_count: usize,
}

#[derive(Clone, Debug, Default)]
struct CameraSpringArmMeshAccel {
    nodes: Vec<CameraSpringArmMeshAccelNode>,
    ordered_triangles: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
struct CameraSpringArmTriangleBuildRef {
    triangle_index: u32,
    min_ls: Vec3,
    max_ls: Vec3,
    centroid_ls: Vec3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CameraSpringArmCollisionTelemetry {
    pub sphere_count: usize,
    pub aabb_count: usize,
    pub mesh_count: usize,
    pub cached_mesh_count: usize,
    pub accel_builds_this_refresh: usize,
}

#[derive(Clone, Debug, Default)]
pub struct CameraSpringArmCollisionWorld {
    pub colliders: Vec<CameraSpringArmCollider>,
    pub aabbs: Vec<CameraSpringArmAabbCollider>,
    pub meshes: Vec<CameraSpringArmMeshCollider>,
    mesh_accel_cache: HashMap<u64, (u64, Arc<CameraSpringArmMeshAccel>)>,
    accel_builds_this_refresh: usize,
}

impl CameraSpringArmCollisionWorld {
    #[inline]
    pub fn clear(&mut self) {
        self.colliders.clear();
        self.aabbs.clear();
        self.meshes.clear();
        self.accel_builds_this_refresh = 0;
    }

    #[inline]
    pub fn telemetry(&self) -> CameraSpringArmCollisionTelemetry {
        CameraSpringArmCollisionTelemetry {
            sphere_count: self.colliders.len(),
            aabb_count: self.aabbs.len(),
            mesh_count: self.meshes.len(),
            cached_mesh_count: self.mesh_accel_cache.len(),
            accel_builds_this_refresh: self.accel_builds_this_refresh,
        }
    }

    #[inline]
    pub fn push(&mut self, collider: CameraSpringArmCollider) {
        if collider.radius.is_finite() && collider.radius > 0.0 && collider.center_ws.is_finite() {
            self.colliders.push(collider);
        }
    }

    #[inline]
    pub fn push_aabb(&mut self, collider: CameraSpringArmAabbCollider) {
        if collider.min_ws.is_finite()
            && collider.max_ws.is_finite()
            && collider.min_ws.x <= collider.max_ws.x
            && collider.min_ws.y <= collider.max_ws.y
            && collider.min_ws.z <= collider.max_ws.z
        {
            self.aabbs.push(collider);
        }
    }

    #[inline]
    pub fn push_mesh(&mut self, collider: CameraSpringArmMeshCollider) {
        if collider.position_ws.is_finite()
            && collider.rotation_ws.is_finite()
            && collider.min_ls.is_finite()
            && collider.max_ls.is_finite()
            && collider.min_ls.x <= collider.max_ls.x
            && collider.min_ls.y <= collider.max_ls.y
            && collider.min_ls.z <= collider.max_ls.z
            && !collider.vertices.is_empty()
            && !collider.triangles.is_empty()
        {
            let entity_key = collider.entity.stable_u64();
            let needs_rebuild = self
                .mesh_accel_cache
                .get(&entity_key)
                .map(|(revision, _)| *revision != collider.revision)
                .unwrap_or(true);
            if needs_rebuild {
                self.accel_builds_this_refresh = self.accel_builds_this_refresh.saturating_add(1);
                self.mesh_accel_cache.insert(
                    entity_key,
                    (
                        collider.revision,
                        Arc::new(build_mesh_accel(&collider.vertices, &collider.triangles)),
                    ),
                );
            }
            self.meshes.push(collider);
        }
    }
}

#[inline]
fn ray_aabb_entry_t(origin: Vec3, dir: Vec3, max_t: f32, min: Vec3, max: Vec3) -> Option<f32> {
    let mut t_min = 0.0f32;
    let mut t_max = max_t;
    for (o, d, lo, hi) in [
        (origin.x, dir.x, min.x, max.x),
        (origin.y, dir.y, min.y, max.y),
        (origin.z, dir.z, min.z, max.z),
    ] {
        if d.abs() <= 1.0e-7 {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d;
        let mut t1 = (lo - o) * inv;
        let mut t2 = (hi - o) * inv;
        if t1 > t2 {
            core::mem::swap(&mut t1, &mut t2);
        }
        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_min > t_max {
            return None;
        }
    }
    if t_max < 0.0 || t_min > max_t {
        None
    } else {
        Some(t_min.max(0.0))
    }
}

#[inline]
fn triangle_vertex(vertices: &[[f32; 3]], index: u32) -> Option<Vec3> {
    let value = vertices.get(index as usize)?;
    Some(Vec3::new(value[0], value[1], value[2]))
}

fn build_mesh_accel(vertices: &[[f32; 3]], triangles: &[[u32; 3]]) -> CameraSpringArmMeshAccel {
    let mut refs = Vec::with_capacity(triangles.len());
    for (triangle_index, triangle) in triangles.iter().copied().enumerate() {
        let (Some(a), Some(b), Some(c)) = (
            triangle_vertex(vertices, triangle[0]),
            triangle_vertex(vertices, triangle[1]),
            triangle_vertex(vertices, triangle[2]),
        ) else {
            continue;
        };
        let min_ls = a.min(b).min(c);
        let max_ls = a.max(b).max(c);
        refs.push(CameraSpringArmTriangleBuildRef {
            triangle_index: triangle_index as u32,
            min_ls,
            max_ls,
            centroid_ls: (a + b + c) / 3.0,
        });
    }
    let mut accel = CameraSpringArmMeshAccel::default();
    if !refs.is_empty() {
        build_mesh_accel_node(&mut refs, &mut accel);
    }
    accel
}

fn build_mesh_accel_node(
    refs: &mut [CameraSpringArmTriangleBuildRef],
    accel: &mut CameraSpringArmMeshAccel,
) -> usize {
    let mut min_ls = Vec3::splat(f32::INFINITY);
    let mut max_ls = Vec3::splat(f32::NEG_INFINITY);
    let mut centroid_min = Vec3::splat(f32::INFINITY);
    let mut centroid_max = Vec3::splat(f32::NEG_INFINITY);
    for triangle in refs.iter().copied() {
        min_ls = min_ls.min(triangle.min_ls);
        max_ls = max_ls.max(triangle.max_ls);
        centroid_min = centroid_min.min(triangle.centroid_ls);
        centroid_max = centroid_max.max(triangle.centroid_ls);
    }

    let node_index = accel.nodes.len();
    accel.nodes.push(CameraSpringArmMeshAccelNode {
        min_ls,
        max_ls,
        left: None,
        right: None,
        first_triangle: 0,
        triangle_count: 0,
    });

    const LEAF_TRIANGLES: usize = 12;
    if refs.len() <= LEAF_TRIANGLES {
        let first_triangle = accel.ordered_triangles.len();
        accel
            .ordered_triangles
            .extend(refs.iter().map(|triangle| triangle.triangle_index));
        accel.nodes[node_index].first_triangle = first_triangle;
        accel.nodes[node_index].triangle_count = refs.len();
        return node_index;
    }

    let extent = centroid_max - centroid_min;
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    refs.sort_unstable_by(|a, b| {
        let av = match axis {
            0 => a.centroid_ls.x,
            1 => a.centroid_ls.y,
            _ => a.centroid_ls.z,
        };
        let bv = match axis {
            0 => b.centroid_ls.x,
            1 => b.centroid_ls.y,
            _ => b.centroid_ls.z,
        };
        av.total_cmp(&bv)
    });
    let mid = refs.len() / 2;
    let (left_refs, right_refs) = refs.split_at_mut(mid);
    let left = build_mesh_accel_node(left_refs, accel);
    let right = build_mesh_accel_node(right_refs, accel);
    accel.nodes[node_index].left = Some(left);
    accel.nodes[node_index].right = Some(right);
    node_index
}

#[inline]
fn ray_triangle_t(origin: Vec3, dir: Vec3, max_t: f32, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    // Two-sided Moller-Trumbore. Static world meshes may contain mixed winding.
    let edge1 = b - a;
    let edge2 = c - a;
    let p = dir.cross(edge2);
    let det = edge1.dot(p);
    if !det.is_finite() || det.abs() <= 1.0e-7 {
        return None;
    }
    let inv_det = 1.0 / det;
    let tvec = origin - a;
    let u = tvec.dot(p) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = tvec.cross(edge1);
    let v = dir.dot(q) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = edge2.dot(q) * inv_det;
    (t.is_finite() && t >= 0.0 && t <= max_t).then_some(t)
}

#[inline]
fn mesh_ray_hit_t(
    collider: &CameraSpringArmMeshCollider,
    accel: &CameraSpringArmMeshAccel,
    origin_ws: Vec3,
    dir_ws: Vec3,
    max_t: f32,
) -> Option<f32> {
    let rotation = collider.rotation_ws.normalize_or_identity();
    let inv = rotation.inverse();
    let origin = inv * (origin_ws - collider.position_ws);
    let dir = (inv * dir_ws).normalize_or_zero();
    if dir.length_squared() <= 1.0e-12 || accel.nodes.is_empty() {
        return None;
    }

    let mut nearest = max_t;
    let mut hit = false;
    let mut stack = Vec::with_capacity(32);
    stack.push(0usize);
    while let Some(node_index) = stack.pop() {
        let Some(node) = accel.nodes.get(node_index).copied() else {
            continue;
        };
        if ray_aabb_entry_t(origin, dir, nearest, node.min_ls, node.max_ls).is_none() {
            continue;
        }
        if node.triangle_count != 0 {
            let end = node
                .first_triangle
                .saturating_add(node.triangle_count)
                .min(accel.ordered_triangles.len());
            for ordered_index in node.first_triangle..end {
                let Some(triangle_index) = accel.ordered_triangles.get(ordered_index).copied()
                else {
                    continue;
                };
                let Some(triangle) = collider.triangles.get(triangle_index as usize).copied()
                else {
                    continue;
                };
                let (Some(a), Some(b), Some(c)) = (
                    triangle_vertex(&collider.vertices, triangle[0]),
                    triangle_vertex(&collider.vertices, triangle[1]),
                    triangle_vertex(&collider.vertices, triangle[2]),
                ) else {
                    continue;
                };
                if let Some(t) = ray_triangle_t(origin, dir, nearest, a, b, c) {
                    nearest = t;
                    hit = true;
                }
            }
        } else {
            if let Some(left) = node.left {
                stack.push(left);
            }
            if let Some(right) = node.right {
                stack.push(right);
            }
        }
    }
    hit.then_some(nearest)
}

/// Applies a conservative sphere-proxy spring-arm constraint to a desired local offset.
///
/// The runtime keeps the *authored* offset in profile/config and computes a constrained offset
/// per frame, so collision never permanently deforms the camera rig.
#[inline]
pub fn constrain_spring_arm_offset_ls(
    target_entity: EntityId,
    target_pos: Vec3,
    target_rot: Quat,
    desired_offset_ls: Vec3,
    config: CameraSpringArmConfig,
    collision_world: Option<&CameraSpringArmCollisionWorld>,
) -> Vec3 {
    let config = config.sanitized();
    if !config.enabled {
        return desired_offset_ls;
    }
    let Some(collision_world) = collision_world else {
        return desired_offset_ls;
    };

    let target_rot = target_rot.normalize_or_identity();
    let desired_ws = target_rot * desired_offset_ls;
    let desired_dist = desired_ws.length();
    if desired_dist <= config.min_distance.max(0.001) {
        return desired_offset_ls;
    }

    let dir_ws = desired_ws / desired_dist;
    let max_t = desired_dist;
    let mut constrained_t = max_t;
    let expanded_padding = config.probe_radius + config.collision_padding;

    for collider in collision_world.colliders.iter().copied() {
        if collider.entity == target_entity {
            continue;
        }
        let radius = collider.radius.max(0.001) + expanded_padding;
        let oc = target_pos - collider.center_ws;
        let b = oc.dot(dir_ws);
        let c = oc.dot(oc) - radius * radius;
        let discriminant = b * b - c;
        if discriminant < 0.0 {
            continue;
        }
        let hit_t = -b - discriminant.sqrt();
        if hit_t.is_finite() && hit_t >= 0.0 && hit_t < constrained_t && hit_t <= max_t {
            constrained_t = hit_t;
        }
    }

    for collider in collision_world.aabbs.iter().copied() {
        if collider.entity == target_entity {
            continue;
        }
        let expand = Vec3::splat(expanded_padding);
        let min = collider.min_ws - expand;
        let max = collider.max_ws + expand;
        if let Some(hit_t) = ray_aabb_entry_t(target_pos, dir_ws, max_t, min, max) {
            if hit_t.is_finite() && hit_t < constrained_t {
                constrained_t = hit_t;
            }
        }
    }

    for collider in &collision_world.meshes {
        if collider.entity == target_entity {
            continue;
        }
        let entity_key = collider.entity.stable_u64();
        let Some((cached_revision, accel)) = collision_world.mesh_accel_cache.get(&entity_key)
        else {
            continue;
        };
        if *cached_revision != collider.revision {
            continue;
        }
        if let Some(surface_t) = mesh_ray_hit_t(collider, accel, target_pos, dir_ws, max_t) {
            // Triangle intersection gives the actual surface rather than the enclosing mesh AABB.
            // Pull the camera sphere inward by its radius; collision_padding is applied uniformly
            // below with the other proxy types.
            let hit_t = (surface_t - config.probe_radius).max(0.0);
            if hit_t.is_finite() && hit_t < constrained_t {
                constrained_t = hit_t;
            }
        }
    }

    if constrained_t >= max_t {
        return desired_offset_ls;
    }

    let safe_t = (constrained_t - config.collision_padding).max(config.min_distance.min(max_t));
    let constrained_ws = dir_ws * safe_t;
    target_rot.inverse() * constrained_ws
}

#[inline]
fn sanitize_non_negative(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_ecs::World;

    #[test]
    fn aabb_collision_retracts_spring_arm() {
        let mut ecs = World::new();
        let target = ecs.spawn();
        let wall = ecs.spawn();
        let mut world = CameraSpringArmCollisionWorld::default();
        world.push_aabb(CameraSpringArmAabbCollider {
            entity: wall,
            min_ws: Vec3::new(-2.0, -2.0, 2.0),
            max_ws: Vec3::new(2.0, 2.0, 2.2),
        });
        let desired = Vec3::new(0.0, 0.0, 4.0);
        let constrained = constrain_spring_arm_offset_ls(
            target,
            Vec3::ZERO,
            Quat::IDENTITY,
            desired,
            CameraSpringArmConfig::default(),
            Some(&world),
        );
        assert!(constrained.length() < desired.length());
        assert!(constrained.z > 0.75);
    }

    #[test]
    fn mesh_collision_uses_triangles_not_enclosing_aabb_volume() {
        let mut ecs = World::new();
        let target = ecs.spawn();
        let mesh = ecs.spawn();
        let vertices: Arc<[[f32; 3]]> = Arc::from(
            vec![
                [-10.0, -10.0, -10.0],
                [-9.0, -10.0, -10.0],
                [-10.0, -9.0, -10.0],
                [10.0, 10.0, 10.0],
            ]
            .into_boxed_slice(),
        );
        let triangles: Arc<[[u32; 3]]> = Arc::from(vec![[0, 1, 2]].into_boxed_slice());
        let mut world = CameraSpringArmCollisionWorld::default();
        world.push_mesh(CameraSpringArmMeshCollider {
            entity: mesh,
            revision: 1,
            position_ws: Vec3::ZERO,
            rotation_ws: Quat::IDENTITY,
            min_ls: Vec3::splat(-10.0),
            max_ls: Vec3::splat(10.0),
            vertices,
            triangles,
        });
        let desired = Vec3::new(0.0, 0.0, 4.0);
        let constrained = constrain_spring_arm_offset_ls(
            target,
            Vec3::ZERO,
            Quat::IDENTITY,
            desired,
            CameraSpringArmConfig::default(),
            Some(&world),
        );
        assert_eq!(constrained, desired);
    }

    #[test]
    fn mesh_triangle_retracts_spring_arm_at_actual_surface() {
        let mut ecs = World::new();
        let target = ecs.spawn();
        let mesh = ecs.spawn();
        let vertices: Arc<[[f32; 3]]> = Arc::from(
            vec![[-2.0, -2.0, 2.0], [2.0, -2.0, 2.0], [0.0, 2.0, 2.0]].into_boxed_slice(),
        );
        let triangles: Arc<[[u32; 3]]> = Arc::from(vec![[0, 1, 2]].into_boxed_slice());
        let mut world = CameraSpringArmCollisionWorld::default();
        world.push_mesh(CameraSpringArmMeshCollider {
            entity: mesh,
            revision: 1,
            position_ws: Vec3::ZERO,
            rotation_ws: Quat::IDENTITY,
            min_ls: Vec3::new(-2.0, -2.0, 2.0),
            max_ls: Vec3::new(2.0, 2.0, 2.0),
            vertices,
            triangles,
        });
        let desired = Vec3::new(0.0, 0.0, 4.0);
        let constrained = constrain_spring_arm_offset_ls(
            target,
            Vec3::ZERO,
            Quat::IDENTITY,
            desired,
            CameraSpringArmConfig::default(),
            Some(&world),
        );
        assert!(constrained.z > 0.75);
        assert!(constrained.z < 2.0);
    }

    #[test]
    fn mesh_acceleration_cache_does_not_thrash_above_256_colliders() {
        let mut ecs = World::new();
        let vertices: Arc<[[f32; 3]]> = Arc::from(
            vec![[-1.0, -1.0, 2.0], [1.0, -1.0, 2.0], [0.0, 1.0, 2.0]].into_boxed_slice(),
        );
        let triangles: Arc<[[u32; 3]]> = Arc::from(vec![[0, 1, 2]].into_boxed_slice());
        let mut world = CameraSpringArmCollisionWorld::default();
        let mut first_entity = None;
        for index in 0..320u32 {
            let entity = ecs.spawn();
            first_entity.get_or_insert(entity);
            world.push_mesh(CameraSpringArmMeshCollider {
                entity,
                revision: 1,
                position_ws: Vec3::new(index as f32 * 0.01, 0.0, 0.0),
                rotation_ws: Quat::IDENTITY,
                min_ls: Vec3::new(-1.0, -1.0, 2.0),
                max_ls: Vec3::new(1.0, 1.0, 2.0),
                vertices: Arc::clone(&vertices),
                triangles: Arc::clone(&triangles),
            });
        }
        assert_eq!(world.mesh_accel_cache.len(), 320);
        let first_key = first_entity.unwrap().stable_u64();
        assert!(world.mesh_accel_cache.contains_key(&first_key));

        // A revision replacement updates only that entity and never evicts unrelated meshes.
        let replacement = ecs.spawn();
        world.push_mesh(CameraSpringArmMeshCollider {
            entity: replacement,
            revision: 1,
            position_ws: Vec3::ZERO,
            rotation_ws: Quat::IDENTITY,
            min_ls: Vec3::new(-1.0, -1.0, 2.0),
            max_ls: Vec3::new(1.0, 1.0, 2.0),
            vertices: Arc::clone(&vertices),
            triangles: Arc::clone(&triangles),
        });
        let before = world.mesh_accel_cache.len();
        world.push_mesh(CameraSpringArmMeshCollider {
            entity: replacement,
            revision: 2,
            position_ws: Vec3::ZERO,
            rotation_ws: Quat::IDENTITY,
            min_ls: Vec3::new(-1.0, -1.0, 2.0),
            max_ls: Vec3::new(1.0, 1.0, 2.0),
            vertices,
            triangles,
        });
        assert_eq!(world.mesh_accel_cache.len(), before);
        assert_eq!(world.mesh_accel_cache[&replacement.stable_u64()].0, 2);
        assert!(world.mesh_accel_cache.contains_key(&first_key));
    }

    #[test]
    fn target_collider_is_ignored() {
        let mut ecs = World::new();
        let target = ecs.spawn();
        let mut world = CameraSpringArmCollisionWorld::default();
        world.push_aabb(CameraSpringArmAabbCollider {
            entity: target,
            min_ws: Vec3::new(-1.0, -1.0, 0.5),
            max_ws: Vec3::new(1.0, 1.0, 2.0),
        });
        let desired = Vec3::new(0.0, 0.0, 4.0);
        let constrained = constrain_spring_arm_offset_ls(
            target,
            Vec3::ZERO,
            Quat::IDENTITY,
            desired,
            CameraSpringArmConfig::default(),
            Some(&world),
        );
        assert_eq!(constrained, desired);
    }
}
