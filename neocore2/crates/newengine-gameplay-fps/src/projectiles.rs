use newengine_ecs::{EntityId, World};
use newengine_game_data::default_game_data;
use newengine_gameplay_fps_api::{FpsActionFrame, FpsGameplayPolicySnapshot};
use newengine_math::{Quat, Vec3};
use newengine_primitives::{builtins as prim_builtins, Primitive};
use newengine_scene::{components::Name, SceneState};
use newengine_sim::{AngularVelocity, CameraRigComp, Velocity};
use newengine_transform::Transform;

use crate::game_data::active_game_data;

use newengine_engine_runtime::gameplay::{
    CollisionShapeDesc, DisplayVisibility, GameplayActor, PhysicsBodyDesc, PhysicsSurface,
    PlayerCommandFrame, PlayerController, PlayerStanceState,
};

/// Runtime tuning for the simple physics sphere launcher used by the GameReady FPS profile.
///
/// It is a normal ECS resource rather than a renderer/debug shortcut, so authored profiles or
/// future gameplay code can replace these values without changing the physics backend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectileSphereTuning {
    pub radius: f32,
    pub speed: f32,
    pub lifetime_seconds: f32,
    pub spawn_clearance: f32,
    pub restitution: f32,
    pub friction: f32,
    pub density: f32,
}

impl Default for ProjectileSphereTuning {
    fn default() -> Self {
        let data = default_game_data().gameplay.projectile;
        Self {
            radius: data.radius,
            speed: data.speed,
            lifetime_seconds: data.lifetime_seconds,
            spawn_clearance: data.spawn_clearance,
            restitution: data.restitution,
            friction: data.friction,
            density: data.density,
        }
    }
}

impl ProjectileSphereTuning {
    #[inline]
    pub fn from_data(data: &newengine_game_data::ProjectileData) -> Self {
        Self {
            radius: data.radius,
            speed: data.speed,
            lifetime_seconds: data.lifetime_seconds,
            spawn_clearance: data.spawn_clearance,
            restitution: data.restitution,
            friction: data.friction,
            density: data.density,
        }
    }

    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            radius: finite_or(self.radius, default_game_data().gameplay.projectile.radius)
                .clamp(0.03, 2.0),
            speed: finite_or(self.speed, default_game_data().gameplay.projectile.speed)
                .clamp(0.1, 250.0),
            lifetime_seconds: finite_or(
                self.lifetime_seconds,
                default_game_data().gameplay.projectile.lifetime_seconds,
            )
            .clamp(0.25, 120.0),
            spawn_clearance: finite_or(
                self.spawn_clearance,
                default_game_data().gameplay.projectile.spawn_clearance,
            )
            .clamp(0.05, 8.0),
            restitution: finite_or(
                self.restitution,
                default_game_data().gameplay.projectile.restitution,
            )
            .clamp(0.0, 1.0),
            friction: finite_or(
                self.friction,
                default_game_data().gameplay.projectile.friction,
            )
            .clamp(0.0, 2.0),
            density: finite_or(
                self.density,
                default_game_data().gameplay.projectile.density,
            )
            .clamp(0.01, 1000.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectileSphereRuntime {
    pub owner: EntityId,
    pub source_frame: u64,
    pub remaining_seconds: f32,
}

/// Consumes the semantic `player.projectile.launch` pulse and launches exactly one sphere per
/// input press. The projectile is spawned from the active camera ray, so its trajectory matches
/// the center of the rendered viewport rather than an approximate character-forward vector.
pub fn step_projectile_sphere_launcher(world: &mut World, dt: f32) {
    let dt = finite_or(dt, 0.0).clamp(0.0, 0.1);
    let allow_projectile_launch = world
        .resource::<FpsGameplayPolicySnapshot>()
        .map(|policy| policy.player.allow_projectile_launch)
        .unwrap_or(true);
    let tuning = world
        .resource::<ProjectileSphereTuning>()
        .copied()
        .unwrap_or_else(|| {
            ProjectileSphereTuning::from_data(&active_game_data(world).gameplay.projectile)
        })
        .sanitized();

    let launch_requests = world
        .query2_ids::<PlayerController, PlayerCommandFrame>()
        .filter_map(|player| {
            let commands = world.get::<PlayerCommandFrame>(player)?;
            (allow_projectile_launch
                && FpsActionFrame::from_commands(&commands.actions).launch_projectile_pressed)
                .then_some((player, commands.source_frame))
        })
        .collect::<Vec<_>>();

    for (player, source_frame) in launch_requests {
        if let Some((origin, direction)) = camera_center_ray(world, player) {
            let _ = spawn_projectile_sphere(world, player, source_frame, origin, direction, tuning);
        }
    }

    expire_projectile_spheres(world, dt);
}

pub fn spawn_projectile_sphere(
    world: &mut World,
    owner: EntityId,
    source_frame: u64,
    camera_origin: Vec3,
    camera_forward: Vec3,
    tuning: ProjectileSphereTuning,
) -> Option<EntityId> {
    let tuning = tuning.sanitized();
    let direction = camera_forward.normalize_or_zero();
    if direction.length_squared() <= 1.0e-8 || !camera_origin.is_finite() {
        return None;
    }

    // Keep the initial sphere outside the player/camera capsule so the first physics step does not
    // resolve an artificial self-overlap and kick the projectile sideways.
    let center = camera_origin + direction * (tuning.spawn_clearance + tuning.radius);
    let entity = world.spawn();
    let _ = world.insert(
        entity,
        Name(format!(
            "Projectile/Sphere/{:016x}/{source_frame}",
            owner.stable_u64()
        )),
    );
    let _ = world.insert(
        entity,
        Transform {
            position: center,
            rotation: Quat::IDENTITY,
            // Built-in UV sphere has radius 0.5, therefore diameter is the transform scale.
            scale: Vec3::splat(tuning.radius * 2.0),
        },
    );
    let _ = world.insert(
        entity,
        Primitive {
            id: prim_builtins::ID_SPHERE_UV,
            color: active_game_data(world).gameplay.projectile.color,
        },
    );
    let _ = world.insert(entity, DisplayVisibility::default());
    // Projectile spheres are fast transient geometry. Let them receive world shadows,
    // but do not let every bullet invalidate the whole raster shadow atlas.
    let mut render_options = newengine_model_domain_api::MeshRenderOptions::world_opaque();
    render_options.shadow_policy = newengine_model_domain_api::MeshShadowPolicy::ReceiveOnly;
    let _ = world.insert(entity, render_options);
    let _ = world.insert(entity, GameplayActor);
    let _ = world.insert(entity, PhysicsSurface::default());

    let mut body = PhysicsBodyDesc::dynamic_solid(CollisionShapeDesc::Sphere {
        radius: tuning.radius,
    });
    body.material.friction = tuning.friction;
    body.material.restitution = tuning.restitution;
    body.material.density = tuning.density;
    let _ = world.insert(entity, body);
    let _ = world.insert(entity, body.to_bounds());
    let _ = world.insert(entity, Velocity(direction * tuning.speed));
    let _ = world.insert(
        entity,
        AngularVelocity({
            let v = active_game_data(world).gameplay.projectile.angular_velocity;
            Vec3::new(v[0], v[1], v[2])
        }),
    );
    let _ = world.insert(
        entity,
        ProjectileSphereRuntime {
            owner,
            source_frame,
            remaining_seconds: tuning.lifetime_seconds,
        },
    );

    newengine_ulog_api::ulog::info!(
        "gameplay projectile sphere launched entity={:?} owner={:?} source_frame={} origin={:?} direction={:?} radius={:.3} speed={:.2}",
        entity,
        owner,
        source_frame,
        center,
        direction,
        tuning.radius,
        tuning.speed,
    );
    Some(entity)
}

fn camera_center_ray(world: &World, player: EntityId) -> Option<(Vec3, Vec3)> {
    if let Some(camera) = world
        .resource::<SceneState>()
        .and_then(|state| state.active_camera)
    {
        if let Some(rig) = world.get::<CameraRigComp>(camera) {
            let forward = rig.0.forward().normalize_or_zero();
            if rig.0.position.is_finite() && forward.length_squared() > 1.0e-8 {
                return Some((rig.0.position, forward));
            }
        }
        if let Some(transform) = world.get::<Transform>(camera).copied() {
            let forward = (transform.rotation * -Vec3::Z).normalize_or_zero();
            if transform.position.is_finite() && forward.length_squared() > 1.0e-8 {
                return Some((transform.position, forward));
            }
        }
    }

    // Safe first-person fallback for tests/headless simulation without an active camera entity.
    let transform = world.get::<Transform>(player).copied()?;
    let eye_height = world
        .get::<PlayerStanceState>(player)
        .map(|stance| stance.current_eye_height)
        .unwrap_or(active_game_data(world).player.tuning.camera_eye_height);
    let forward = (transform.rotation * -Vec3::Z).normalize_or_zero();
    (forward.length_squared() > 1.0e-8)
        .then_some((transform.position + Vec3::Y * eye_height, forward))
}

fn expire_projectile_spheres(world: &mut World, dt: f32) {
    if dt <= 0.0 {
        return;
    }
    let projectiles = world
        .query::<ProjectileSphereRuntime>()
        .map(|(entity, runtime)| (entity, *runtime))
        .collect::<Vec<_>>();
    for (entity, mut runtime) in projectiles {
        runtime.remaining_seconds = (runtime.remaining_seconds - dt).max(0.0);
        if runtime.remaining_seconds <= 0.0 {
            let _ = world.despawn(entity);
        } else {
            let _ = world.insert(entity, runtime);
        }
    }
}

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_gameplay_fps_api::action as fps_action;
    use newengine_input_actions_api::ActionCommandFrame;
    use newengine_scene::SceneState;
    use newengine_sim::CameraRigComp;

    fn setup_player_and_camera(world: &mut World) -> (EntityId, EntityId) {
        let player = world.spawn();
        let _ = world.insert(player, PlayerController::default());
        let _ = world.insert(player, PlayerCommandFrame::default());
        let _ = world.insert(player, Transform::default());

        let camera = world.spawn();
        let rotation = Quat::from_rotation_y(-0.5) * Quat::from_rotation_x(0.25);
        let _ = world.insert(
            camera,
            CameraRigComp(newengine_camera::CameraRig {
                position: Vec3::new(2.0, 3.0, 4.0),
                rotation,
            }),
        );
        world.insert_resource(SceneState::new(None, Some(camera)));
        (player, camera)
    }

    #[test]
    fn launch_pulse_spawns_one_dynamic_sphere_along_camera_forward() {
        let mut world = World::new();
        let (player, camera) = setup_player_and_camera(&mut world);
        let actions = ActionCommandFrame {
            pressed: vec![fps_action::PLAYER_LAUNCH_PROJECTILE.into()],
            ..ActionCommandFrame::default()
        };
        let _ = world.insert(player, PlayerCommandFrame::new(77, actions));
        let rig = world.get::<CameraRigComp>(camera).copied().unwrap();
        let expected_forward = rig.0.forward().normalize_or_zero();

        step_projectile_sphere_launcher(&mut world, 1.0 / 60.0);

        let projectiles = world
            .query::<ProjectileSphereRuntime>()
            .map(|(entity, runtime)| (entity, *runtime))
            .collect::<Vec<_>>();
        assert_eq!(projectiles.len(), 1);
        let (entity, runtime) = projectiles[0];
        assert_eq!(runtime.owner, player);
        assert_eq!(runtime.source_frame, 77);
        let velocity = world.get::<Velocity>(entity).copied().unwrap().0;
        assert!(velocity.normalize_or_zero().dot(expected_forward) > 0.9999);
        assert!(matches!(
            world.get::<PhysicsBodyDesc>(entity).map(|body| body.shape),
            Some(CollisionShapeDesc::Sphere { .. })
        ));
    }

    #[test]
    fn projectile_lifetime_despawns_entity() {
        let mut world = World::new();
        let owner = world.spawn();
        let entity = spawn_projectile_sphere(
            &mut world,
            owner,
            1,
            Vec3::ZERO,
            -Vec3::Z,
            ProjectileSphereTuning {
                lifetime_seconds: 0.25,
                ..ProjectileSphereTuning::default()
            },
        )
        .unwrap();
        for _ in 0..3 {
            expire_projectile_spheres(&mut world, 0.1);
        }
        assert!(!world.exists(entity));
    }
}
