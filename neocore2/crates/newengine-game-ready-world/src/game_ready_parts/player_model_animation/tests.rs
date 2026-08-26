#[cfg(test)]
mod transition_tests {
    use super::*;

    #[test]
    fn rifle_ready_pole_ik_converges_without_moving_stock_anchored_weapon() {
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
                // Real Abby arm lengths are roughly 0.26 m upper arm and 0.25 m forearm/hand.
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
        let rig = build_weapon_arm_ik_rig(&skeleton).expect("rifle IK rig");
        let source_to_model = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let mut frames = Vec::new();
        rebuild_model_joint_frames(&skeleton, source_to_model, &pose, &mut frames)
            .expect("initial frames");
        let contract_before = crate::weapon_grip::rifle_ready_solve_contract(
            frames[rig.chest],
            frames[rig.right_shoulder],
            frames[rig.left_shoulder],
        )
        .expect("ReadyHold solve contract");
        let root_before = contract_before.root;
        let right_target = crate::weapon_grip::weapon_ready_right_palm_position(root_before);
        let left_target = crate::weapon_grip::weapon_ready_left_palm_position(root_before);
        let initial_error = (
            (frames[rig.right_palm].transform_point3(Vec3::ZERO) - right_target).length(),
            (frames[rig.left_palm].transform_point3(Vec3::ZERO) - left_target).length(),
        );

        let final_error = apply_equipped_rifle_support_ik(
            Some(&rig),
            &skeleton,
            source_to_model,
            &mut pose,
            &mut frames,
            None,
            0.0,
            0.0,
            true,
        )
        .expect("bilateral ReadyHold IK")
        .expect("IK enabled");

        let final_right =
            (frames[rig.right_palm].transform_point3(Vec3::ZERO) - right_target).length();
        let final_left =
            (frames[rig.left_palm].transform_point3(Vec3::ZERO) - left_target).length();
        assert!(
            final_right < initial_error.0,
            "right initial={} final={final_right}",
            initial_error.0
        );
        assert!(
            final_left < initial_error.1,
            "left initial={} final={final_left}",
            initial_error.1
        );
        assert!(final_error < 0.035, "final={final_error}");

        let contract_after = crate::weapon_grip::rifle_ready_solve_contract(
            frames[rig.chest],
            frames[rig.right_shoulder],
            frames[rig.left_shoulder],
        )
        .expect("ReadyHold solve contract after IK");
        let root_after = contract_after.root;
        assert!((root_before.position - root_after.position).length() < 1.0e-6);
        assert!(root_before.rotation.dot(root_after.rotation).abs() > 0.999_999);
        assert!((contract_after.stock_contact - contract_after.shoulder_pocket).length() < 1.0e-6);
    }

    #[test]
    fn detached_control_and_face_share_the_same_canonical_headb_delta() {
        let rig = DetachedHeadFollowRig {
            headb_driver: 0,
            control_followers: vec![1],
            face_followers: vec![2],
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

    #[test]
    fn local_pose_crossfade_preserves_endpoints_and_shortest_quaternion_path() {
        let from = [JointLocalPose {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: Some([1.0, 1.0, 1.0]),
        }];
        let to = [JointLocalPose {
            translation: [2.0, 4.0, 6.0],
            // Same identity rotation with opposite quaternion sign.
            rotation: [0.0, 0.0, 0.0, -1.0],
            scale: Some([1.0, 1.0, 1.0]),
        }];
        let mut out = Vec::new();
        blend_local_poses(&from, &to, 0.5, &mut out).expect("blend");
        assert_eq!(out.len(), 1);
        assert!((out[0].translation[0] - 1.0).abs() <= 1.0e-6);
        assert!((out[0].translation[1] - 2.0).abs() <= 1.0e-6);
        assert!((out[0].translation[2] - 3.0).abs() <= 1.0e-6);
        assert!(out[0].rotation[0].abs() <= 1.0e-6);
        assert!(out[0].rotation[1].abs() <= 1.0e-6);
        assert!(out[0].rotation[2].abs() <= 1.0e-6);
        assert!((out[0].rotation[3].abs() - 1.0).abs() <= 1.0e-6);
    }
}
