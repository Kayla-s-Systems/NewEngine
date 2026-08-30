#[cfg(test)]
mod transition_tests {
    use super::*;

    #[test]
    fn equipment_arm_ik_requires_authored_rig_even_for_humanoid_topology() {
        use newengine_model_skeleton_api::{ModelSkeletonAnchors, ModelSkeletonJointMetadata};

        let names = [
            "root",
            "spined",
            "r_shoulder",
            "r_elbow",
            "r_wrist",
            "r_palm",
            "l_shoulder",
            "l_elbow",
            "l_wrist",
            "l_palm",
        ];
        let parents = [
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(1),
            Some(6),
            Some(7),
            Some(8),
        ];
        let joints = names
            .iter()
            .enumerate()
            .map(|(index, name)| ModelSkeletonJointMetadata {
                index: index as u32,
                tag: index as u32,
                name: (*name).to_owned(),
                parent: parents[index].map(|parent| names[parent].to_owned()),
                parent_index: parents[index].map(|parent| parent as u32),
                position_ls: [0.0, 0.1, 0.0],
                rotation_ls: [0.0, 0.0, 0.0, 1.0],
                scale_ls: [1.0, 1.0, 1.0],
                flags: Vec::new(),
            })
            .collect();
        let skeleton = ModelSkeletonMetadata {
            source: "authored-only-ik-test".to_owned(),
            source_format: "test".to_owned(),
            container_magic: "TEST".to_owned(),
            byte_len: 0,
            content_hash: String::new(),
            decode_status: "ok".to_owned(),
            joints,
            anchors: ModelSkeletonAnchors {
                root: "root".to_owned(),
                hips: "root".to_owned(),
                head: "spined".to_owned(),
                left_hand: "l_palm".to_owned(),
                right_hand: "r_palm".to_owned(),
                left_foot: "root".to_owned(),
                right_foot: "root".to_owned(),
                eye: "spined".to_owned(),
                eye_height: 1.6,
            },
        };
        let mut presentation =
            newengine_engine_runtime::gameplay::PlayerCharacterPresentation::default();
        presentation.equipment_arm_ik = true;
        presentation.equipment_arm_ik_rig = None;

        assert!(resolve_authored_equipment_arm_ik(&skeleton, &presentation).is_none());
    }

    #[test]
    fn bilateral_rifle_ik_drives_both_hands_toward_readyhold_contacts() {
        use newengine_model_skeleton_api::{ModelSkeletonAnchors, ModelSkeletonJointMetadata};

        let names = [
            "root",
            "spined",
            "r_shoulder",
            "r_elbow",
            "r_wrist",
            "r_palm",
            "l_shoulder",
            "l_elbow",
            "l_wrist",
            "l_palm",
        ];
        let joint = |index: u32, parent_index: Option<u32>, position_ls: [f32; 3]| {
            ModelSkeletonJointMetadata {
                index,
                tag: index,
                name: names[index as usize].to_owned(),
                parent: parent_index.map(|parent| names[parent as usize].to_owned()),
                parent_index,
                position_ls,
                rotation_ls: [0.0, 0.0, 0.0, 1.0],
                scale_ls: [1.0, 1.0, 1.0],
                flags: Vec::new(),
            }
        };
        let skeleton = ModelSkeletonMetadata {
            source: "test".to_owned(),
            source_format: "test".to_owned(),
            container_magic: "TEST".to_owned(),
            byte_len: 0,
            content_hash: String::new(),
            decode_status: "ok".to_owned(),
            joints: vec![
                joint(0, None, [0.0, 0.0, 0.0]),
                joint(1, Some(0), [0.0, 1.285_745, 0.0]),
                joint(2, Some(1), [-0.17, 0.06, 0.0]),
                joint(3, Some(2), [0.0, -0.26, 0.0]),
                joint(4, Some(3), [0.0, -0.24, 0.0]),
                joint(5, Some(4), [0.0, -0.015, 0.0]),
                joint(6, Some(1), [0.17, 0.06, 0.0]),
                joint(7, Some(6), [0.0, -0.26, 0.0]),
                joint(8, Some(7), [0.0, -0.24, 0.0]),
                joint(9, Some(8), [0.0, -0.015, 0.0]),
            ],
            anchors: ModelSkeletonAnchors {
                root: "root".to_owned(),
                hips: "root".to_owned(),
                head: "spined".to_owned(),
                left_hand: "l_palm".to_owned(),
                right_hand: "r_palm".to_owned(),
                left_foot: "root".to_owned(),
                right_foot: "root".to_owned(),
                eye: "spined".to_owned(),
                eye_height: 0.0,
            },
        };
        let mut pose = skeleton
            .joints
            .iter()
            .map(|joint| JointLocalPose {
                translation: joint.position_ls,
                rotation: joint.rotation_ls,
                scale: Some(joint.scale_ls),
            })
            .collect::<Vec<_>>();
        let authored_rig = newengine_engine_runtime::gameplay::PlayerWeaponArmIkRigDefinition {
            chest: "spined".to_owned(),
            right_shoulder: "r_shoulder".to_owned(),
            right_elbow: "r_elbow".to_owned(),
            right_wrist: "r_wrist".to_owned(),
            right_palm: "r_palm".to_owned(),
            right_prop_attachment: None,
            left_shoulder: "l_shoulder".to_owned(),
            left_elbow: "l_elbow".to_owned(),
            left_wrist: "l_wrist".to_owned(),
            left_palm: "l_palm".to_owned(),
            left_prop_attachment: None,
        };
        let rig = build_weapon_arm_ik_rig(&skeleton, &authored_rig).expect("rifle IK rig");
        let source_to_model = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let presentation = newengine_engine_runtime::gameplay::WeaponPresentationDefinition {
            enabled: true,
            handle_from_root: [0.0, 0.014, -0.030],
            left_grip_from_handle: [-0.021, 0.043, 0.306],
            stock_contact_from_handle: [-0.020, 0.053, -0.341],
            ready_body_to_root_rotation: [0.036, 0.608, -0.041, 0.792],
            ready_left_palm_to_left_grip: [0.003, 0.101, 0.006],
            ready_right_palm_to_weapon: [-0.656, 0.722, 0.174, 0.133],
            ready_left_palm_to_weapon: [-0.023, -0.459, -0.303, 0.835],
            right_palm_to_handle: [0.019, 0.033, -0.083],
            ..Default::default()
        }
        .sanitized();
        let animation_runtime = AnimationSkeletonRuntime::compile(&skeleton, source_to_model)
            .expect("compile animation skeleton");
        let mut frames = Vec::new();
        rebuild_model_joint_frames(&animation_runtime, &pose, &mut frames).expect("initial frames");
        let right_before = frames[rig.right_palm].transform_point3(Vec3::ZERO);
        let left_before = frames[rig.left_palm].transform_point3(Vec3::ZERO);

        let final_result = apply_equipped_weapon_support_ik(
            &presentation,
            Some(&rig),
            &skeleton,
            &animation_runtime,
            &mut pose,
            &mut frames,
            None,
            None,
            None,
            false,
            0.0,
            0.0,
            0.0,
            0.0,
            Vec3::ZERO,
            true,
            true,
            true,
        )
        .expect("authored rifle support IK")
        .expect("IK enabled");

        let right_after = frames[rig.right_palm].transform_point3(Vec3::ZERO);
        let left_after = frames[rig.left_palm].transform_point3(Vec3::ZERO);
        let right_target = crate::weapon_grip::weapon_ready_right_palm_position(
            &presentation,
            final_result.base_root,
        );
        let left_target = crate::weapon_grip::weapon_ready_left_palm_position(
            &presentation,
            final_result.base_root,
        );
        let right_before_error = (right_before - right_target).length();
        let left_before_error = (left_before - left_target).length();
        let right_after_error = (right_after - right_target).length();
        let left_after_error = (left_after - left_target).length();
        assert!(right_after_error < right_before_error);
        assert!(left_after_error < left_before_error);
        assert!(final_result.error_m.is_finite());
    }

    #[test]
    fn partial_equipment_overlay_never_replaces_missing_joint_with_bind_pose() {
        use newengine_model_skeleton_api::{ModelSkeletonAnchors, ModelSkeletonJointMetadata};

        let joint =
            |index: u32, name: &str, parent_index: Option<u32>| ModelSkeletonJointMetadata {
                index,
                tag: index,
                name: name.to_owned(),
                parent: parent_index.map(|_| "root".to_owned()),
                parent_index,
                position_ls: if index == 0 {
                    [0.0, 0.0, 0.0]
                } else {
                    [0.2, 0.0, 0.0]
                },
                rotation_ls: [0.0, 0.0, 0.0, 1.0],
                scale_ls: [1.0, 1.0, 1.0],
                flags: Vec::new(),
            };
        let skeleton = ModelSkeletonMetadata {
            source: "test".to_owned(),
            source_format: "test".to_owned(),
            container_magic: "TEST".to_owned(),
            byte_len: 0,
            content_hash: String::new(),
            decode_status: "ok".to_owned(),
            joints: vec![joint(0, "root", None), joint(1, "arm", Some(0))],
            anchors: ModelSkeletonAnchors {
                root: "root".to_owned(),
                hips: "root".to_owned(),
                head: "arm".to_owned(),
                left_hand: "arm".to_owned(),
                right_hand: "arm".to_owned(),
                left_foot: "root".to_owned(),
                right_foot: "root".to_owned(),
                eye: "arm".to_owned(),
                eye_height: 0.0,
            },
        };
        let live_rotation = Quat::from_rotation_y(0.75);
        let mut target = vec![
            JointLocalPose {
                translation: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: Some([1.0, 1.0, 1.0]),
            },
            JointLocalPose {
                translation: [0.2, 0.0, 0.0],
                rotation: [
                    live_rotation.x,
                    live_rotation.y,
                    live_rotation.z,
                    live_rotation.w,
                ],
                scale: Some([1.0, 1.0, 1.0]),
            },
        ];
        let raw_clip = AnimationClip {
            name: "partial".to_owned(),
            skeleton_ref: "test".to_owned(),
            source: "test".to_owned(),
            duration_seconds: 1.0 / 30.0,
            sample_rate_hz: 30.0,
            looped: false,
            joint_tags: vec![0],
            events: Vec::new(),
            poses: vec![JointLocalPose {
                translation: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: Some([1.0, 1.0, 1.0]),
            }],
        };
        let animation_runtime =
            AnimationSkeletonRuntime::compile(&skeleton, Mat4::IDENTITY.to_cols_array())
                .expect("compile animation skeleton");
        let binding = raw_clip
            .bind_to_skeleton(&animation_runtime)
            .expect("bind partial clip");
        let clip = PlayerAnimationRuntimeClip {
            clip_ref: "test@partial".to_owned(),
            clip: raw_clip.into(),
            binding,
            event_cursor: AnimationEventCursor::default(),
        };
        let before = target[1];
        let mut scratch = Vec::new();
        let rules = resolve_joint_blend_rules(
            &skeleton,
            &[
                newengine_engine_runtime::gameplay::PlayerJointRotationWeight {
                    joint: "arm".to_owned(),
                    weight: 1.0,
                    channels:
                        newengine_engine_runtime::gameplay::PlayerJointChannels::rotation_only(),
                },
            ],
        )
        .expect("resolve overlay rule");
        apply_equipment_rotation_overlay(
            Some(&clip),
            &animation_runtime,
            &mut scratch,
            &mut target,
            0.0,
            &rules,
            1.0,
        )
        .unwrap();
        assert_eq!(target[1], before);
    }

    #[test]
    fn weapon_reach_fit_corrects_small_mismatch_but_never_masks_large_pose_error() {
        let pose = vec![
            JointLocalPose {
                translation: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: Some([1.0, 1.0, 1.0])
            };
            4
        ];
        let frames = vec![
            Mat4::from_translation(Vec3::new(0.0, 0.0, 0.0)),
            Mat4::from_translation(Vec3::new(0.0, 0.0, 0.25)),
            Mat4::from_translation(Vec3::new(0.0, 0.0, 0.50)),
            Mat4::from_translation(Vec3::new(0.0, 0.0, 0.51)),
        ];
        let small = arm_reach_fit_correction(
            &pose,
            &frames,
            0,
            1,
            2,
            3,
            Vec3::new(0.0, 0.0, 0.52),
            Quat::IDENTITY,
        );
        assert!(small.length() > 0.015 && small.length() < 0.017);
        assert!(small.z < 0.0);

        let large = arm_reach_fit_correction(
            &pose,
            &frames,
            0,
            1,
            2,
            3,
            Vec3::new(0.0, 0.0, 0.62),
            Quat::IDENTITY,
        );
        assert!(large.length() < 1.0e-6);
    }

    #[test]
    fn detached_control_and_face_share_the_same_canonical_headb_delta() {
        let rig = DetachedHeadFollowRig {
            driver_joint: 0,
            followers: vec![1, 2],
        };
        let mut palette = vec![Mat4::IDENTITY; 3];
        palette[0] = Mat4::from_translation(Vec3::new(0.2, 0.1, -0.3));
        palette[1] = Mat4::from_translation(Vec3::new(0.0, 0.02, 0.0));
        palette[2] = Mat4::from_translation(Vec3::new(0.0, 0.03, 0.0));

        apply_detached_head_follow_palette(Some(&rig), &mut palette).expect("projection");

        let control = palette[1].transform_point3(Vec3::ZERO);
        assert!((control.x - 0.2).abs() < 1.0e-5);
        assert!((control.y - 0.12).abs() < 1.0e-5);
        assert!((control.z + 0.3).abs() < 1.0e-5);

        // The face gets headb + its own detached deformation only. It must not
        // receive the MCH control deformation a second time (old result y=0.15).
        let face = palette[2].transform_point3(Vec3::ZERO);
        assert!((face.x - 0.2).abs() < 1.0e-5);
        assert!((face.y - 0.13).abs() < 1.0e-5);
        assert!((face.z + 0.3).abs() < 1.0e-5);
    }

    #[test]
    fn native_abby_eye_palette_enforces_parent_deformation_invariant() {
        let contract = EyeRuntimeContract {
            parent: 0,
            left: 1,
            right: 2,
            preserve_bind_local: true,
        };
        let head_delta = Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 1.0, 1.0),
            Quat::from_rotation_y(0.25),
            Vec3::new(0.2, 0.1, -0.3),
        );
        let mut palette = vec![head_delta, head_delta, head_delta];
        validate_eye_palette(Some(&contract), &palette).expect("stable eyes");

        palette[contract.left] = Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 1.0, 1.0),
            Quat::from_rotation_x(0.08),
            Vec3::ZERO,
        ) * palette[contract.left];
        let error = validate_eye_palette(Some(&contract), &palette)
            .expect_err("extra eye deformation must be rejected");
        assert!(error.contains("eye palette drift"));
    }
}
