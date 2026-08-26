/// Consumes the semantic `player.projectile.launch` pulse and launches exactly one sphere per
/// input press. The projectile is spawned from the active camera ray, so its trajectory matches
/// the center of the rendered viewport rather than an approximate character-forward vector.
pub fn step_projectile_sphere_launcher(world: &mut World, dt: f32) {
    let dt = finite_or(dt, 0.0).clamp(0.0, 0.1);
    let allow_projectile_launch = world
        .resource::<FpsGameplayPolicySnapshot>()
        .map(|policy| policy.player.allow_projectile_launch)
        .unwrap_or(true);
    let Some(game_data) = active_game_data(world) else {
        return;
    };
    let tuning = world
        .resource::<ProjectileSphereTuning>()
        .copied()
        .unwrap_or_else(|| ProjectileSphereTuning::from_data(&game_data.gameplay.projectile))
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
    let game_data = active_game_data(world)?;
    let projectile_color = game_data.gameplay.projectile.color;
    let projectile_angular_velocity = game_data.gameplay.projectile.angular_velocity;
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
            color: projectile_color,
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
            let v = projectile_angular_velocity;
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
    let eye_height = match world.get::<PlayerStanceState>(player) {
        Some(stance) => stance.current_eye_height,
        None => active_game_data(world)?.player.tuning.camera_eye_height,
    };
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
