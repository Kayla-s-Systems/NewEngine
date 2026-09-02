#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponAnimationDefinition {
    pub skeleton: String,
    pub animation_dictionary: String,
    pub idle: String,
    pub fire: String,
    pub reload: String,
    pub spawn_pose: String,
}

impl AuthoredWeaponAnimationDefinition {
    pub(super) fn compile(&self) -> WeaponAnimationDefinition {
        fn clip(value: &str) -> Option<String> {
            let value = value.trim().replace('\\', "/");
            (!value.is_empty()).then_some(value)
        }
        WeaponAnimationDefinition {
            skeleton: clip(&self.skeleton),
            animation_dictionary: clip(&self.animation_dictionary),
            idle: clip(&self.idle),
            fire: clip(&self.fire),
            reload: clip(&self.reload),
            spawn_pose: clip(&self.spawn_pose),
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponPresentationDefinition {
    pub enabled: bool,
    pub handle_from_root: [f32; 3],
    pub handle_rotation_from_root: [f32; 4],
    pub muzzle_from_root: [f32; 3],
    pub left_grip_from_handle: [f32; 3],
    pub stock_contact_from_handle: [f32; 3],
    pub ready_shoulder_pocket_offset: [f32; 3],
    pub ads_shoulder_pocket_offset: [f32; 3],
    pub fire_kick_duration_seconds: f32,
    pub fire_kick_pitch_radians: f32,
    /// Third-person ReadyHold body -> weapon native-rig rotation; runtime basis completion is
    /// carried separately by `native_rig_to_runtime_basis`.
    pub ready_body_to_root_rotation: [f32; 4],
    pub ready_right_elbow_pole_offset: [f32; 3],
    pub ready_left_elbow_pole_offset: [f32; 3],
    pub ready_left_palm_to_left_grip: [f32; 3],
    pub ready_right_palm_to_weapon: [f32; 4],
    pub ready_left_palm_to_weapon: [f32; 4],
    pub right_palm_to_handle: [f32; 3],
    pub right_palm_to_native_rig: [f32; 4],
    /// Authored native-rig -> runtime basis correction for weapon-space offsets and orientation.
    pub native_rig_to_runtime_basis: [f32; 4],
    pub authored_socket_to_weapon_handle_basis: [f32; 4],
    pub first_person_hip_handle_offset: [f32; 3],
    pub first_person_full_body_hip_handle_offset: Option<[f32; 3]>,
    pub ads_rear_sight_from_handle: [f32; 3],
    pub ads_front_sight_from_handle: [f32; 3],
    pub ads_camera_to_rear_sight: [f32; 3],
    pub ads_camera_translation_weight: [f32; 3],
    pub first_person_hip_convergence_m: f32,
    pub aim_response_hz: f32,
    pub secondary_hip_max_angle_radians: f32,
    pub secondary_ads_max_angle_radians: f32,
    pub secondary_angular_inertia_gain: f32,
    pub secondary_movement_inertia_gain: f32,
    pub secondary_natural_hz_hip: f32,
    pub secondary_natural_hz_ads: f32,
    pub secondary_obstruction_hz_boost: f32,
}

impl Default for AuthoredWeaponPresentationDefinition {
    fn default() -> Self {
        let runtime = WeaponPresentationDefinition::default();
        Self {
            enabled: runtime.enabled,
            handle_from_root: runtime.handle_from_root,
            handle_rotation_from_root: runtime.handle_rotation_from_root,
            muzzle_from_root: runtime.muzzle_from_root,
            left_grip_from_handle: runtime.left_grip_from_handle,
            stock_contact_from_handle: runtime.stock_contact_from_handle,
            ready_shoulder_pocket_offset: runtime.ready_shoulder_pocket_offset,
            ads_shoulder_pocket_offset: runtime.ads_shoulder_pocket_offset,
            fire_kick_duration_seconds: runtime.fire_kick_duration_seconds,
            fire_kick_pitch_radians: runtime.fire_kick_pitch_radians,
            ready_body_to_root_rotation: runtime.ready_body_to_root_rotation,
            ready_right_elbow_pole_offset: runtime.ready_right_elbow_pole_offset,
            ready_left_elbow_pole_offset: runtime.ready_left_elbow_pole_offset,
            ready_left_palm_to_left_grip: runtime.ready_left_palm_to_left_grip,
            ready_right_palm_to_weapon: runtime.ready_right_palm_to_weapon,
            ready_left_palm_to_weapon: runtime.ready_left_palm_to_weapon,
            right_palm_to_handle: runtime.right_palm_to_handle,
            right_palm_to_native_rig: runtime.right_palm_to_native_rig,
            native_rig_to_runtime_basis: runtime.native_rig_to_runtime_basis,
            authored_socket_to_weapon_handle_basis: runtime.authored_socket_to_weapon_handle_basis,
            first_person_hip_handle_offset: runtime.first_person_hip_handle_offset,
            first_person_full_body_hip_handle_offset: None,
            ads_rear_sight_from_handle: runtime.ads_rear_sight_from_handle,
            ads_front_sight_from_handle: runtime.ads_front_sight_from_handle,
            ads_camera_to_rear_sight: runtime.ads_camera_to_rear_sight,
            ads_camera_translation_weight: runtime.ads_camera_translation_weight,
            first_person_hip_convergence_m: runtime.first_person_hip_convergence_m,
            aim_response_hz: runtime.aim_response_hz,
            secondary_hip_max_angle_radians: runtime.secondary_hip_max_angle_radians,
            secondary_ads_max_angle_radians: runtime.secondary_ads_max_angle_radians,
            secondary_angular_inertia_gain: runtime.secondary_angular_inertia_gain,
            secondary_movement_inertia_gain: runtime.secondary_movement_inertia_gain,
            secondary_natural_hz_hip: runtime.secondary_natural_hz_hip,
            secondary_natural_hz_ads: runtime.secondary_natural_hz_ads,
            secondary_obstruction_hz_boost: runtime.secondary_obstruction_hz_boost,
        }
    }
}

impl AuthoredWeaponPresentationDefinition {
    pub(super) fn compile(&self) -> WeaponPresentationDefinition {
        WeaponPresentationDefinition {
            enabled: self.enabled,
            handle_from_root: self.handle_from_root,
            handle_rotation_from_root: self.handle_rotation_from_root,
            muzzle_from_root: self.muzzle_from_root,
            left_grip_from_handle: self.left_grip_from_handle,
            stock_contact_from_handle: self.stock_contact_from_handle,
            ready_shoulder_pocket_offset: self.ready_shoulder_pocket_offset,
            ads_shoulder_pocket_offset: self.ads_shoulder_pocket_offset,
            fire_kick_duration_seconds: self.fire_kick_duration_seconds,
            fire_kick_pitch_radians: self.fire_kick_pitch_radians,
            ready_body_to_root_rotation: self.ready_body_to_root_rotation,
            ready_right_elbow_pole_offset: self.ready_right_elbow_pole_offset,
            ready_left_elbow_pole_offset: self.ready_left_elbow_pole_offset,
            ready_left_palm_to_left_grip: self.ready_left_palm_to_left_grip,
            ready_right_palm_to_weapon: self.ready_right_palm_to_weapon,
            ready_left_palm_to_weapon: self.ready_left_palm_to_weapon,
            right_palm_to_handle: self.right_palm_to_handle,
            right_palm_to_native_rig: self.right_palm_to_native_rig,
            native_rig_to_runtime_basis: self.native_rig_to_runtime_basis,
            authored_socket_to_weapon_handle_basis: self.authored_socket_to_weapon_handle_basis,
            first_person_hip_handle_offset: self.first_person_hip_handle_offset,
            first_person_full_body_hip_handle_offset: self
                .first_person_full_body_hip_handle_offset
                .unwrap_or(self.first_person_hip_handle_offset),
            ads_rear_sight_from_handle: self.ads_rear_sight_from_handle,
            ads_front_sight_from_handle: self.ads_front_sight_from_handle,
            ads_camera_to_rear_sight: self.ads_camera_to_rear_sight,
            ads_camera_translation_weight: self.ads_camera_translation_weight,
            first_person_hip_convergence_m: self.first_person_hip_convergence_m,
            aim_response_hz: self.aim_response_hz,
            secondary_hip_max_angle_radians: self.secondary_hip_max_angle_radians,
            secondary_ads_max_angle_radians: self.secondary_ads_max_angle_radians,
            secondary_angular_inertia_gain: self.secondary_angular_inertia_gain,
            secondary_movement_inertia_gain: self.secondary_movement_inertia_gain,
            secondary_natural_hz_hip: self.secondary_natural_hz_hip,
            secondary_natural_hz_ads: self.secondary_natural_hz_ads,
            secondary_obstruction_hz_boost: self.secondary_obstruction_hz_boost,
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponCasingDefinition {
    pub model_dictionary: String,
    pub variants: Vec<String>,
    pub material_ref: String,
    pub half_extents: [f32; 3],
    pub ejection_delay_seconds: f32,
    pub ejection_joint: String,
    pub inherit_socket_linear_velocity: f32,
    pub inherit_socket_angular_velocity: f32,
    /// Local `[right, up, forward]` coefficients relative to the authored ejection socket.
    pub origin_local: [f32; 3],
    pub velocity_local: [f32; 3],
    pub velocity_jitter: [f32; 3],
    pub axis_local: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub angular_velocity_jitter: [f32; 3],
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub contact_min_impulse: f32,
    pub contact_medium_impulse: f32,
    pub contact_hard_impulse: f32,
    pub soft_surface_contains: Vec<String>,
}

impl Default for AuthoredWeaponCasingDefinition {
    fn default() -> Self {
        let runtime = WeaponCasingDefinition::default();
        Self {
            model_dictionary: String::new(),
            variants: Vec::new(),
            material_ref: String::new(),
            half_extents: runtime.half_extents,
            ejection_delay_seconds: runtime.ejection_delay_seconds,
            ejection_joint: runtime.ejection_joint.unwrap_or_default(),
            inherit_socket_linear_velocity: runtime.inherit_socket_linear_velocity,
            inherit_socket_angular_velocity: runtime.inherit_socket_angular_velocity,
            origin_local: runtime.origin_local,
            velocity_local: runtime.velocity_local,
            velocity_jitter: runtime.velocity_jitter,
            axis_local: runtime.axis_local,
            angular_velocity: runtime.angular_velocity,
            angular_velocity_jitter: runtime.angular_velocity_jitter,
            friction: runtime.friction,
            restitution: runtime.restitution,
            density: runtime.density,
            linear_damping: runtime.linear_damping,
            angular_damping: runtime.angular_damping,
            contact_min_impulse: runtime.contact_min_impulse,
            contact_medium_impulse: runtime.contact_medium_impulse,
            contact_hard_impulse: runtime.contact_hard_impulse,
            soft_surface_contains: runtime.soft_surface_contains,
        }
    }
}

impl AuthoredWeaponCasingDefinition {
    pub(super) fn compile(&self) -> WeaponCasingDefinition {
        WeaponCasingDefinition {
            model_dictionary: (!self.model_dictionary.trim().is_empty())
                .then(|| self.model_dictionary.trim().replace('\\', "/")),
            variants: self.variants.clone(),
            material_ref: (!self.material_ref.trim().is_empty())
                .then(|| self.material_ref.trim().replace('\\', "/")),
            half_extents: self.half_extents,
            ejection_delay_seconds: self.ejection_delay_seconds,
            ejection_joint: (!self.ejection_joint.trim().is_empty())
                .then(|| self.ejection_joint.trim().to_owned()),
            inherit_socket_linear_velocity: self.inherit_socket_linear_velocity,
            inherit_socket_angular_velocity: self.inherit_socket_angular_velocity,
            origin_local: self.origin_local,
            velocity_local: self.velocity_local,
            velocity_jitter: self.velocity_jitter,
            axis_local: self.axis_local,
            angular_velocity: self.angular_velocity,
            angular_velocity_jitter: self.angular_velocity_jitter,
            friction: self.friction,
            restitution: self.restitution,
            density: self.density,
            linear_damping: self.linear_damping,
            angular_damping: self.angular_damping,
            contact_min_impulse: self.contact_min_impulse,
            contact_medium_impulse: self.contact_medium_impulse,
            contact_hard_impulse: self.contact_hard_impulse,
            soft_surface_contains: self.soft_surface_contains.clone(),
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponVfxDefinition {
    pub shot: String,
    pub tracer: String,
    pub ricochet: String,
    pub exit: String,
    pub impact_default: String,
    pub impact_by_surface: std::collections::BTreeMap<String, String>,
}

impl AuthoredWeaponVfxDefinition {
    pub(super) fn compile(&self) -> WeaponVfxDefinition {
        WeaponVfxDefinition {
            shot: (!self.shot.trim().is_empty()).then(|| self.shot.trim().to_owned()),
            tracer: (!self.tracer.trim().is_empty()).then(|| self.tracer.trim().to_owned()),
            ricochet: (!self.ricochet.trim().is_empty()).then(|| self.ricochet.trim().to_owned()),
            exit: (!self.exit.trim().is_empty()).then(|| self.exit.trim().to_owned()),
            impact_default: (!self.impact_default.trim().is_empty())
                .then(|| self.impact_default.trim().to_owned()),
            impact_by_surface: self.impact_by_surface.clone(),
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredWeaponAudioDefinition {
    pub fire: String,
    pub reload_start: String,
    pub reload_complete: String,
    pub equip: String,
    pub unequip: String,
    pub empty: String,
    pub shell_eject: String,
    pub shell_contact_small: String,
    pub shell_contact_medium: String,
    pub shell_contact_hard: String,
    pub shell_contact_soft: String,
}

impl AuthoredWeaponAudioDefinition {
    pub(super) fn compile(&self) -> WeaponAudioDefinition {
        fn clip(value: &str) -> Option<String> {
            let value = value.trim().replace('\\', "/");
            (!value.is_empty()).then_some(value)
        }
        WeaponAudioDefinition {
            fire: clip(&self.fire),
            reload_start: clip(&self.reload_start),
            reload_complete: clip(&self.reload_complete),
            equip: clip(&self.equip),
            unequip: clip(&self.unequip),
            empty: clip(&self.empty),
            shell_eject: clip(&self.shell_eject),
            shell_contact_small: clip(&self.shell_contact_small),
            shell_contact_medium: clip(&self.shell_contact_medium),
            shell_contact_hard: clip(&self.shell_contact_hard),
            shell_contact_soft: clip(&self.shell_contact_soft),
        }
        .sanitized()
    }
}
