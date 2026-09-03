#[cfg(test)]
mod alignment_tests {
    use super::*;

    #[test]
    fn deterministic_failed_admission_matches_only_same_instance_avatar_and_generation() {
        let item = newengine_engine_runtime::gameplay::ItemId(0x1111);
        let instance = newengine_engine_runtime::gameplay::ItemInstanceId(0x2222);
        let key = WeaponVisualAdmissionKey {
            item,
            instance_id: instance,
            avatar_root: None,
            dependency_generation: 0x3333,
        };
        assert!(weapon_visual_failure_matches(
            key, item, instance, None, 0x3333,
        ));
        assert!(!weapon_visual_failure_matches(
            key, item, instance, None, 0x3334,
        ));
        assert!(!weapon_visual_failure_matches(
            key,
            item,
            newengine_engine_runtime::gameplay::ItemInstanceId(0x2223),
            None,
            0x3333,
        ));
    }

    #[test]
    fn malformed_ycd_is_deterministic_but_readiness_gap_is_transient() {
        assert_eq!(
            classify_weapon_visual_admission_failure(
                "YCD clip payload outside body offset=333 len=282908"
            ),
            WeaponVisualAdmissionFailureClass::Deterministic
        );
        assert_eq!(
            classify_weapon_visual_admission_failure("player avatar visual root is not ready"),
            WeaponVisualAdmissionFailureClass::Transient
        );
    }

    #[test]
    fn canonical_skinned_weapon_visual_space_accepts_handle_centered_rifle() {
        let min = Vec3::new(-0.069_917_45, -0.065_805_55, -0.372_692_38);
        let max = Vec3::new(0.120_714_34, 0.127_575_71, 0.633_752_35);
        validate_canonical_skinned_weapon_visual_space(min, max).expect("canonical rifle space");
    }

    #[test]
    fn canonical_skinned_weapon_visual_space_accepts_native_pistol() {
        let min = Vec3::new(-0.015_536, -0.074_239, -0.044_798);
        let max = Vec3::new(0.015_630, 0.092_383, 0.235_774);
        validate_canonical_skinned_weapon_visual_space(min, max).expect("canonical pistol space");
    }

    #[test]
    fn stale_crowd_space_weapon_is_rejected() {
        let min = Vec3::new(0.051_332_55, 0.582_858_7, -0.221_605_55);
        let max = Vec3::new(0.241_964_34, 1.589_303_4, -0.028_224_29);
        assert!(validate_canonical_skinned_weapon_visual_space(min, max).is_err());
    }
    #[test]
    fn equipped_weapon_is_explicit_cast_and_receive_world_opaque() {
        let options = equipped_weapon_render_options();
        assert_eq!(
            options.role,
            newengine_model_domain_api::MeshRenderRole::WorldOpaque
        );
        assert_eq!(
            options.shadow_policy,
            newengine_model_domain_api::MeshShadowPolicy::CastAndReceive
        );
        assert_eq!(
            options.depth_policy,
            newengine_model_domain_api::MeshDepthPolicy::ReadWrite
        );
    }

    #[test]
    fn rifle_recoil_recovery_is_fast_and_monotonic() {
        let presentation =
            newengine_engine_runtime::gameplay::WeaponPresentationDefinition::default().sanitized();
        let recovery_hz = 3.0 / presentation.fire_kick_duration_seconds.max(0.001);
        let mut value = 1.0_f32;
        for _ in 0..12 {
            let next = value * (-recovery_hz * (1.0 / 60.0)).exp();
            assert!(next >= 0.0 && next < value);
            value = next;
        }
        assert!(value < 0.05);
    }

    #[test]
    fn long_gun_secondary_spring_is_bounded_and_decays_after_fast_turn() {
        let dt = 1.0 / 60.0;
        let presentation =
            newengine_engine_runtime::gameplay::WeaponPresentationDefinition::default().sanitized();
        let mut state = step_long_gun_secondary_dynamics(
            WeaponSecondaryDynamicsState::default(),
            &presentation,
            Quat::IDENTITY,
            Vec3::ZERO,
            Quat::IDENTITY,
            dt,
            0.0,
            0.0,
            0.0,
        );
        state = step_long_gun_secondary_dynamics(
            state,
            &presentation,
            Quat::from_rotation_y(24.0_f32.to_radians()),
            Vec3::ZERO,
            Quat::IDENTITY,
            dt,
            0.0,
            0.0,
            0.0,
        );
        let injected = state.rotation_offset_local.length();
        assert!(
            injected > 0.001,
            "fast target rotation must inject inertial lag"
        );
        assert!(
            injected <= presentation.secondary_hip_max_angle_radians + 1.0e-6,
            "secondary motion must stay inside the authored grip envelope"
        );
        let target = Quat::from_rotation_y(24.0_f32.to_radians());
        for _ in 0..90 {
            state = step_long_gun_secondary_dynamics(
                state,
                &presentation,
                target,
                Vec3::ZERO,
                Quat::IDENTITY,
                dt,
                0.0,
                0.0,
                0.0,
            );
        }
        assert!(
            state.rotation_offset_local.length() < injected * 0.02,
            "critically damped secondary motion must settle back to the authored pose"
        );
    }

    #[test]
    fn authored_secondary_motion_tuning_changes_runtime_response() {
        let dt = 1.0 / 60.0;
        let loose = newengine_engine_runtime::gameplay::WeaponPresentationDefinition {
            secondary_angular_inertia_gain: 0.8,
            secondary_hip_max_angle_radians: 0.12,
            ..Default::default()
        };
        let loose = loose.sanitized();
        let mut tight = loose.clone();
        tight.secondary_angular_inertia_gain = 0.08;
        tight.secondary_hip_max_angle_radians = 0.02;
        let seed =
            |presentation: &newengine_engine_runtime::gameplay::WeaponPresentationDefinition| {
                let state = step_long_gun_secondary_dynamics(
                    WeaponSecondaryDynamicsState::default(),
                    presentation,
                    Quat::IDENTITY,
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    dt,
                    0.0,
                    0.0,
                    0.0,
                );
                step_long_gun_secondary_dynamics(
                    state,
                    presentation,
                    Quat::from_rotation_y(30.0_f32.to_radians()),
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    dt,
                    0.0,
                    0.0,
                    0.0,
                )
                .rotation_offset_local
                .length()
            };
        assert!(seed(&loose) > seed(&tight));
    }

    #[test]
    fn first_person_aim_alpha_converges_without_overshoot() {
        let mut value = 0.0;
        for _ in 0..30 {
            value = smooth_first_person_aim_alpha(value, 1.0, 1.0 / 60.0, 18.0);
            assert!((0.0..=1.0).contains(&value));
        }
        assert!(value > 0.99);
        let released = smooth_first_person_aim_alpha(value, 0.0, 1.0 / 60.0, 18.0);
        assert!(released < value);
    }

    #[test]
    fn raw_aim_command_without_equipped_weapon_cannot_activate_ads() {
        let mut world = newengine_ecs::World::new();
        let owner = world.spawn();
        let mut commands = PlayerCommandFrame::default();
        commands
            .actions
            .held
            .push(newengine_gameplay_fps_api::action::PLAYER_AIM.to_owned());
        let _ = world.insert(owner, commands);
        assert!(!equipped_weapon_aim_held(
            &world,
            owner,
            newengine_engine_runtime::gameplay::ItemInstanceId(1),
        ));
    }

    #[test]
    fn equipped_rifle_material_library_resolves_mesh_slots_to_nemat_entries() {
        assert_eq!(
            equipped_part_material_asset(None, "m00", Some("shared/materials/weapon_rifle.nemat")),
            Some("shared/materials/weapon_rifle.nemat@m00".to_owned())
        );
        assert_eq!(
            equipped_part_material_asset(
                Some("shared/materials/weapon_rifle.nemat"),
                "m01",
                Some("ignored.nemat"),
            ),
            Some("shared/materials/weapon_rifle.nemat@m01".to_owned())
        );
    }
}
