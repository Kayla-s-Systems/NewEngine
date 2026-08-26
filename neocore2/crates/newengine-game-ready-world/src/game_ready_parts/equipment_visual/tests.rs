#[cfg(test)]
mod alignment_tests {
    use super::*;

    #[test]
    fn canonical_rifle_visual_space_accepts_handle_centered_source() {
        let min = Vec3::new(-0.069_917_45, -0.065_805_55, -0.372_692_38);
        let max = Vec3::new(0.120_714_34, 0.127_575_71, 0.633_752_35);
        validate_canonical_rifle_visual_space(min, max).expect("canonical rifle space");
    }

    #[test]
    fn stale_crowd_space_rifle_is_rejected() {
        let min = Vec3::new(0.051_332_55, 0.582_858_7, -0.221_605_55);
        let max = Vec3::new(0.241_964_34, 1.589_303_4, -0.028_224_29);
        assert!(validate_canonical_rifle_visual_space(min, max).is_err());
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
        let mut value = 1.0_f32;
        for _ in 0..12 {
            let next = value * (-RIFLE_RECOIL_RECOVERY_HZ * (1.0 / 60.0)).exp();
            assert!(next >= 0.0 && next < value);
            value = next;
        }
        assert!(value < 0.05);
    }

    #[test]
    fn first_person_aim_alpha_converges_without_overshoot() {
        let mut value = 0.0;
        for _ in 0..30 {
            value = smooth_first_person_aim_alpha(value, 1.0, 1.0 / 60.0);
            assert!((0.0..=1.0).contains(&value));
        }
        assert!(value > 0.99);
        let released = smooth_first_person_aim_alpha(value, 0.0, 1.0 / 60.0);
        assert!(released < value);
    }

    #[test]
    fn first_person_aim_reads_current_render_frame_command_before_fixed_step() {
        let mut world = newengine_ecs::World::new();
        let owner = world.spawn();
        let mut commands = PlayerCommandFrame::default();
        commands
            .actions
            .held
            .push(newengine_gameplay_fps_api::action::PLAYER_AIM.to_owned());
        let _ = world.insert(owner, commands);
        assert!(first_person_aim_held(&world, owner));
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
