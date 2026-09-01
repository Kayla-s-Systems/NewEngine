#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponAudioAction {
    #[default]
    Fire,
    ReloadStart,
    ReloadComplete,
    Equip,
    Unequip,
    Empty,
    ShellEject,
    ShellContactSmall,
    ShellContactMedium,
    ShellContactHard,
    ShellContactSoft,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponAnimationDefinition {
    pub skeleton: Option<String>,
    pub animation_dictionary: Option<String>,
    pub idle: Option<String>,
    pub fire: Option<String>,
    pub reload: Option<String>,
    pub spawn_pose: Option<String>,
}

impl WeaponAnimationDefinition {
    pub fn sanitized(mut self) -> Self {
        fn clean(value: Option<String>) -> Option<String> {
            value
                .map(|value| value.trim().replace('\\', "/"))
                .filter(|value| !value.is_empty())
        }
        self.skeleton = clean(self.skeleton);
        self.animation_dictionary = clean(self.animation_dictionary);
        self.idle = clean(self.idle);
        self.fire = clean(self.fire);
        self.reload = clean(self.reload);
        self.spawn_pose = clean(self.spawn_pose);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponAudioDefinition {
    pub fire: Option<String>,
    pub reload_start: Option<String>,
    pub reload_complete: Option<String>,
    pub equip: Option<String>,
    pub unequip: Option<String>,
    pub empty: Option<String>,
    pub shell_eject: Option<String>,
    pub shell_contact_small: Option<String>,
    pub shell_contact_medium: Option<String>,
    pub shell_contact_hard: Option<String>,
    pub shell_contact_soft: Option<String>,
}

impl WeaponAudioDefinition {
    pub fn sanitized(mut self) -> Self {
        fn clean(value: Option<String>) -> Option<String> {
            value
                .map(|value| value.trim().replace('\\', "/"))
                .filter(|value| !value.is_empty())
        }
        self.fire = clean(self.fire);
        self.reload_start = clean(self.reload_start);
        self.reload_complete = clean(self.reload_complete);
        self.equip = clean(self.equip);
        self.unequip = clean(self.unequip);
        self.empty = clean(self.empty);
        self.shell_eject = clean(self.shell_eject);
        self.shell_contact_small = clean(self.shell_contact_small);
        self.shell_contact_medium = clean(self.shell_contact_medium);
        self.shell_contact_hard = clean(self.shell_contact_hard);
        self.shell_contact_soft = clean(self.shell_contact_soft);
        self
    }

    #[inline]
    pub fn clip(&self, action: WeaponAudioAction) -> Option<&str> {
        match action {
            WeaponAudioAction::Fire => self.fire.as_deref(),
            WeaponAudioAction::ReloadStart => self.reload_start.as_deref(),
            WeaponAudioAction::ReloadComplete => self.reload_complete.as_deref(),
            WeaponAudioAction::Equip => self.equip.as_deref(),
            WeaponAudioAction::Unequip => self.unequip.as_deref(),
            WeaponAudioAction::Empty => self.empty.as_deref(),
            WeaponAudioAction::ShellEject => self.shell_eject.as_deref(),
            WeaponAudioAction::ShellContactSmall => self.shell_contact_small.as_deref(),
            WeaponAudioAction::ShellContactMedium => self.shell_contact_medium.as_deref(),
            WeaponAudioAction::ShellContactHard => self.shell_contact_hard.as_deref(),
            WeaponAudioAction::ShellContactSoft => self.shell_contact_soft.as_deref(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponPresentationDefinition {
    pub enabled: bool,
    pub handle_from_root: [f32; 3],
    pub muzzle_from_root: [f32; 3],
    pub left_grip_from_handle: [f32; 3],
    pub stock_contact_from_handle: [f32; 3],
    pub ready_shoulder_pocket_offset: [f32; 3],
    pub ads_shoulder_pocket_offset: [f32; 3],
    pub fire_kick_duration_seconds: f32,
    pub fire_kick_pitch_radians: f32,
    /// Third-person ReadyHold body -> weapon native-rig rotation. The runtime must compose
    /// `native_rig_to_runtime_basis` after this quaternion exactly once.
    pub ready_body_to_root_rotation: [f32; 4],
    pub ready_right_elbow_pole_offset: [f32; 3],
    pub ready_left_elbow_pole_offset: [f32; 3],
    pub ready_left_palm_to_left_grip: [f32; 3],
    pub ready_right_palm_to_weapon: [f32; 4],
    pub ready_left_palm_to_weapon: [f32; 4],
    pub right_palm_to_handle: [f32; 3],
    pub right_palm_to_native_rig: [f32; 4],
    /// The single weapon native-rig -> North Star canonical runtime basis correction.
    /// Third-person, first-person, grip, muzzle and ADS presentation must all consume this same
    /// basis; view-specific orientation compensation is forbidden.
    pub native_rig_to_runtime_basis: [f32; 4],
    /// Camera/viewmodel hip handle placement. Kept separate from anatomical full-body reach.
    pub first_person_hip_handle_offset: [f32; 3],
    /// Camera-owned full-body FPP handle placement. This must keep both authored arm contacts
    /// physically reachable; weapon definitions own the value rather than a runtime reach clamp.
    pub first_person_full_body_hip_handle_offset: [f32; 3],
    pub ads_rear_sight_from_handle: [f32; 3],
    pub ads_front_sight_from_handle: [f32; 3],
    pub ads_camera_to_rear_sight: [f32; 3],
    /// Per-axis weight applied when resolving the rendered-weapon ADS camera anchor against the
    /// stable anatomical eye center. `[1,0,1]` consumes lateral/depth eye-relief while preserving
    /// eye height; `[1,1,1]` follows the complete weapon-derived anchor.
    pub ads_camera_translation_weight: [f32; 3],
    pub first_person_hip_convergence_m: f32,
    /// Response speed for authored ADS/ready interpolation.
    pub aim_response_hz: f32,
    /// Maximum bounded secondary angular lag in hip/ready presentation.
    pub secondary_hip_max_angle_radians: f32,
    /// Maximum bounded secondary angular lag while aiming.
    pub secondary_ads_max_angle_radians: f32,
    /// Angular target-motion inertia gain.
    pub secondary_angular_inertia_gain: f32,
    /// Player acceleration -> weapon inertia gain.
    pub secondary_movement_inertia_gain: f32,
    pub secondary_natural_hz_hip: f32,
    pub secondary_natural_hz_ads: f32,
    pub secondary_obstruction_hz_boost: f32,
}

impl Default for WeaponPresentationDefinition {
    fn default() -> Self {
        Self {
            enabled: false,
            handle_from_root: [0.0; 3],
            muzzle_from_root: [0.0, 0.0, 0.5],
            left_grip_from_handle: [0.0, 0.0, 0.25],
            stock_contact_from_handle: [0.0, 0.0, -0.25],
            ready_shoulder_pocket_offset: [0.0, -0.1, -0.04],
            ads_shoulder_pocket_offset: [0.0, -0.08, -0.03],
            fire_kick_duration_seconds: 0.15,
            fire_kick_pitch_radians: 0.0,
            ready_body_to_root_rotation: [0.0, 0.0, 0.0, 1.0],
            ready_right_elbow_pole_offset: [-0.15, -0.14, 0.06],
            ready_left_elbow_pole_offset: [0.15, -0.16, 0.08],
            ready_left_palm_to_left_grip: [0.0; 3],
            ready_right_palm_to_weapon: [0.0, 0.0, 0.0, 1.0],
            ready_left_palm_to_weapon: [0.0, 0.0, 0.0, 1.0],
            right_palm_to_handle: [0.0; 3],
            right_palm_to_native_rig: [0.0, 0.0, 0.0, 1.0],
            native_rig_to_runtime_basis: [0.0, 0.0, 0.0, 1.0],
            first_person_hip_handle_offset: [0.2, -0.2, -0.5],
            // Compatibility default only. Authored item compilation inherits the ordinary FPP
            // offset when no explicit full-body value exists.
            first_person_full_body_hip_handle_offset: [0.2, -0.2, -0.5],
            ads_rear_sight_from_handle: [0.0; 3],
            ads_front_sight_from_handle: [0.0, 0.0, 0.4],
            ads_camera_to_rear_sight: [0.0, 0.0, -0.075],
            ads_camera_translation_weight: [1.0, 0.0, 1.0],
            first_person_hip_convergence_m: 12.0,
            aim_response_hz: 18.0,
            secondary_hip_max_angle_radians: 5.0_f32.to_radians(),
            secondary_ads_max_angle_radians: 2.25_f32.to_radians(),
            secondary_angular_inertia_gain: 0.38,
            secondary_movement_inertia_gain: 1.0,
            secondary_natural_hz_hip: 5.4,
            secondary_natural_hz_ads: 9.0,
            secondary_obstruction_hz_boost: 6.0,
        }
    }
}

impl WeaponPresentationDefinition {
    pub fn sanitized(mut self) -> Self {
        fn vec3(value: [f32; 3], fallback: [f32; 3], limit: f32) -> [f32; 3] {
            let mut out = value;
            for (index, component) in out.iter_mut().enumerate() {
                if !component.is_finite() || component.abs() > limit {
                    *component = fallback[index];
                }
            }
            out
        }
        fn weight3(value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
            let mut out = value;
            for (index, component) in out.iter_mut().enumerate() {
                *component = if component.is_finite() {
                    component.clamp(0.0, 1.0)
                } else {
                    fallback[index]
                };
            }
            out
        }
        fn quat(value: [f32; 4]) -> [f32; 4] {
            let len2 = value.iter().map(|value| value * value).sum::<f32>();
            if value.iter().all(|value| value.is_finite()) && len2 > 1.0e-8 {
                let inv = len2.sqrt().recip();
                [
                    value[0] * inv,
                    value[1] * inv,
                    value[2] * inv,
                    value[3] * inv,
                ]
            } else {
                [0.0, 0.0, 0.0, 1.0]
            }
        }
        let fallback = Self::default();
        self.handle_from_root = vec3(self.handle_from_root, fallback.handle_from_root, 10.0);
        self.muzzle_from_root = vec3(self.muzzle_from_root, fallback.muzzle_from_root, 10.0);
        self.left_grip_from_handle = vec3(
            self.left_grip_from_handle,
            fallback.left_grip_from_handle,
            10.0,
        );
        self.stock_contact_from_handle = vec3(
            self.stock_contact_from_handle,
            fallback.stock_contact_from_handle,
            10.0,
        );
        self.ready_shoulder_pocket_offset = vec3(
            self.ready_shoulder_pocket_offset,
            fallback.ready_shoulder_pocket_offset,
            5.0,
        );
        self.ads_shoulder_pocket_offset = vec3(
            self.ads_shoulder_pocket_offset,
            fallback.ads_shoulder_pocket_offset,
            5.0,
        );
        self.ready_right_elbow_pole_offset = vec3(
            self.ready_right_elbow_pole_offset,
            fallback.ready_right_elbow_pole_offset,
            5.0,
        );
        self.ready_left_elbow_pole_offset = vec3(
            self.ready_left_elbow_pole_offset,
            fallback.ready_left_elbow_pole_offset,
            5.0,
        );
        self.ready_left_palm_to_left_grip = vec3(
            self.ready_left_palm_to_left_grip,
            fallback.ready_left_palm_to_left_grip,
            5.0,
        );
        self.right_palm_to_handle = vec3(
            self.right_palm_to_handle,
            fallback.right_palm_to_handle,
            5.0,
        );
        self.first_person_hip_handle_offset = vec3(
            self.first_person_hip_handle_offset,
            fallback.first_person_hip_handle_offset,
            5.0,
        );
        self.first_person_full_body_hip_handle_offset = vec3(
            self.first_person_full_body_hip_handle_offset,
            self.first_person_hip_handle_offset,
            5.0,
        );
        self.ads_rear_sight_from_handle = vec3(
            self.ads_rear_sight_from_handle,
            fallback.ads_rear_sight_from_handle,
            5.0,
        );
        self.ads_front_sight_from_handle = vec3(
            self.ads_front_sight_from_handle,
            fallback.ads_front_sight_from_handle,
            5.0,
        );
        self.ads_camera_to_rear_sight = vec3(
            self.ads_camera_to_rear_sight,
            fallback.ads_camera_to_rear_sight,
            5.0,
        );
        self.ads_camera_translation_weight = weight3(
            self.ads_camera_translation_weight,
            fallback.ads_camera_translation_weight,
        );
        self.ready_body_to_root_rotation = quat(self.ready_body_to_root_rotation);
        self.ready_right_palm_to_weapon = quat(self.ready_right_palm_to_weapon);
        self.ready_left_palm_to_weapon = quat(self.ready_left_palm_to_weapon);
        self.right_palm_to_native_rig = quat(self.right_palm_to_native_rig);
        self.native_rig_to_runtime_basis = quat(self.native_rig_to_runtime_basis);
        self.fire_kick_duration_seconds = if self.fire_kick_duration_seconds.is_finite() {
            self.fire_kick_duration_seconds.clamp(0.001, 10.0)
        } else {
            fallback.fire_kick_duration_seconds
        };
        self.fire_kick_pitch_radians = if self.fire_kick_pitch_radians.is_finite() {
            self.fire_kick_pitch_radians
                .clamp(-std::f32::consts::PI, std::f32::consts::PI)
        } else {
            0.0
        };
        self.first_person_hip_convergence_m = if self.first_person_hip_convergence_m.is_finite() {
            self.first_person_hip_convergence_m.clamp(0.1, 10_000.0)
        } else {
            fallback.first_person_hip_convergence_m
        };
        self.aim_response_hz =
            finite_or(self.aim_response_hz, fallback.aim_response_hz).clamp(0.1, 120.0);
        self.secondary_hip_max_angle_radians = finite_or(
            self.secondary_hip_max_angle_radians,
            fallback.secondary_hip_max_angle_radians,
        )
        .clamp(0.0, std::f32::consts::FRAC_PI_2);
        self.secondary_ads_max_angle_radians = finite_or(
            self.secondary_ads_max_angle_radians,
            fallback.secondary_ads_max_angle_radians,
        )
        .clamp(0.0, std::f32::consts::FRAC_PI_2);
        self.secondary_angular_inertia_gain = finite_or(
            self.secondary_angular_inertia_gain,
            fallback.secondary_angular_inertia_gain,
        )
        .clamp(0.0, 4.0);
        self.secondary_movement_inertia_gain = finite_or(
            self.secondary_movement_inertia_gain,
            fallback.secondary_movement_inertia_gain,
        )
        .clamp(0.0, 4.0);
        self.secondary_natural_hz_hip = finite_or(
            self.secondary_natural_hz_hip,
            fallback.secondary_natural_hz_hip,
        )
        .clamp(0.1, 120.0);
        self.secondary_natural_hz_ads = finite_or(
            self.secondary_natural_hz_ads,
            fallback.secondary_natural_hz_ads,
        )
        .clamp(0.1, 120.0);
        self.secondary_obstruction_hz_boost = finite_or(
            self.secondary_obstruction_hz_boost,
            fallback.secondary_obstruction_hz_boost,
        )
        .clamp(0.0, 120.0);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponVfxDefinition {
    pub shot: Option<String>,
    /// Independent single-frame/swept tracer presentation for an already-resolved shot segment.
    pub tracer: Option<String>,
    /// Shallow-angle collision sweetener spawned only when ballistics schedules a ricochet trace.
    pub ricochet: Option<String>,
    /// Exit-side particle composition emitted only after a successful material penetration.
    pub exit: Option<String>,
    pub impact_default: Option<String>,
    pub impact_by_surface: BTreeMap<String, String>,
}

impl WeaponVfxDefinition {
    pub fn sanitized(mut self) -> Self {
        fn clean(value: Option<String>) -> Option<String> {
            value
                .map(|value| value.trim().replace('\\', "/"))
                .filter(|value| !value.is_empty())
        }
        self.shot = clean(self.shot);
        self.tracer = clean(self.tracer);
        self.ricochet = clean(self.ricochet);
        self.exit = clean(self.exit);
        self.impact_default = clean(self.impact_default);
        self.impact_by_surface = self
            .impact_by_surface
            .into_iter()
            .filter_map(|(surface, effect)| {
                let surface = surface.trim().to_ascii_lowercase();
                let effect = effect.trim().replace('\\', "/");
                (!surface.is_empty() && !effect.is_empty()).then_some((surface, effect))
            })
            .collect();
        self
    }

    #[inline]
    pub fn impact_effect(&self, surface: Option<&str>) -> Option<&str> {
        let surface = surface.map(|value| value.trim().to_ascii_lowercase());
        if let Some(surface) = surface.as_deref() {
            if let Some(exact) = self.impact_by_surface.get(surface) {
                return Some(exact.as_str());
            }
            // Physics surfaces are commonly hierarchical (`surface.metal.floor`,
            // `environment.concrete.wall`, ...). Project-authored impact rules are semantic
            // match tokens; prefer the longest matching token so a specific rule wins over a
            // broad material family without requiring runtime hard-coding.
            if let Some((_, effect)) = self
                .impact_by_surface
                .iter()
                .filter(|(needle, _)| !needle.is_empty() && surface.contains(needle.as_str()))
                .max_by_key(|(needle, _)| needle.len())
            {
                return Some(effect.as_str());
            }
        }
        self.impact_default.as_deref()
    }

    pub fn effect_refs(&self) -> impl Iterator<Item = &str> {
        self.shot
            .iter()
            .map(String::as_str)
            .chain(self.tracer.iter().map(String::as_str))
            .chain(self.ricochet.iter().map(String::as_str))
            .chain(self.exit.iter().map(String::as_str))
            .chain(self.impact_default.iter().map(String::as_str))
            .chain(self.impact_by_surface.values().map(String::as_str))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponCasingDefinition {
    /// Runtime model dictionary; `variants` are entry selectors inside this dictionary.
    pub model_dictionary: Option<String>,
    pub variants: Vec<String>,
    pub material_ref: Option<String>,
    /// Dynamic rigid-body box half extents in metres.
    pub half_extents: [f32; 3],
    /// Delay from the shot event to physical casing spawn.
    pub ejection_delay_seconds: f32,
    /// Optional joint/socket on the weapon skeleton. If authored, casing emission follows the
    /// animated weapon entity rather than reconstructing a pose from the player or camera.
    pub ejection_joint: Option<String>,
    /// Fraction of measured socket linear/angular velocity inherited by the ejected casing.
    pub inherit_socket_linear_velocity: f32,
    pub inherit_socket_angular_velocity: f32,
    /// Local basis coefficients `[right, up, forward]` relative to the ejection socket pose.
    pub origin_local: [f32; 3],
    pub velocity_local: [f32; 3],
    /// Signed scalar jitter is multiplied component-wise by this vector.
    pub velocity_jitter: [f32; 3],
    /// Local axis used to orient the casing model; interpreted in `[right, up, forward]`.
    pub axis_local: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub angular_velocity_jitter: [f32; 3],
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
    /// Linear drag applied by the physics backend while the casing is dynamic (1/s).
    pub linear_damping: f32,
    /// Angular drag applied by the physics backend while the casing is dynamic (1/s).
    pub angular_damping: f32,
    pub contact_min_impulse: f32,
    pub contact_medium_impulse: f32,
    pub contact_hard_impulse: f32,
    /// Case-insensitive surface-id substrings that select the soft-contact cue.
    pub soft_surface_contains: Vec<String>,
}

impl Default for WeaponCasingDefinition {
    fn default() -> Self {
        // Disabled schema default. A concrete weapon must author its casing contract.
        Self {
            model_dictionary: None,
            variants: Vec::new(),
            material_ref: None,
            half_extents: [0.01, 0.01, 0.01],
            ejection_delay_seconds: 0.0,
            ejection_joint: None,
            inherit_socket_linear_velocity: 1.0,
            inherit_socket_angular_velocity: 0.35,
            origin_local: [0.0; 3],
            velocity_local: [0.0; 3],
            velocity_jitter: [0.0; 3],
            axis_local: [1.0, 0.0, 0.0],
            angular_velocity: [0.0; 3],
            angular_velocity_jitter: [0.0; 3],
            friction: 0.4,
            restitution: 0.1,
            density: 1.0,
            // Legacy cylinder motion policy, now explicit and content-overridable.
            linear_damping: 0.015,
            angular_damping: 0.025,
            contact_min_impulse: 0.0,
            contact_medium_impulse: 0.0,
            contact_hard_impulse: 0.0,
            soft_surface_contains: Vec::new(),
        }
    }
}

impl WeaponCasingDefinition {
    pub fn sanitized(mut self) -> Self {
        fn clean(value: Option<String>) -> Option<String> {
            value
                .map(|value| value.trim().replace('\\', "/"))
                .filter(|value| !value.is_empty())
        }
        fn finite_vec3(mut value: [f32; 3], fallback: [f32; 3], limit: f32) -> [f32; 3] {
            for index in 0..3 {
                value[index] = if value[index].is_finite() {
                    value[index].clamp(-limit, limit)
                } else {
                    fallback[index]
                };
            }
            value
        }
        self.model_dictionary = clean(self.model_dictionary);
        self.material_ref = clean(self.material_ref);
        self.ejection_joint = clean(self.ejection_joint);
        self.variants = self
            .variants
            .into_iter()
            .map(|value| value.trim().trim_start_matches('@').to_owned())
            .filter(|value| !value.is_empty() && !value.contains('/') && !value.contains('\\'))
            .collect();
        self.variants.sort();
        self.variants.dedup();
        self.half_extents = sanitize_positive_vec3(self.half_extents, 0.0005, 1.0);
        self.ejection_delay_seconds = if self.ejection_delay_seconds.is_finite() {
            self.ejection_delay_seconds.clamp(0.0, 2.0)
        } else {
            0.0
        };
        self.inherit_socket_linear_velocity =
            finite_or(self.inherit_socket_linear_velocity, 1.0).clamp(0.0, 4.0);
        self.inherit_socket_angular_velocity =
            finite_or(self.inherit_socket_angular_velocity, 0.35).clamp(0.0, 4.0);
        self.origin_local = finite_vec3(self.origin_local, [0.0; 3], 10.0);
        self.velocity_local = finite_vec3(self.velocity_local, [0.0; 3], 100.0);
        self.velocity_jitter = finite_vec3(self.velocity_jitter, [0.0; 3], 100.0);
        self.axis_local = finite_vec3(self.axis_local, [1.0, 0.0, 0.0], 1.0);
        if self
            .axis_local
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            <= 1.0e-8
        {
            self.axis_local = [1.0, 0.0, 0.0];
        }
        self.angular_velocity = finite_vec3(self.angular_velocity, [0.0; 3], 500.0);
        self.angular_velocity_jitter = finite_vec3(self.angular_velocity_jitter, [0.0; 3], 500.0);
        self.friction = if self.friction.is_finite() {
            self.friction.clamp(0.0, 2.0)
        } else {
            0.4
        };
        self.restitution = if self.restitution.is_finite() {
            self.restitution.clamp(0.0, 1.0)
        } else {
            0.1
        };
        self.density = if self.density.is_finite() {
            // Permit physically meaningful authored material densities (e.g. brass/steel) while
            // keeping pathological values bounded for backend stability.
            self.density.clamp(0.01, 25_000.0)
        } else {
            1.0
        };
        self.linear_damping = finite_or(self.linear_damping, 0.015).clamp(0.0, 20.0);
        self.angular_damping = finite_or(self.angular_damping, 0.025).clamp(0.0, 20.0);
        self.contact_min_impulse = finite_or(self.contact_min_impulse, 0.0).max(0.0);
        self.contact_medium_impulse =
            finite_or(self.contact_medium_impulse, self.contact_min_impulse)
                .max(self.contact_min_impulse);
        self.contact_hard_impulse =
            finite_or(self.contact_hard_impulse, self.contact_medium_impulse)
                .max(self.contact_medium_impulse);
        self.soft_surface_contains = self
            .soft_surface_contains
            .into_iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect();
        self.soft_surface_contains.sort();
        self.soft_surface_contains.dedup();
        self
    }

    #[inline]
    pub fn enabled(&self) -> bool {
        self.model_dictionary.is_some() && !self.variants.is_empty()
    }

    pub fn model_ref(&self, variant_index: usize) -> Option<String> {
        let dictionary = self.model_dictionary.as_deref()?;
        let selector = self
            .variants
            .get(variant_index % self.variants.len().max(1))?;
        Some(format!("{dictionary}@{selector}"))
    }
}
