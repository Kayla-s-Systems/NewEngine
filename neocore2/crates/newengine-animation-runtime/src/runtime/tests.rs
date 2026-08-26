#[cfg(test)]
mod tests {
    use super::*;
    use newengine_model_skeleton_api::{ModelSkeletonAnchors, ModelSkeletonJointMetadata};

    fn test_body() -> Vec<u8> {
        let strings = b"idle\0skeleton.ymt@body\0source.ycd\0";
        let table_offset = YCD_BODY_HEADER_LEN;
        let string_offset = table_offset + YCD_CLIP_RECORD_LEN;
        let payload_offset = string_offset + strings.len();
        let joint_count = 1u32;
        let frame_count = 2u32;
        let payload_len = 4 + 2 * LOCAL_POSE_STRIDE_V2;
        let mut out = Vec::new();
        out.extend_from_slice(&YCD_BODY_SCHEMA_VERSION.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        for value in [
            table_offset as u64,
            string_offset as u64,
            strings.len() as u64,
            payload_offset as u64,
            0,
        ] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&1u64.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&5u32.to_le_bytes());
        out.extend_from_slice(&joint_count.to_le_bytes());
        out.extend_from_slice(&frame_count.to_le_bytes());
        out.extend_from_slice(&1.0f32.to_le_bytes());
        out.extend_from_slice(&2.0f32.to_le_bytes());
        out.extend_from_slice(&YCD_CLIP_FLAG_LOOP.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(payload_offset as u64).to_le_bytes());
        out.extend_from_slice(&(payload_len as u64).to_le_bytes());
        out.extend_from_slice(&23u64.to_le_bytes());
        out.extend_from_slice(strings);
        out.extend_from_slice(&42u32.to_le_bytes());
        for (translation, rotation) in [
            ([0.0f32, 0.0, 0.0], [0.0f32, 0.0, 0.0, 1.0]),
            ([1.0f32, 0.0, 0.0], [0.0f32, 0.0, 0.0, 1.0]),
        ] {
            for value in translation
                .into_iter()
                .chain(rotation)
                .chain([1.0f32, 1.0, 1.0])
            {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        out
    }

    fn one_joint_skeleton() -> ModelSkeletonMetadata {
        ModelSkeletonMetadata {
            source: "skeleton.ymt".to_owned(),
            source_format: "test".to_owned(),
            container_magic: "NEF8".to_owned(),
            byte_len: 0,
            content_hash: "test".to_owned(),
            decode_status: "test".to_owned(),
            joints: vec![ModelSkeletonJointMetadata {
                index: 0,
                tag: 42,
                name: "root".to_owned(),
                parent: None,
                parent_index: None,
                position_ls: [0.0, 0.0, 0.0],
                rotation_ls: [0.0, 0.0, 0.0, 1.0],
                scale_ls: [1.0, 1.0, 1.0],
                flags: Vec::new(),
            }],
            anchors: ModelSkeletonAnchors {
                root: "root".to_owned(),
                hips: "root".to_owned(),
                head: "root".to_owned(),
                left_hand: "root".to_owned(),
                right_hand: "root".to_owned(),
                left_foot: "root".to_owned(),
                right_foot: "root".to_owned(),
                eye: "root".to_owned(),
                eye_height: 1.0,
            },
        }
    }

    fn two_joint_skeleton() -> ModelSkeletonMetadata {
        let mut skeleton = one_joint_skeleton();
        skeleton.joints[0].tag = 10;
        skeleton.joints.push(ModelSkeletonJointMetadata {
            index: 1,
            tag: 20,
            name: "child".to_owned(),
            parent: Some("root".to_owned()),
            parent_index: Some(0),
            position_ls: [0.0, 1.0, 0.0],
            rotation_ls: [0.0, 0.0, 0.0, 1.0],
            scale_ls: [1.0, 1.0, 1.0],
            flags: Vec::new(),
        });
        skeleton
    }

    #[test]
    fn sparse_clip_overlays_native_tags_on_bind_pose() {
        let clip = AnimationClip {
            name: "native-sparse".to_owned(),
            skeleton_ref: "skeleton.ymt@body".to_owned(),
            source: "northstar.tlou2.pc://test".to_owned(),
            duration_seconds: 1.0,
            sample_rate_hz: 2.0,
            looped: true,
            joint_tags: vec![20],
            poses: vec![
                JointLocalPose {
                    translation: [0.0, 1.0, 0.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: Some([1.0, 1.0, 1.0]),
                },
                JointLocalPose {
                    translation: [2.0, 1.0, 0.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: Some([1.0, 1.0, 1.0]),
                },
            ],
        };
        let skeleton = two_joint_skeleton();
        let mut sampled = Vec::new();
        clip.sample_local_pose_for_skeleton(0.25, &skeleton, &mut sampled)
            .expect("sparse sample");
        assert_eq!(sampled.len(), 2);
        assert_eq!(sampled[0].translation, [0.0, 0.0, 0.0]);
        assert!((sampled[1].translation[0] - 1.0).abs() < 1.0e-6);
        assert!((sampled[1].translation[1] - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn decodes_and_interpolates_canonical_ycd() {
        let clip = decode_ycd_body(&test_body(), Some("idle")).expect("decode");
        assert_eq!(clip.joint_tags, vec![42]);
        assert_eq!(clip.frame_count(), 2);
        let mut sampled = Vec::new();
        clip.sample_local_pose(0.25, &mut sampled).expect("sample");
        assert!((sampled[0].translation[0] - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn palette_conjugates_source_motion_into_model_space() {
        let clip = decode_ycd_body(&test_body(), None).expect("decode");
        let source_to_model = [
            2.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0, 0.0, 1.0,
        ];
        let mut sampled = Vec::new();
        let mut palette = Vec::new();
        build_skin_palette(
            &clip,
            &one_joint_skeleton(),
            source_to_model,
            0.25,
            &mut sampled,
            &mut palette,
        )
        .expect("palette");
        let moved = palette[0].transform_point3(Vec3::ZERO);
        // Source +0.5 X is scaled by source_to_model to +1.0 model-space X.
        assert!((moved.x - 1.0).abs() < 1.0e-5, "moved={moved:?}");
        assert!(moved.y.abs() < 1.0e-5);
        assert!(moved.z.abs() < 1.0e-5);
    }
    #[test]
    fn model_joint_frames_preserve_absolute_animated_pose_in_baked_space() {
        let source_to_model = [
            2.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0, 0.0, 1.0,
        ];
        let pose = [JointLocalPose {
            translation: [0.5, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: Some([1.0, 1.0, 1.0]),
        }];
        let mut frames = Vec::new();
        build_model_joint_frames_from_local_pose(
            &one_joint_skeleton(),
            source_to_model,
            &pose,
            &mut frames,
        )
        .expect("joint frames");
        assert_eq!(frames.len(), 1);
        let origin = frames[0].transform_point3(Vec3::ZERO);
        assert!((origin.x - 1.0).abs() < 1.0e-5, "origin={origin:?}");
        assert!((origin.y - 3.0).abs() < 1.0e-5, "origin={origin:?}");
        assert!(origin.z.abs() < 1.0e-5, "origin={origin:?}");
    }

    #[test]
    fn bind_pose_palette_is_validated_identity() {
        let source_to_model = [
            2.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0, 0.0, 1.0,
        ];
        let mut palette = Vec::new();
        build_bind_pose_palette(&one_joint_skeleton(), source_to_model, &mut palette)
            .expect("bind palette");
        assert_eq!(palette.len(), 1);
        let actual = palette[0].to_cols_array();
        let expected = Mat4::IDENTITY.to_cols_array();
        assert!(actual
            .iter()
            .zip(expected.iter())
            .all(|(a, b)| (a - b).abs() < 1.0e-5));
    }
}
