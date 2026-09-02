#[inline]
fn shortest_rotation_vector(mut rotation: Quat) -> Vec3 {
    rotation = rotation.normalize_or_identity();
    if rotation.w < 0.0 {
        rotation = Quat::from_xyzw(-rotation.x, -rotation.y, -rotation.z, -rotation.w);
    }
    let w = rotation.w.clamp(-1.0, 1.0);
    let sin_half = (1.0 - w * w).max(0.0).sqrt();
    if sin_half <= 1.0e-6 {
        return Vec3::new(rotation.x, rotation.y, rotation.z) * 2.0;
    }
    let angle = 2.0 * sin_half.atan2(w);
    Vec3::new(rotation.x, rotation.y, rotation.z) * (angle / sin_half)
}

#[inline]
fn clamp_vec3_length(value: Vec3, max_length: f32) -> Vec3 {
    let length = value.length();
    if !length.is_finite() || !value.is_finite() {
        return Vec3::ZERO;
    }
    if length > max_length.max(0.0) && length > 1.0e-6 {
        value * (max_length / length)
    } else {
        value
    }
}

/// Critically-damped angular spring around the firing-hand pivot. This is intentionally not a
/// rigid body: authored grip animation remains authoritative and only a few degrees of secondary
/// long-gun inertia are permitted. Fast target rotation injects angular lag; player acceleration
/// injects a smaller mass-response impulse. ADS, recoil and obstruction progressively tighten it.
fn step_long_gun_secondary_dynamics(
    mut state: WeaponSecondaryDynamicsState,
    presentation: &newengine_engine_runtime::gameplay::WeaponPresentationDefinition,
    target_rotation: Quat,
    owner_position_world: Vec3,
    owner_rotation_world: Quat,
    dt: f32,
    aim_alpha: f32,
    recoil_alpha: f32,
    obstruction_alpha: f32,
) -> WeaponSecondaryDynamicsState {
    let target_rotation = target_rotation.normalize_or_identity();
    let owner_rotation_world = owner_rotation_world.normalize_or_identity();
    let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
    if !target_rotation.is_finite()
        || !owner_position_world.is_finite()
        || !owner_rotation_world.is_finite()
        || dt <= 0.0
        || dt > 0.050
        || !state.initialized
    {
        return WeaponSecondaryDynamicsState {
            initialized: true,
            rotation_offset_local: Vec3::ZERO,
            angular_velocity_local: Vec3::ZERO,
            previous_target_rotation: target_rotation,
            previous_owner_position_world: owner_position_world,
            previous_owner_velocity_world: Vec3::ZERO,
        };
    }

    let aim_alpha = aim_alpha.clamp(0.0, 1.0);
    let recoil_alpha = recoil_alpha.clamp(0.0, 1.0);
    let obstruction_alpha = obstruction_alpha.clamp(0.0, 1.0);
    let constraint_scale = (1.0 - obstruction_alpha * 0.95) * (1.0 - recoil_alpha * 0.70);
    let angular_inertia_gain =
        presentation.secondary_angular_inertia_gain * (1.0 - aim_alpha * 0.48) * constraint_scale;

    let target_delta_local =
        shortest_rotation_vector(state.previous_target_rotation.inverse() * target_rotation);
    state.rotation_offset_local -= target_delta_local * angular_inertia_gain;

    let owner_velocity_world = (owner_position_world - state.previous_owner_position_world) / dt;
    let mut owner_acceleration_world =
        (owner_velocity_world - state.previous_owner_velocity_world) / dt;
    owner_acceleration_world = clamp_vec3_length(owner_acceleration_world, 35.0);
    let owner_acceleration_local = owner_rotation_world.inverse() * owner_acceleration_world;
    let movement_gain =
        presentation.secondary_movement_inertia_gain * (1.0 - aim_alpha * 0.55) * constraint_scale;
    let movement_impulse = Vec3::new(
        owner_acceleration_local.z * -0.00125 + owner_acceleration_local.y * 0.00045,
        owner_acceleration_local.x * -0.00095,
        owner_acceleration_local.x * 0.00060,
    ) * movement_gain;
    state.rotation_offset_local += movement_impulse;

    // Exact critical-damping solution for a constant zero target over this frame.
    let natural_hz = presentation.secondary_natural_hz_hip
        + (presentation.secondary_natural_hz_ads - presentation.secondary_natural_hz_hip)
            * aim_alpha
        + obstruction_alpha * presentation.secondary_obstruction_hz_boost;
    let omega = 2.0 * core::f32::consts::PI * natural_hz;
    let exp = (-omega * dt).exp();
    let x = state.rotation_offset_local;
    let v = state.angular_velocity_local;
    let c = v + x * omega;
    state.rotation_offset_local = (x + c * dt) * exp;
    state.angular_velocity_local = (v - c * (omega * dt)) * exp;

    let max_angle = (presentation.secondary_hip_max_angle_radians
        + (presentation.secondary_ads_max_angle_radians
            - presentation.secondary_hip_max_angle_radians)
            * aim_alpha)
        * (1.0 - obstruction_alpha * 0.88)
        * (1.0 - recoil_alpha * 0.45);
    state.rotation_offset_local =
        clamp_vec3_length(state.rotation_offset_local, max_angle.max(0.004));
    state.angular_velocity_local = clamp_vec3_length(state.angular_velocity_local, 8.0);
    state.previous_target_rotation = target_rotation;
    state.previous_owner_position_world = owner_position_world;
    state.previous_owner_velocity_world = owner_velocity_world;
    state
}

#[inline]
fn secondary_weapon_dynamics_enabled(
    authored_weapon_presentation: bool,
    first_person_active: bool,
    animation_root_authoritative: bool,
) -> bool {
    authored_weapon_presentation && !first_person_active && !animation_root_authoritative
}

pub(crate) fn equipped_weapon_secondary_rotation_offset_local(
    world: &newengine_ecs::World,
    owner: EntityId,
) -> Vec3 {
    world
        .query::<EquippedWeaponVisualRoot>()
        .find_map(|(root, visual)| {
            (visual.owner == owner).then(|| {
                world
                    .get::<WeaponSecondaryDynamicsState>(root)
                    .copied()
                    .unwrap_or_default()
                    .rotation_offset_local
            })
        })
        .filter(|value| value.is_finite())
        .unwrap_or(Vec3::ZERO)
}

#[inline]
fn equipped_weapon_aim_held(
    world: &newengine_ecs::World,
    owner: EntityId,
    instance_id: newengine_engine_runtime::gameplay::ItemInstanceId,
) -> bool {
    let Some(binding) =
        newengine_engine_runtime::gameplay::active_equipped_weapon_binding(world, owner)
    else {
        return false;
    };
    if binding.instance_id != instance_id || !binding.capabilities().aim {
        return false;
    }
    // Raw RMB is input intent only. Admit it through the exact active weapon instance so render
    // presentation remains immediate without ever aiming a stale visual or a weaponless player.
    world
        .get::<PlayerCommandFrame>(owner)
        .is_some_and(|commands| {
            commands
                .actions
                .is_held(newengine_gameplay_fps_api::action::PLAYER_AIM)
        })
        || newengine_engine_runtime::gameplay::active_equipped_weapon_aiming(world, owner)
}

#[inline]
fn smooth_first_person_aim_alpha(current: f32, target: f32, dt: f32, response_hz: f32) -> f32 {
    let current = if current.is_finite() {
        current.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let target = target.clamp(0.0, 1.0);
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    if dt <= 0.0 {
        return target;
    }
    let response_hz = if response_hz.is_finite() {
        response_hz.max(0.1)
    } else {
        18.0
    };
    let alpha = 1.0 - (-response_hz * dt).exp();
    (current + (target - current) * alpha).clamp(0.0, 1.0)
}

#[inline]
fn equipment_animation_event(
    weapon_type: newengine_engine_runtime::gameplay::WeaponType,
    reload_active: bool,
    aim_alpha: f32,
) -> &'static str {
    use newengine_engine_runtime::gameplay::WeaponType;
    if weapon_type == WeaponType::Melee {
        return "character.equipment.ready";
    }
    if weapon_type != WeaponType::Firearm {
        return "character.equipment.inactive";
    }
    if reload_active {
        "character.equipment.reload"
    } else if aim_alpha > 0.001 {
        "character.equipment.aim"
    } else {
        "character.equipment.ready"
    }
}
