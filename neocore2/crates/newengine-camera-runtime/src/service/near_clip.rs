use super::*;

#[derive(Clone, Copy, Debug)]
struct GameplayNearClipState {
    target: EntityId,
    runner: GameplayCameraRunnerKind,
    current_near: f32,
    initialized: bool,
}

impl Default for GameplayNearClipState {
    fn default() -> Self {
        Self {
            target: EntityId::default(),
            runner: GameplayCameraRunnerKind::FirstPerson,
            current_near: 0.0,
            initialized: false,
        }
    }
}

#[inline]
fn near_clip_limits(config: CameraRuntimeServiceConfig) -> (f32, f32) {
    match config.runner {
        GameplayCameraRunnerKind::FirstPerson => {
            let min = config.first_person_near.clamp(0.005, 0.50);
            let max = config.near_clip_first_person_max_distance.clamp(min, 2.0);
            (min, max)
        }
        _ => {
            let min = config.near_clip_third_person_min_distance.clamp(0.005, 2.0);
            let max = config.near_clip_third_person_max_distance.clamp(min, 4.0);
            (min, max)
        }
    }
}

/// Conservative frustum-volume approximation using a 3x3 family of swept sphere rays.
///
/// The scanner intentionally reuses `CameraSpringArmCollisionWorld`; near-clip protection must not
/// create a second physics/query authority. Each ray terminates on the candidate near plane, so the
/// minimum forward depth reached by any constrained ray is a safe upper bound for that plane.
#[allow(clippy::too_many_arguments)]
fn scan_safe_near_clip(
    target: EntityId,
    camera_position: Vec3,
    camera_rotation: Quat,
    fov_y_radians: f32,
    aspect: f32,
    min_near: f32,
    max_near: f32,
    probe_radius: f32,
    pull_in_distance: f32,
    collision_world: Option<&CameraSpringArmCollisionWorld>,
) -> f32 {
    let Some(collision_world) = collision_world else {
        // Without an authoritative query scene we cannot prove that extending the plane is safe.
        return min_near;
    };
    if !camera_position.is_finite() || !camera_rotation.is_finite() {
        return min_near;
    }
    let camera_rotation = camera_rotation.normalize_or_identity();
    let forward = (camera_rotation * -Vec3::Z).normalize_or_zero();
    let right = (camera_rotation * Vec3::X).normalize_or_zero();
    let up = (camera_rotation * Vec3::Y).normalize_or_zero();
    if forward.length_squared() <= 1.0e-10
        || right.length_squared() <= 1.0e-10
        || up.length_squared() <= 1.0e-10
    {
        return min_near;
    }

    let fov_y = if fov_y_radians.is_finite() {
        fov_y_radians.clamp(10.0_f32.to_radians(), 150.0_f32.to_radians())
    } else {
        60.0_f32.to_radians()
    };
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect.clamp(0.25, 8.0)
    } else {
        16.0 / 9.0
    };
    let half_height = max_near * (0.5 * fov_y).tan();
    let half_width = half_height * aspect;
    let probe_radius = if probe_radius.is_finite() {
        probe_radius.clamp(0.0, 0.25)
    } else {
        0.0
    };
    let pull_in_distance = if pull_in_distance.is_finite() {
        pull_in_distance.clamp(0.0, 0.5)
    } else {
        0.0
    };

    let query = CameraSpringArmConfig {
        enabled: true,
        probe_radius,
        collision_padding: 0.0,
        min_distance: 0.0,
    };
    let mut safe_near = max_near;
    for v in [-1.0_f32, 0.0, 1.0] {
        for u in [-1.0_f32, 0.0, 1.0] {
            let ray_to_plane =
                forward * max_near + right * (u * half_width) + up * (v * half_height);
            if !ray_to_plane.is_finite() || ray_to_plane.length_squared() <= 1.0e-12 {
                continue;
            }
            let constrained = constrain_spring_arm_offset_ls(
                target,
                camera_position,
                Quat::IDENTITY,
                ray_to_plane,
                query,
                Some(collision_world),
            );
            let forward_depth = constrained.dot(forward);
            if forward_depth.is_finite() {
                safe_near = safe_near.min(forward_depth);
            }
        }
    }

    if safe_near >= max_near - 1.0e-5 {
        max_near
    } else {
        (safe_near - pull_in_distance).clamp(min_near, max_near)
    }
}

#[inline]
fn step_near_clip_response(
    current: f32,
    target: f32,
    dt: f32,
    release_time_seconds: f32,
    hysteresis_m: f32,
) -> f32 {
    if !current.is_finite() || current <= 0.0 || !target.is_finite() || target <= 0.0 {
        return target;
    }
    let hysteresis = if hysteresis_m.is_finite() {
        hysteresis_m.clamp(0.0, 0.25)
    } else {
        0.0
    };
    if (target - current).abs() <= hysteresis {
        return current;
    }
    // Pulling the near plane inward is a visibility/safety response and therefore immediate.
    if target < current {
        return target;
    }
    if !(dt.is_finite() && dt > 0.0) {
        return target;
    }
    let tau = if release_time_seconds.is_finite() {
        release_time_seconds.clamp(0.001, 5.0)
    } else {
        0.08
    };
    let alpha = (1.0 - (-dt.min(0.05) / tau).exp()).clamp(0.0, 1.0);
    let next = current + (target - current) * alpha;
    if target - next <= 1.0e-4 {
        target
    } else {
        next
    }
}

impl CameraRuntimeService {
    /// Resolves a dynamic gameplay near plane after the final camera frame is known.
    ///
    /// The project owns limits/tuning; camera runtime owns only the collision scan and temporal
    /// response. The local player entity is excluded through the shared spring-arm query contract.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_gameplay_near_clip(
        world: &mut World,
        camera: EntityId,
        target: EntityId,
        camera_position: Vec3,
        camera_rotation: Quat,
        fov_y_radians: f32,
        aspect: f32,
        config: CameraRuntimeServiceConfig,
        dt: f32,
    ) -> f32 {
        let (min_near, max_near) = near_clip_limits(config);
        if !config.near_clip_enabled || !world.exists(camera) || !world.exists(target) {
            return min_near;
        }

        let target_near = {
            let collision_world = world.resource::<CameraSpringArmCollisionWorld>();
            scan_safe_near_clip(
                target,
                camera_position,
                camera_rotation,
                fov_y_radians,
                aspect,
                min_near,
                max_near,
                config.near_clip_probe_radius,
                config.near_clip_pull_in_distance,
                collision_world,
            )
        };

        let mut state = world
            .get::<GameplayNearClipState>(camera)
            .copied()
            .unwrap_or_default();
        if !state.initialized || state.target != target || state.runner != config.runner {
            state.target = target;
            state.runner = config.runner;
            state.current_near = target_near;
            state.initialized = true;
        } else {
            state.current_near = step_near_clip_response(
                state.current_near,
                target_near,
                dt,
                config.near_clip_release_time_seconds,
                config.near_clip_hysteresis_m,
            )
            .clamp(min_near, max_near);
        }
        let resolved = state.current_near;
        let _ = world.insert(camera, state);
        resolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::CameraSpringArmAabbCollider;

    #[test]
    fn clear_frustum_can_extend_near_plane_to_authored_maximum() {
        let mut ecs = World::new();
        let player = ecs.spawn();
        let camera = ecs.spawn();
        ecs.insert_resource(CameraSpringArmCollisionWorld::default());
        let config = CameraRuntimeServiceConfig::default();
        let near = CameraRuntimeService::resolve_gameplay_near_clip(
            &mut ecs,
            camera,
            player,
            Vec3::ZERO,
            Quat::IDENTITY,
            config.first_person_fov_y_radians,
            16.0 / 9.0,
            config,
            1.0 / 60.0,
        );
        assert!((near - config.near_clip_first_person_max_distance).abs() <= 1.0e-6);
    }

    #[test]
    fn near_geometry_pulls_plane_in_before_it_can_slice_the_surface() {
        let mut ecs = World::new();
        let player = ecs.spawn();
        let camera = ecs.spawn();
        let wall = ecs.spawn();
        let mut collision = CameraSpringArmCollisionWorld::default();
        collision.push_aabb(CameraSpringArmAabbCollider {
            entity: wall,
            min_ws: Vec3::new(-1.0, -1.0, -0.075),
            max_ws: Vec3::new(1.0, 1.0, -0.070),
        });
        ecs.insert_resource(collision);
        let config = CameraRuntimeServiceConfig {
            near_clip_first_person_max_distance: 0.09,
            near_clip_pull_in_distance: 0.010,
            near_clip_probe_radius: 0.0,
            ..Default::default()
        };
        let near = CameraRuntimeService::resolve_gameplay_near_clip(
            &mut ecs,
            camera,
            player,
            Vec3::ZERO,
            Quat::IDENTITY,
            config.first_person_fov_y_radians,
            16.0 / 9.0,
            config,
            1.0 / 60.0,
        );
        assert!(near >= config.first_person_near);
        assert!(near < 0.070);
    }

    #[test]
    fn missing_collision_scene_never_assumes_extending_near_plane_is_safe() {
        let mut ecs = World::new();
        let player = ecs.spawn();
        let camera = ecs.spawn();
        let config = CameraRuntimeServiceConfig::default();
        let near = CameraRuntimeService::resolve_gameplay_near_clip(
            &mut ecs,
            camera,
            player,
            Vec3::ZERO,
            Quat::IDENTITY,
            config.first_person_fov_y_radians,
            16.0 / 9.0,
            config,
            1.0 / 60.0,
        );
        assert!((near - config.first_person_near).abs() <= 1.0e-6);
    }

    #[test]
    fn near_plane_release_is_smooth_but_pull_in_is_immediate() {
        let pulled = step_near_clip_response(0.09, 0.05, 1.0 / 60.0, 0.08, 0.001);
        assert!((pulled - 0.05).abs() <= 1.0e-6);
        let released = step_near_clip_response(0.05, 0.09, 1.0 / 60.0, 0.08, 0.001);
        assert!(released > 0.05 && released < 0.09);
    }
}
