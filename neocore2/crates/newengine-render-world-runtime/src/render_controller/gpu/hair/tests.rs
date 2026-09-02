use super::*;
use newengine_render_api::{
    HairGroomAssetV1, HairGroomRef, HairGuidePointV1, HairGuideStrandV1, HairSimulationSettingsV1,
};

fn tiny_scene_and_registry() -> (HairSceneV1, HairGroomRegistryV1) {
    let groom_ref = HairGroomRef::new("characters/test/hair.groom");
    let mut registry = HairGroomRegistryV1::default();
    registry
        .insert(HairGroomAssetV1 {
            groom: groom_ref.clone(),
            guide_points: vec![
                HairGuidePointV1 {
                    rest_position: [0.0, 0.0, 0.0],
                    inverse_mass: 0.0,
                },
                HairGuidePointV1 {
                    rest_position: [0.0, -0.1, 0.0],
                    inverse_mass: 1.0,
                },
                HairGuidePointV1 {
                    rest_position: [0.0, -0.2, 0.0],
                    inverse_mass: 1.0,
                },
            ],
            guide_strands: vec![HairGuideStrandV1 {
                first_point: 0,
                point_count: 3,
                group: 0,
                root_uv: [0.5, 0.5],
                root_joint_index: 0,
            }],
            collision_capsules: Vec::new(),
            follow_strands_per_guide: 2,
        })
        .unwrap();
    let mut instance = newengine_render_api::HairInstanceDescV1 {
        instance_id: 1,
        groom: groom_ref,
        simulation: HairSimulationSettingsV1::default(),
        ..Default::default()
    };
    instance.material.strand_width_mm = 0.08;
    let mut scene = HairSceneV1::new(HairShaderSetV1::new(
        "shaders/hair/guide_sim.comp",
        "shaders/hair/strand_ribbon.vert",
        "shaders/hair/strand_ribbon.frag",
    ));
    scene.instances.push(instance);
    (scene, registry)
}

#[test]
fn backend_must_explicitly_negotiate_hair_compute() {
    let mut renderer = HairGpuRenderer::new();
    let mut caps = RenderBackendCapabilities::raster_default();
    renderer.apply_backend_capabilities(&caps);
    assert!(!renderer.backend_supported);

    caps.features.push(RenderFeature::HairStrands);
    caps.features.push(RenderFeature::HairGpuSimulation);
    renderer.apply_backend_capabilities(&caps);
    assert!(renderer.backend_supported);
    assert!(!renderer.backend_shadows_supported);

    caps.features.push(RenderFeature::HairShadows);
    renderer.apply_backend_capabilities(&caps);
    assert!(renderer.backend_shadows_supported);

    caps.limits.max_storage_buffer_range = HAIR_SSBO_BYTES - 1;
    renderer.apply_backend_capabilities(&caps);
    assert!(!renderer.backend_supported);
}

fn f32_at(bytes: &[u8], index: usize) -> f32 {
    let offset = index * 4;
    f32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn test_shadow_frame() -> ShadowFrame {
    let mut frame = ShadowFrame::disabled(TextureId::new(1));
    frame.params = [1.0, 0.0025, 0.85, 1.0];
    frame.cascade_count = 1;
    frame.cascade_splits = [48.0, 48.0, 48.0, 48.0];
    frame.cascades[0].light_mvp = Mat4::orthographic_rh(-8.0, 8.0, -8.0, 8.0, 0.1, 64.0);
    frame.cascade_light_mvp[0] = frame.cascades[0].light_mvp;
    frame
}

#[test]
fn hair_shadow_ubo_uses_current_simulation_write_buffer() {
    let bytes = encode_shadow_ubo(Mat4::IDENTITY, [0.0, -1.0, 0.0, 3.0], 321, POINT_B_BASE, 2);
    assert_eq!(bytes.len(), HAIR_SHADOW_UBO_BYTES as usize);
    assert_eq!(f32_at(&bytes, 20), 321.0);
    assert_eq!(f32_at(&bytes, 21), POINT_B_BASE as f32);
    assert_eq!(f32_at(&bytes, 22), SEGMENT_BASE as f32);
    assert_eq!(f32_at(&bytes, 23), INSTANCE_BASE as f32);
    assert_eq!(f32_at(&bytes, 24), HAIR_INSTANCE_SLOT_COUNT as f32);
    assert_eq!(f32_at(&bytes, 25), 2.0);
}

#[test]
fn hair_frame_shadow_payload_is_append_only_and_texel_scaled() {
    let frame = test_shadow_frame();
    let bias = hair_shadow_receiver_bias(frame, 0, Extent2D::new(2048, 2048));
    assert!(bias.is_finite());
    assert!((0.000002..=0.002).contains(&bias));

    let bytes = encode_frame_ubo(
        Mat4::IDENTITY,
        Vec3::new(1.0, 2.0, 3.0),
        Vec3::X,
        Vec3::Y,
        1.0 / 60.0,
        [0.0, -1.0, 0.0, 2.0],
        [1.0; 4],
        [0.1; 4],
        HairTopologyCounts {
            point_count: 10,
            strand_count: 2,
            render_segment_count: 8,
            rendered_strand_count: 4,
        },
        POINT_A_BASE,
        POINT_B_BASE,
        Vec3::new(0.0, 0.0, -1.0),
        frame,
        Extent2D::new(2048, 2048),
        true,
    );
    assert_eq!(bytes.len(), HAIR_FRAME_UBO_BYTES as usize);
    // Original V1 prefix remains byte/float aligned through index 51.
    assert_eq!(f32_at(&bytes, 40), 10.0);
    assert_eq!(f32_at(&bytes, 44), POINT_A_BASE as f32);
    assert_eq!(f32_at(&bytes, 45), POINT_B_BASE as f32);
    assert_eq!(f32_at(&bytes, 48), INSTANCE_BASE as f32);
    // CSM payload starts only after the old prefix.
    assert_eq!(f32_at(&bytes, 52), 1.0);
    assert_eq!(f32_at(&bytes, 53), 1.0);
    assert_eq!(f32_at(&bytes, 56), 2048.0);
    assert!((f32_at(&bytes, 64) - bias).abs() < 1.0e-8);
    assert_eq!(f32_at(&bytes, 70), -1.0);
}

#[test]
fn shadow_shader_pair_changes_pipeline_cache_identity() {
    let base = HairShaderSetV1::new(
        "shaders/hair/guide_sim.comp",
        "shaders/hair/strand_ribbon.vert",
        "shaders/hair/strand_ribbon.frag",
    );
    let shadowed = base.clone().with_shadows(
        "shaders/hair/strand_shadow.vert",
        "shaders/hair/strand_shadow.frag",
    );
    assert_ne!(shader_set_key(&base), shader_set_key(&shadowed));
}

#[test]
fn topology_expands_followers_without_duplicating_simulation_points() {
    let (scene, registry) = tiny_scene_and_registry();
    let topology = build_topology(&scene, &registry, None).unwrap();
    assert_eq!(topology.counts.point_count, 3);
    assert_eq!(topology.counts.strand_count, 1);
    assert_eq!(topology.counts.rendered_strand_count, 3);
    assert_eq!(topology.counts.render_segment_count, 6);
}

#[test]
fn instance_record_is_four_std430_slots() {
    let (scene, registry) = tiny_scene_and_registry();
    let topology = build_topology(&scene, &registry, None).unwrap();
    let slots = build_instance_slots(&scene, &topology.instance_ranges);
    assert_eq!(slots.len(), HAIR_INSTANCE_SLOT_COUNT);
    assert_eq!(
        slots_to_bytes(&slots).len(),
        HAIR_INSTANCE_SLOT_COUNT * HAIR_SLOT_BYTES
    );
}

#[test]
fn topology_uses_skin_pose_without_hashing_animation_revision() {
    let (mut scene, mut registry) = tiny_scene_and_registry();
    let mut groom = registry
        .get(&scene.instances[0].groom)
        .cloned()
        .expect("test groom");
    groom.guide_strands[0].root_joint_index = 1;
    registry.insert(groom).unwrap();
    scene.instances[0].skin_pose_id = Some(7);

    let identity = Mat4::IDENTITY.to_cols_array();
    let translated = Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)).to_cols_array();
    let mut poses = HairSkinPoseRegistryV1::default();
    poses
        .upsert(newengine_render_api::HairSkinPoseV1 {
            pose_id: 7,
            revision: 1,
            joint_deforms: vec![identity, translated],
        })
        .unwrap();
    let topology = build_topology(&scene, &registry, Some(&poses)).unwrap();
    assert!((topology.points[0].0[0] - 1.0).abs() < 1.0e-5);
    assert_eq!(topology.instance_ranges[0].palette_count, 2);
    assert_eq!(
        build_skin_palette_slots(&scene, Some(&poses), &topology.instance_ranges)
            .unwrap()
            .len(),
        2
    );
    let key_v1 = topology_key(
        &scene,
        registry.generation(),
        poses.layout_generation(),
        Some(&poses),
    );
    poses
        .upsert(newengine_render_api::HairSkinPoseV1 {
            pose_id: 7,
            revision: 2,
            joint_deforms: vec![
                identity,
                Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)).to_cols_array(),
            ],
        })
        .unwrap();
    let key_v2 = topology_key(
        &scene,
        registry.generation(),
        poses.layout_generation(),
        Some(&poses),
    );
    assert_eq!(
        key_v1, key_v2,
        "animation matrix changes must not rebuild topology"
    );
}

#[test]
fn topology_key_ignores_pose_but_tracks_groom_generation() {
    let (mut scene, registry) = tiny_scene_and_registry();
    let before = topology_key(&scene, registry.generation(), 0, None);
    scene.instances[0].root_transform[12] = 10.0;
    let moved = topology_key(&scene, registry.generation(), 0, None);
    assert_eq!(before, moved);
    assert_ne!(
        before,
        topology_key(&scene, registry.generation() + 1, 0, None)
    );
}
