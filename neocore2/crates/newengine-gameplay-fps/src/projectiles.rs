use newengine_ecs::{EntityId, World};
use newengine_game_data::default_game_data;
use newengine_gameplay_fps_api::{FpsActionFrame, FpsGameplayPolicySnapshot};
use newengine_lighting::PointLight;
use newengine_math::{EulerRot, Quat, Vec3};
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

const WEAPON_TRACER_SPEED_MPS: f32 = 320.0;
const WEAPON_TRACER_HALF_LENGTH_M: f32 = 0.12;
const MUZZLE_FLASH_LIFETIME_SECONDS: f32 = 0.042;
const MUZZLE_CORE_LIFETIME_SECONDS: f32 = 0.030;
const SHELL_CASING_LIFETIME_SECONDS: f32 = 1.35;
const SHELL_CASING_GRAVITY_MPS2: f32 = 9.81;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WeaponShotFxKind {
    MuzzleFlash,
    MuzzleCore,
    Tracer,
    ShellCasing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WeaponShotFxRuntime {
    owner: EntityId,
    shot_sequence: u64,
    kind: WeaponShotFxKind,
    origin: Vec3,
    velocity: Vec3,
    spin_radians_per_second: Vec3,
    traveled: f32,
    max_distance: f32,
    remaining_seconds: f32,
}

#[inline]
fn weapon_fx_render_options() -> newengine_model_domain_api::MeshRenderOptions {
    let mut render = newengine_model_domain_api::MeshRenderOptions::world_opaque();
    render.shadow_policy = newengine_model_domain_api::MeshShadowPolicy::None;
    render
}

/// Spawns presentation-only firing effects from the already-resolved physical muzzle. Damage and
/// collision stay authoritative in the hitscan query path. The composition mirrors a shouldered
/// semi-auto rifle: compact directional muzzle flame, hot core/light pulse, subtle tracer and a
/// receiver-side brass casing. No presentation entity participates in ballistic collision.
pub(crate) fn spawn_weapon_shot_fx(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    origin: Vec3,
    direction: Vec3,
    range: f32,
) {
    let direction = direction.normalize_or_zero();
    if !origin.is_finite() || direction.length_squared() <= 1.0e-8 {
        return;
    }
    let tracer_rotation = Quat::from_rotation_arc(Vec3::Z, direction).normalize_or_identity();
    let cone_rotation = Quat::from_rotation_arc(Vec3::Y, direction).normalize_or_identity();

    // Directional flame: cone base sits close to the crown, apex extends downrange.
    let flash = world.spawn();
    let _ = world.insert(
        flash,
        Name(format!(
            "WeaponFx/MuzzleFlash/{:016x}/{shot_sequence}",
            owner.stable_u64()
        )),
    );
    let _ = world.insert(
        flash,
        Transform {
            position: origin + direction * 0.085,
            rotation: cone_rotation,
            scale: Vec3::new(0.075, 0.17, 0.075),
        },
    );
    let _ = world.insert(
        flash,
        Primitive {
            id: prim_builtins::ID_CONE,
            color: [1.0, 0.54, 0.10, 1.0],
        },
    );
    let _ = world.insert(flash, DisplayVisibility::default());
    let _ = world.insert(flash, GameplayActor);
    let _ = world.insert(flash, weapon_fx_render_options());
    let _ = world.insert(
        flash,
        WeaponShotFxRuntime {
            owner,
            shot_sequence,
            kind: WeaponShotFxKind::MuzzleFlash,
            origin,
            velocity: Vec3::ZERO,
            spin_radians_per_second: Vec3::ZERO,
            traveled: 0.0,
            max_distance: 0.0,
            remaining_seconds: MUZZLE_FLASH_LIFETIME_SECONDS,
        },
    );

    // Compact white-hot core also owns the light pulse. Keeping this much smaller than the old
    // sphere avoids the debug-ball silhouette while preserving a strong flash in dark scenes.
    let core = world.spawn();
    let _ = world.insert(
        core,
        Name(format!(
            "WeaponFx/MuzzleCore/{:016x}/{shot_sequence}",
            owner.stable_u64()
        )),
    );
    let _ = world.insert(
        core,
        Transform {
            position: origin + direction * 0.018,
            rotation: tracer_rotation,
            scale: Vec3::new(0.040, 0.040, 0.065),
        },
    );
    let _ = world.insert(
        core,
        Primitive {
            id: prim_builtins::ID_SPHERE_UV,
            color: [1.0, 0.86, 0.48, 1.0],
        },
    );
    let _ = world.insert(core, DisplayVisibility::default());
    let _ = world.insert(core, GameplayActor);
    let _ = world.insert(core, weapon_fx_render_options());
    let _ = world.insert(
        core,
        PointLight {
            color: [1.0, 0.58, 0.18],
            intensity: 34.0,
            range: 3.8,
        },
    );
    let _ = world.insert(
        core,
        WeaponShotFxRuntime {
            owner,
            shot_sequence,
            kind: WeaponShotFxKind::MuzzleCore,
            origin,
            velocity: Vec3::ZERO,
            spin_radians_per_second: Vec3::ZERO,
            traveled: 0.0,
            max_distance: 0.0,
            remaining_seconds: MUZZLE_CORE_LIFETIME_SECONDS,
        },
    );

    let max_distance = if range.is_finite() {
        range.clamp(0.1, 100_000.0)
    } else {
        120.0
    };
    let tracer = world.spawn();
    let _ = world.insert(
        tracer,
        Name(format!(
            "WeaponFx/Tracer/{:016x}/{shot_sequence}",
            owner.stable_u64()
        )),
    );
    let _ = world.insert(
        tracer,
        Transform {
            position: origin + direction * WEAPON_TRACER_HALF_LENGTH_M,
            rotation: tracer_rotation,
            scale: Vec3::new(0.004, 0.004, WEAPON_TRACER_HALF_LENGTH_M * 2.0),
        },
    );
    let _ = world.insert(
        tracer,
        Primitive {
            id: prim_builtins::ID_CUBE,
            color: [1.0, 0.72, 0.24, 1.0],
        },
    );
    let _ = world.insert(tracer, DisplayVisibility::default());
    let _ = world.insert(tracer, GameplayActor);
    let _ = world.insert(tracer, weapon_fx_render_options());
    let _ = world.insert(
        tracer,
        WeaponShotFxRuntime {
            owner,
            shot_sequence,
            kind: WeaponShotFxKind::Tracer,
            origin,
            velocity: direction * WEAPON_TRACER_SPEED_MPS,
            spin_radians_per_second: Vec3::ZERO,
            traveled: WEAPON_TRACER_HALF_LENGTH_M,
            max_distance,
            remaining_seconds: (max_distance / WEAPON_TRACER_SPEED_MPS + 0.06).clamp(0.06, 0.8),
        },
    );

    // Ejection basis is reconstructed from the muzzle ray. The casing originates near the
    // receiver (~43 cm behind this rifle's muzzle) and is thrown outward/up, never from the crown.
    let mut right = direction.cross(Vec3::Y).normalize_or_zero();
    if right.length_squared() <= 1.0e-8 {
        right = Vec3::X;
    }
    let up = right.cross(direction).normalize_or_zero();
    let jitter = (((shot_sequence.wrapping_mul(1_103_515_245).wrapping_add(12_345) >> 8)
        & 0xffff) as f32
        / 65_535.0)
        - 0.5;
    let casing_origin = origin - direction * 0.43 + right * 0.050 + up * 0.025;
    let casing_velocity = right * (1.85 + jitter * 0.35)
        + up * (1.25 + jitter.abs() * 0.25)
        - direction * 0.22;
    let casing_axis = (right * 0.85 + up * 0.15).normalize_or_zero();
    let casing = world.spawn();
    let _ = world.insert(
        casing,
        Name(format!(
            "WeaponFx/ShellCasing/{:016x}/{shot_sequence}",
            owner.stable_u64()
        )),
    );
    let _ = world.insert(
        casing,
        Transform {
            position: casing_origin,
            rotation: Quat::from_rotation_arc(Vec3::Y, casing_axis).normalize_or_identity(),
            scale: Vec3::new(0.010, 0.026, 0.010),
        },
    );
    let _ = world.insert(
        casing,
        Primitive {
            id: prim_builtins::ID_CYLINDER,
            color: [0.72, 0.48, 0.18, 1.0],
        },
    );
    let _ = world.insert(casing, DisplayVisibility::default());
    let _ = world.insert(casing, GameplayActor);
    let _ = world.insert(casing, weapon_fx_render_options());
    let _ = world.insert(
        casing,
        WeaponShotFxRuntime {
            owner,
            shot_sequence,
            kind: WeaponShotFxKind::ShellCasing,
            origin: casing_origin,
            velocity: casing_velocity,
            spin_radians_per_second: Vec3::new(18.0 + jitter * 4.0, 11.0, 23.0 - jitter * 5.0),
            traveled: 0.0,
            max_distance: 0.0,
            remaining_seconds: SHELL_CASING_LIFETIME_SECONDS,
        },
    );
}

/// Narrows an already spawned tracer to the authoritative hitscan impact. The tracer continues
/// travelling visually, but it can no longer pass through the wall/target that the shot hit.
pub(crate) fn clamp_weapon_shot_fx_to_hit(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    point: Vec3,
) {
    if !point.is_finite() {
        return;
    }
    let effects = world
        .query::<WeaponShotFxRuntime>()
        .filter_map(|(entity, runtime)| {
            (runtime.owner == owner
                && runtime.shot_sequence == shot_sequence
                && runtime.kind == WeaponShotFxKind::Tracer)
                .then_some((entity, *runtime))
        })
        .collect::<Vec<_>>();
    for (entity, mut runtime) in effects {
        let hit_distance = (point - runtime.origin).length();
        if hit_distance.is_finite() {
            runtime.max_distance = runtime.max_distance.min(hit_distance.max(0.0));
            let _ = world.insert(entity, runtime);
        }
    }
}

pub(crate) fn step_weapon_shot_fx(world: &mut World, dt: f32) {
    let dt = finite_or(dt, 0.0).clamp(0.0, 0.1);
    if dt <= 0.0 {
        return;
    }
    let effects = world
        .query::<WeaponShotFxRuntime>()
        .map(|(entity, runtime)| (entity, *runtime))
        .collect::<Vec<_>>();
    for (entity, mut runtime) in effects {
        runtime.remaining_seconds = (runtime.remaining_seconds - dt).max(0.0);
        let mut expire = runtime.remaining_seconds <= 0.0;
        match runtime.kind {
            WeaponShotFxKind::Tracer if !expire => {
                let speed = runtime.velocity.length();
                let direction = runtime.velocity.normalize_or_zero();
                if speed <= 1.0e-6 || direction.length_squared() <= 1.0e-8 {
                    expire = true;
                } else {
                    let remaining_distance = (runtime.max_distance - runtime.traveled).max(0.0);
                    let advance = (speed * dt).min(remaining_distance);
                    if let Some(transform) = world.get_mut::<Transform>(entity) {
                        transform.position += direction * advance;
                    }
                    runtime.traveled += advance;
                    if runtime.traveled + 1.0e-4 >= runtime.max_distance {
                        expire = true;
                    }
                }
            }
            WeaponShotFxKind::ShellCasing if !expire => {
                runtime.velocity.y -= SHELL_CASING_GRAVITY_MPS2 * dt;
                if let Some(transform) = world.get_mut::<Transform>(entity) {
                    transform.position += runtime.velocity * dt;
                    let spin = runtime.spin_radians_per_second * dt;
                    transform.rotation = (
                        Quat::from_euler(EulerRot::XYZ, spin.x, spin.y, spin.z)
                            * transform.rotation
                    )
                        .normalize_or_identity();
                }
            }
            _ => {}
        }
        if expire {
            let _ = world.despawn(entity);
        } else {
            let _ = world.insert(entity, runtime);
        }
    }
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
    fn weapon_shot_fx_starts_at_muzzle_and_stops_at_hitscan_impact() {
        let mut world = World::new();
        let owner = world.spawn();
        let origin = Vec3::new(1.0, 1.5, 2.0);
        let direction = -Vec3::Z;
        spawn_weapon_shot_fx(&mut world, owner, 17, origin, direction, 120.0);

        let effects = world
            .query::<WeaponShotFxRuntime>()
            .map(|(entity, runtime)| (entity, *runtime))
            .collect::<Vec<_>>();
        assert_eq!(effects.len(), 4);
        let (tracer, tracer_runtime) = effects
            .iter()
            .copied()
            .find(|(_, runtime)| runtime.kind == WeaponShotFxKind::Tracer)
            .expect("tracer");
        let tracer_transform = world
            .get::<Transform>(tracer)
            .copied()
            .expect("tracer transform");
        assert!((tracer_runtime.origin - origin).length() < 1.0e-6);
        assert!(tracer_runtime.velocity.normalize_or_zero().dot(direction) > 0.999_999);
        assert!(tracer_transform.position.z < origin.z);
        let (casing, casing_runtime) = effects
            .iter()
            .copied()
            .find(|(_, runtime)| runtime.kind == WeaponShotFxKind::ShellCasing)
            .expect("shell casing");
        let casing_before = world.get::<Transform>(casing).copied().expect("casing transform");
        assert!(casing_runtime.velocity.length() > 1.0);
        step_weapon_shot_fx(&mut world, 0.0005);
        let casing_after = world.get::<Transform>(casing).copied().expect("moving casing");
        assert!((casing_after.position - casing_before.position).length() > 0.001);

        let hit = origin + direction * 1.0;
        clamp_weapon_shot_fx_to_hit(&mut world, owner, 17, hit);
        let clamped = world
            .get::<WeaponShotFxRuntime>(tracer)
            .copied()
            .expect("clamped tracer");
        assert!((clamped.max_distance - 1.0).abs() < 1.0e-6);

        step_weapon_shot_fx(&mut world, 0.002);
        let after = world
            .get::<Transform>(tracer)
            .copied()
            .expect("travelling tracer");
        assert!((after.position - origin).length() <= 1.0 + WEAPON_TRACER_HALF_LENGTH_M + 1.0e-4);
        step_weapon_shot_fx(&mut world, 0.01);
        assert!(
            !world.exists(tracer),
            "tracer must terminate at authoritative hit range"
        );
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
