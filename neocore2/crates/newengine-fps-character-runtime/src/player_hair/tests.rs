#[cfg(test)]
mod tests {
    use super::*;
    use newengine_engine_runtime::gameplay::PlayerSkinPose;
    use newengine_math::{Mat4, Vec3};
    use newengine_render_api::{HairGroomRef, HairInstanceDescV1, HairShaderSetV1};

    fn prepared_with_prefixes(prefixes: &[&str]) -> PreparedPlayerHairV1 {
        PreparedPlayerHairV1 {
            groom: HairGroomAssetV1 {
                groom: HairGroomRef::new("characters/test/hair.nehair"),
                guide_points: Vec::new(),
                guide_strands: Vec::new(),
                collision_capsules: Vec::new(),
                follow_strands_per_guide: 0,
            },
            instance: HairInstanceDescV1::default(),
            shaders: HairShaderSetV1::new(
                "shaders/hair/guide_sim.comp",
                "shaders/hair/strand_ribbon.vert",
                "shaders/hair/strand_ribbon.frag",
            ),
            source_mesh_prefixes: prefixes.iter().map(|value| (*value).to_owned()).collect(),
            hide_in_first_person: true,
        }
    }

    #[test]
    fn ellie_cutover_keeps_cap_back_and_scrunchy() {
        let prepared = prepared_with_prefixes(&[
            "default_hair_wet_extra_",
            "default_hair_thick_",
            "default_hair_tail_thin_",
            "default_hair_flying_",
            "default_strand_hair_loose_",
        ]);
        assert!(source_mesh_replaced_by_hair_v1(
            &prepared,
            "default_hair_thick_lod0_LODShape0_shader2_merged_partition0"
        ));
        assert!(source_mesh_replaced_by_hair_v1(
            &prepared,
            "default_strand_hair_loose_lod0_LODShape0_shader1_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "default_hair_base_lod0_LODShape0_shader1_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "default_hair_back_lod0_LODShape0_shader7_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "default_scrunchy_lod0_LODShape0_shader5_merged_partition0"
        ));
    }

    #[test]
    fn isaac_cutover_excludes_beard_brows_fuzz_and_scalp_cap() {
        let prepared = prepared_with_prefixes(&[
            "hair_LODShape0_shader8_",
            "hair_LODShape0_shader10_",
            "hair_LODShape0_shader11_",
        ]);
        assert!(source_mesh_replaced_by_hair_v1(
            &prepared,
            "hair_LODShape0_shader8_merged_partition0"
        ));
        assert!(source_mesh_replaced_by_hair_v1(
            &prepared,
            "hair_LODShape0_shader10_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "hair_LODShape0_shader7_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "hair_LODShape0_shader9_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "beard_LODShape0_shader12_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "eyebrows_LODShape0_shader4_merged_partition0"
        ));
    }

    #[test]
    fn compiled_groom_binding_targets_player_pose_id() {
        let mut world = World::new();
        let player = world.spawn();
        let groom = HairGroomAssetV1 {
            groom: HairGroomRef::new("characters/test/hair.nehair"),
            guide_points: vec![
                newengine_render_api::HairGuidePointV1 {
                    rest_position: [0.0, 0.0, 0.0],
                    inverse_mass: 0.0,
                },
                newengine_render_api::HairGuidePointV1 {
                    rest_position: [0.0, -0.1, 0.0],
                    inverse_mass: 1.0,
                },
            ],
            guide_strands: vec![newengine_render_api::HairGuideStrandV1 {
                first_point: 0,
                point_count: 2,
                group: 0,
                root_uv: [0.5, 0.5],
                root_joint_index: 0,
            }],
            collision_capsules: Vec::new(),
            follow_strands_per_guide: 0,
        };
        let shaders = HairShaderSetV1::new(
            "shaders/hair/guide_sim.comp",
            "shaders/hair/strand_ribbon.vert",
            "shaders/hair/strand_ribbon.frag",
        );
        bind_compiled_player_groom_v1(
            &mut world,
            player,
            groom,
            HairInstanceDescV1 {
                instance_id: 55,
                ..Default::default()
            },
            shaders,
        )
        .unwrap();
        let scene = world.resource::<HairSceneV1>().unwrap();
        assert_eq!(scene.instances.len(), 1);
        assert_eq!(scene.instances[0].skin_pose_id, Some(player.stable_u64()));
        assert_eq!(
            scene.instances[0].groom.as_str(),
            "characters/test/hair.nehair"
        );
    }

    #[test]
    fn player_palette_is_published_without_animation_types_in_hair_contract() {
        let mut world = World::new();
        let player = world.spawn();
        assert!(world.insert(
            player,
            PlayerSkinPose {
                palette: vec![Mat4::IDENTITY],
                revision: 4,
            },
        ));
        let mut scene = HairSceneV1::new(HairShaderSetV1::new(
            "shaders/hair/guide_sim.comp",
            "shaders/hair/strand_ribbon.vert",
            "shaders/hair/strand_ribbon.frag",
        ));
        scene.instances.push(HairInstanceDescV1 {
            instance_id: 9,
            groom: HairGroomRef::new("characters/test/hair.nehair"),
            skin_pose_id: Some(player.stable_u64()),
            ..Default::default()
        });
        world.insert_resource(scene);

        let root = Mat4::from_translation(Vec3::new(3.0, 2.0, 1.0));
        publish_player_hair_pose(&mut world, player, root);

        let registry = world.resource::<HairSkinPoseRegistryV1>().unwrap();
        let pose = registry.get(player.stable_u64()).unwrap();
        assert_eq!(pose.revision, 4);
        assert_eq!(pose.joint_deforms.len(), 1);
        let scene = world.resource::<HairSceneV1>().unwrap();
        assert_eq!(scene.instances[0].root_transform, root.to_cols_array());
    }
}
