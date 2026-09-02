#[cfg(test)]
mod grounding_tests {
    use super::*;
    use newengine_engine_runtime::gameplay::{
        apply_player_stance_geometry, spawn_default_player, PlayerModelAssignment,
        PlayerModelBinding, PlayerStanceKind,
    };

    #[test]
    fn first_person_semantic_mask_hides_face_shell_without_skin_stream() {
        let face = PlayerRuntimeModelPart {
            source_mesh_name: "character_face_shell".to_owned(),
            primitive_id: PrimitiveId(1),
            first_person_primitive_id: None,
            material_id: MaterialId(1),
            material_slot: "m_face".to_owned(),
            color: [1.0; 4],
            skin: None,
        };
        let body = PlayerRuntimeModelPart {
            source_mesh_name: "character_torso".to_owned(),
            primitive_id: PrimitiveId(2),
            first_person_primitive_id: None,
            material_id: MaterialId(2),
            material_slot: "m_body".to_owned(),
            color: [1.0; 4],
            skin: None,
        };
        assert_eq!(
            runtime_part_visibility_policy(&face, None),
            newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::HideInFirstPerson
        );
        assert_eq!(
            runtime_part_visibility_policy(&body, None),
            newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::AlwaysVisible
        );
    }

    #[test]
    fn visual_root_preserves_world_foot_plane_when_crouching() {
        let mut world = newengine_ecs::World::new();
        let player = spawn_default_player(
            &mut world,
            None,
            "crouch-grounding",
            Vec3::new(2.0, 3.0, -4.0),
        );
        let visual_root = world.spawn();
        let local_offset = Vec3::new(0.15, 0.08, -0.12);
        let _ = world.insert(
            visual_root,
            Transform {
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        );
        set_parent(&mut world, visual_root, Some(player));
        let _ = world.insert(
            player,
            PlayerModelAssignment {
                enabled: true,
                revision: 1,
                local_offset,
                ..PlayerModelAssignment::default()
            },
        );
        let _ = world.insert(
            player,
            PlayerModelBinding {
                assignment_revision: 1,
                visual_root: Some(visual_root),
                ..PlayerModelBinding::default()
            },
        );

        tick_player_model_grounding(&mut world);
        let standing_center_y = world
            .get::<Transform>(player)
            .expect("player transform")
            .position
            .y;
        let standing_root_y = world
            .get::<Transform>(visual_root)
            .expect("visual transform")
            .position
            .y;
        let standing_world_anchor_y = standing_center_y + standing_root_y;

        assert!(
            apply_player_stance_geometry(&mut world, player, PlayerStanceKind::Crouched, 41),
            "crouch geometry must apply"
        );
        tick_player_model_grounding(&mut world);

        let crouched_center_y = world
            .get::<Transform>(player)
            .expect("player transform")
            .position
            .y;
        let crouched_root_y = world
            .get::<Transform>(visual_root)
            .expect("visual transform")
            .position
            .y;
        let crouched_world_anchor_y = crouched_center_y + crouched_root_y;

        assert!(
            (standing_world_anchor_y - crouched_world_anchor_y).abs() <= 1.0e-5,
            "visual root moved through support plane standing={standing_world_anchor_y} crouched={crouched_world_anchor_y}"
        );
        assert!(
            crouched_root_y > standing_root_y,
            "shorter crouch capsule must raise child local root to compensate the lowered capsule center"
        );
        assert!(
            (world.get::<Transform>(visual_root).unwrap().position.x - local_offset.x).abs()
                <= 1.0e-6
        );
        assert!(
            (world.get::<Transform>(visual_root).unwrap().position.z - local_offset.z).abs()
                <= 1.0e-6
        );
    }
}
