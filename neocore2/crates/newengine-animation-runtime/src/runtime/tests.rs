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

    fn two_clip_test_body() -> Vec<u8> {
        fn push_string(strings: &mut Vec<u8>, value: &str) -> u32 {
            let offset = strings.len() as u32;
            strings.extend_from_slice(value.as_bytes());
            strings.push(0);
            offset
        }

        let mut strings = Vec::new();
        let idle_name = push_string(&mut strings, "idle");
        let walk_name = push_string(&mut strings, "walk");
        let skeleton_ref = push_string(&mut strings, "skeleton.ymt@body");
        let source_ref = push_string(&mut strings, "source.ycd");
        let clip_count = 2usize;
        let table_offset = YCD_BODY_HEADER_LEN;
        let string_offset = table_offset + clip_count * YCD_CLIP_RECORD_LEN;
        let payload_len = 4 + LOCAL_POSE_STRIDE_V2;
        let payload_floor = string_offset + strings.len();
        let payload_offsets = [payload_floor, payload_floor + payload_len];

        let mut out = Vec::new();
        out.extend_from_slice(&YCD_BODY_SCHEMA_VERSION.to_le_bytes());
        out.extend_from_slice(&(clip_count as u32).to_le_bytes());
        for value in [
            table_offset as u64,
            string_offset as u64,
            strings.len() as u64,
            payload_floor as u64,
            0,
        ] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for (index, (name_offset, payload_offset)) in [idle_name, walk_name]
            .into_iter()
            .zip(payload_offsets)
            .enumerate()
        {
            out.extend_from_slice(&((index + 1) as u64).to_le_bytes());
            out.extend_from_slice(&name_offset.to_le_bytes());
            out.extend_from_slice(&skeleton_ref.to_le_bytes());
            out.extend_from_slice(&1u32.to_le_bytes());
            out.extend_from_slice(&1u32.to_le_bytes());
            out.extend_from_slice(&1.0f32.to_le_bytes());
            out.extend_from_slice(&30.0f32.to_le_bytes());
            out.extend_from_slice(&YCD_CLIP_FLAG_LOOP.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&(payload_offset as u64).to_le_bytes());
            out.extend_from_slice(&(payload_len as u64).to_le_bytes());
            out.extend_from_slice(&(source_ref as u64).to_le_bytes());
        }
        out.extend_from_slice(&strings);
        for translation_x in [0.0f32, 1.0] {
            out.extend_from_slice(&42u32.to_le_bytes());
            for value in [translation_x, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0] {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        out
    }

    fn two_clip_body_with_invalid_walk_quaternion() -> Vec<u8> {
        let mut body = two_clip_test_body();
        let payload_floor =
            u64::from_le_bytes(body[32..40].try_into().expect("payload floor")) as usize;
        let payload_len = 4 + LOCAL_POSE_STRIDE_V2;
        let walk_payload = payload_floor + payload_len;
        // payload = joint tag + translation.xyz + rotation.xyzw + scale.xyz
        let rotation_offset = walk_payload + 4 + 12;
        body[rotation_offset..rotation_offset + 16].fill(0);
        body
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

    fn branched_skeleton() -> ModelSkeletonMetadata {
        let mut skeleton = two_joint_skeleton();
        skeleton.joints[1].name = "arm".to_owned();
        skeleton.joints.push(ModelSkeletonJointMetadata {
            index: 2,
            tag: 30,
            name: "hand".to_owned(),
            parent: Some("arm".to_owned()),
            parent_index: Some(1),
            position_ls: [0.0, 1.0, 0.0],
            rotation_ls: [0.0, 0.0, 0.0, 1.0],
            scale_ls: [1.0, 1.0, 1.0],
            flags: Vec::new(),
        });
        skeleton.joints.push(ModelSkeletonJointMetadata {
            index: 3,
            tag: 40,
            name: "sibling".to_owned(),
            parent: Some("root".to_owned()),
            parent_index: Some(0),
            position_ls: [1.0, 0.0, 0.0],
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
            source: "northstar.pc://test".to_owned(),
            duration_seconds: 1.0,
            sample_rate_hz: 2.0,
            looped: true,
            joint_tags: vec![20],
            events: Vec::new(),
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
    fn selected_entry_lookup_skips_malformed_unrelated_name() {
        let mut body = two_clip_test_body();
        let string_offset =
            u64::from_le_bytes(body[16..24].try_into().expect("string offset")) as usize;
        body[string_offset] = 0xff;

        let strict_error = decode_ycd_dictionary(&body).expect_err("strict dictionary must fail");
        assert!(strict_error.contains("not UTF-8"));
        let walk = decode_ycd_body(&body, Some("walk")).expect("unrelated valid selector");
        assert_eq!(walk.name, "walk");
    }

    #[test]
    fn selected_entry_decode_isolated_from_invalid_neighbor_clip() {
        let body = two_clip_body_with_invalid_walk_quaternion();
        let strict_error = decode_ycd_dictionary(&body).expect_err("strict dictionary must fail");
        assert!(strict_error.contains("walk"));
        assert!(strict_error.contains("invalid quaternion"));

        let idle = decode_ycd_body(&body, Some("idle")).expect("selected valid clip");
        assert_eq!(idle.name, "idle");
        let walk_error = decode_ycd_body(&body, Some("walk")).expect_err("selected invalid clip");
        assert!(walk_error.contains("walk"));
        assert!(walk_error.contains("invalid quaternion"));
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
    fn incremental_subtree_joint_frame_refresh_matches_full_fk_and_preserves_siblings() {
        let skeleton = branched_skeleton();
        let source_to_model = Mat4::from_quat(Quat::from_rotation_y(0.37)).to_cols_array();
        let runtime = AnimationSkeletonRuntime::compile(&skeleton, source_to_model)
            .expect("compile branched skeleton");
        let mut pose = runtime.bind_locals().to_vec();
        let mut incremental = Vec::new();
        runtime
            .build_model_joint_frames_from_local_pose(&pose, &mut incremental)
            .expect("initial frames");
        let sibling_before = incremental[3];

        let arm_rotation = Quat::from_rotation_z(0.42);
        pose[1].rotation = [
            arm_rotation.x,
            arm_rotation.y,
            arm_rotation.z,
            arm_rotation.w,
        ];
        runtime
            .refresh_model_joint_frames_subtree_from_local_pose(&pose, &mut incremental, 1)
            .expect("incremental arm refresh");

        let mut full = Vec::new();
        runtime
            .build_model_joint_frames_from_local_pose(&pose, &mut full)
            .expect("full frames");
        assert_eq!(incremental.len(), full.len());
        for (joint, (actual, expected)) in incremental.iter().zip(&full).enumerate() {
            let max_error = actual
                .to_cols_array()
                .into_iter()
                .zip(expected.to_cols_array())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f32, f32::max);
            assert!(max_error <= 1.0e-6, "joint={joint} error={max_error}");
        }
        let sibling_error = incremental[3]
            .to_cols_array()
            .into_iter()
            .zip(sibling_before.to_cols_array())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            sibling_error <= 1.0e-7,
            "sibling changed error={sibling_error}"
        );
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

    #[test]
    fn compiled_binding_matches_legacy_sparse_sampling_and_palette() {
        let clip = AnimationClip {
            name: "compiled-sparse".to_owned(),
            skeleton_ref: "skeleton.ymt@body".to_owned(),
            source: "test".to_owned(),
            duration_seconds: 1.0,
            sample_rate_hz: 2.0,
            looped: true,
            joint_tags: vec![20],
            events: Vec::new(),
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
        let source_to_model = Mat4::IDENTITY.to_cols_array();
        let runtime = AnimationSkeletonRuntime::compile(&skeleton, source_to_model)
            .expect("compile animation skeleton");
        let binding = clip
            .bind_to_skeleton(&runtime)
            .expect("bind clip to animation skeleton");

        let mut legacy_pose = Vec::new();
        clip.sample_local_pose_for_skeleton(0.25, &skeleton, &mut legacy_pose)
            .expect("legacy sample");
        let mut compiled_pose = Vec::new();
        clip.sample_local_pose_bound(0.25, &runtime, &binding, &mut compiled_pose)
            .expect("compiled sample");
        assert_eq!(compiled_pose, legacy_pose);

        let mut legacy_palette = Vec::new();
        build_skin_palette_from_local_pose(
            &skeleton,
            source_to_model,
            &legacy_pose,
            &mut legacy_palette,
        )
        .expect("legacy palette");
        let mut compiled_palette = Vec::new();
        runtime
            .build_skin_palette_from_local_pose(&compiled_pose, &mut compiled_palette)
            .expect("compiled palette");
        assert_eq!(compiled_palette.len(), legacy_palette.len());
        for (compiled, legacy) in compiled_palette.iter().zip(legacy_palette.iter()) {
            assert!(compiled
                .to_cols_array()
                .iter()
                .zip(legacy.to_cols_array().iter())
                .all(|(a, b)| (a - b).abs() < 1.0e-5));
        }
    }

    #[test]
    fn compiled_runtime_preserves_finite_zero_scale_as_authored_visibility() {
        let runtime = AnimationSkeletonRuntime::compile(
            &one_joint_skeleton(),
            Mat4::IDENTITY.to_cols_array(),
        )
        .expect("compile animation skeleton");
        let pose = [JointLocalPose {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: Some([0.0, 0.0, 0.0]),
        }];

        let mut palette = Vec::new();
        runtime
            .build_skin_palette_from_local_pose(&pose, &mut palette)
            .expect("finite zero-scale visibility pose must remain valid");
        assert_eq!(palette.len(), 1);
        assert!(palette[0]
            .to_cols_array()
            .iter()
            .all(|value| value.is_finite()));
        let collapsed = palette[0].transform_point3(Vec3::new(1.0, 2.0, 3.0));
        assert!(
            collapsed.length_squared() <= 1.0e-10,
            "collapsed={collapsed:?}"
        );

        let mut frames = Vec::new();
        runtime
            .build_model_joint_frames_from_local_pose(&pose, &mut frames)
            .expect("zero-scale authored joint frame must remain finite");
        assert!(frames[0]
            .to_cols_array()
            .iter()
            .all(|value| value.is_finite()));
    }

    #[test]
    fn compiled_runtime_still_rejects_singular_bind_scale() {
        let mut skeleton = one_joint_skeleton();
        skeleton.joints[0].scale_ls = [0.0, 0.0, 0.0];
        let error = AnimationSkeletonRuntime::compile(&skeleton, Mat4::IDENTITY.to_cols_array())
            .expect_err("bind pose must remain invertible");
        assert!(error.contains("bind scale") && error.contains("singular"));
    }

    #[test]
    fn compiled_runtime_rejects_cyclic_skeleton_before_frame_evaluation() {
        let mut skeleton = two_joint_skeleton();
        skeleton.joints[0].parent = Some("child".to_owned());
        skeleton.joints[0].parent_index = Some(1);
        let error = AnimationSkeletonRuntime::compile(&skeleton, Mat4::IDENTITY.to_cols_array())
            .expect_err("cyclic skeleton must be rejected");
        assert!(error.contains("cycle") || error.contains("unresolvable"));
    }

    #[test]
    fn clip_binding_rejects_invalid_pose_data_at_prepare_time() {
        let clip = AnimationClip {
            name: "invalid-rotation".to_owned(),
            skeleton_ref: "skeleton.ymt@body".to_owned(),
            source: "test".to_owned(),
            duration_seconds: 1.0,
            sample_rate_hz: 30.0,
            looped: false,
            joint_tags: vec![42],
            events: Vec::new(),
            poses: vec![JointLocalPose {
                translation: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 0.0],
                scale: Some([1.0, 1.0, 1.0]),
            }],
        };
        let runtime = AnimationSkeletonRuntime::compile(
            &one_joint_skeleton(),
            Mat4::IDENTITY.to_cols_array(),
        )
        .expect("compile animation skeleton");
        let error = clip
            .bind_to_skeleton(&runtime)
            .expect_err("invalid clip must be rejected before playback");
        assert!(error.contains("quaternion"));
    }

    #[test]
    fn looped_animation_events_cross_wrap_exactly_once() {
        let mut clip = decode_ycd_body(&test_body(), Some("idle")).expect("decode");
        clip.events = vec![
            AnimationEvent::new(0.25, "foot.left.contact"),
            AnimationEvent::new(0.75, "foot.right.contact"),
        ];
        clip.validate_events().expect("valid events");

        let mut cursor = AnimationEventCursor::default();
        cursor.seek(0.70).expect("seed cursor");
        let mut occurrences = Vec::new();
        assert_eq!(
            cursor
                .advance(&clip, 1.30, &mut occurrences)
                .expect("advance across wrap"),
            2
        );
        assert_eq!(occurrences.len(), 2);
        assert_eq!(
            clip.events[occurrences[0].event_index].tag,
            "foot.right.contact"
        );
        assert_eq!(occurrences[0].loop_index, 0);
        assert!((occurrences[0].playback_time_seconds - 0.75).abs() < 1.0e-6);
        assert_eq!(
            clip.events[occurrences[1].event_index].tag,
            "foot.left.contact"
        );
        assert_eq!(occurrences[1].loop_index, 1);
        assert!((occurrences[1].playback_time_seconds - 1.25).abs() < 1.0e-6);

        assert_eq!(
            cursor
                .advance(&clip, 1.30, &mut occurrences)
                .expect("same-time advance"),
            0
        );
        assert_eq!(
            occurrences.len(),
            2,
            "same boundary must never duplicate markers"
        );
    }

    #[test]
    fn loop_zero_event_fires_on_next_cycle_boundary_not_on_cursor_seed() {
        let mut clip = decode_ycd_body(&test_body(), Some("idle")).expect("decode");
        clip.events = vec![AnimationEvent::new(0.0, "cycle.begin")];
        let mut cursor = AnimationEventCursor::default();
        let mut occurrences = Vec::new();
        assert_eq!(cursor.advance(&clip, 0.0, &mut occurrences).unwrap(), 0);
        assert_eq!(cursor.advance(&clip, 0.9, &mut occurrences).unwrap(), 0);
        assert_eq!(cursor.advance(&clip, 1.0, &mut occurrences).unwrap(), 1);
        assert_eq!(occurrences[0].loop_index, 1);
        assert!((occurrences[0].playback_time_seconds - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn looped_event_at_duration_is_rejected_as_ambiguous() {
        let mut clip = decode_ycd_body(&test_body(), Some("idle")).expect("decode");
        clip.events = vec![AnimationEvent::new(clip.duration_seconds, "cycle.end")];
        let error = clip
            .validate_events()
            .expect_err("loop endpoint must be rejected");
        assert!(error.contains("outside clip duration"));
    }

    #[test]
    fn shared_dictionary_store_reuses_arc_and_supports_invalidation() {
        use std::cell::Cell;

        let store = AnimationClipStore::new();
        let loads = Cell::new(0usize);
        let body = test_body();
        let first = store
            .load_ycd_clip("Shared/Characters/Test.ycd@idle", |_| {
                loads.set(loads.get() + 1);
                Ok(body.clone())
            })
            .expect("first dictionary load");
        let second = store
            .load_ycd_clip("shared/characters/test.ycd@IDLE", |_| {
                loads.set(loads.get() + 1);
                Err("cache miss should not happen".to_owned())
            })
            .expect("cached clip load");
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        assert_eq!(loads.get(), 1);
        assert_eq!(
            store.stats().unwrap(),
            AnimationClipStoreStats {
                dictionaries: 1,
                clips: 1,
            }
        );

        assert!(store
            .invalidate_ycd_path("shared/characters/test.ycd")
            .expect("invalidate"));
        let third = store
            .load_ycd_clip("shared/characters/test.ycd@idle", |_| {
                loads.set(loads.get() + 1);
                Ok(body.clone())
            })
            .expect("reloaded clip");
        assert!(!std::sync::Arc::ptr_eq(&first, &third));
        assert_eq!(loads.get(), 2);
    }

    #[test]
    fn clip_store_falls_back_to_isolated_selected_entry_without_weakening_dictionary_validation() {
        use std::cell::Cell;

        let store = AnimationClipStore::new();
        let body = two_clip_body_with_invalid_walk_quaternion();
        let loads = Cell::new(0usize);
        let first = store
            .load_ycd_clip("shared/character/damaged.ycd@idle", |_| {
                loads.set(loads.get() + 1);
                Ok(body.clone())
            })
            .expect("valid selected entry");
        let second = store
            .load_ycd_clip("SHARED/CHARACTER/DAMAGED.YCD@IDLE", |_| {
                loads.set(loads.get() + 1);
                Err("isolated clip cache miss should not happen".to_owned())
            })
            .expect("cached isolated entry");
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        assert_eq!(loads.get(), 1);
        assert_eq!(
            store.stats().unwrap(),
            AnimationClipStoreStats {
                dictionaries: 0,
                clips: 1,
            }
        );

        let strict_error = store
            .load_ycd_dictionary("shared/character/damaged.ycd", |_| Ok(body.clone()))
            .expect_err("whole dictionary must remain invalid");
        assert!(strict_error.contains("walk"));
        assert!(strict_error.contains("invalid quaternion"));
    }

    #[test]
    fn restart_emits_authored_zero_marker() {
        let mut clip = decode_ycd_body(&test_body(), Some("idle")).expect("decode");
        clip.events = vec![AnimationEvent::new(0.0, "weapon.fire")];
        let mut cursor = AnimationEventCursor::default();
        cursor.restart();
        let mut occurrences = Vec::new();
        assert_eq!(cursor.advance(&clip, 0.0, &mut occurrences).unwrap(), 1);
        assert_eq!(occurrences[0].loop_index, 0);
        assert_eq!(occurrences[0].event_index, 0);
    }

    #[test]
    fn installing_event_track_creates_new_immutable_clip_revision() {
        let store = AnimationClipStore::new();
        let body = test_body();
        let original = store
            .load_ycd_clip("shared/character/test.ycd@idle", |_| Ok(body.clone()))
            .expect("load clip");
        assert!(original.events.is_empty());

        let revised = store
            .install_clip_events(
                "SHARED/CHARACTER/TEST.YCD@IDLE",
                vec![AnimationEvent::new(0.5, "foot.left.contact")
                    .with_parameter("surface_policy", "contact-authoritative")],
            )
            .expect("install event track");
        assert_eq!(revised.events.len(), 1);
        assert!(!std::sync::Arc::ptr_eq(&original, &revised));
        assert!(
            original.events.is_empty(),
            "bound old revision must remain immutable"
        );

        let future = store
            .load_ycd_clip("shared/character/test.ycd@idle", |_| {
                Err("cached revised dictionary should not reload".to_owned())
            })
            .expect("future lookup");
        assert!(std::sync::Arc::ptr_eq(&revised, &future));
        assert_eq!(future.events[0].tag, "foot.left.contact");
    }

    #[test]
    fn shared_dictionary_decodes_multiple_selectors_once() {
        use std::cell::Cell;

        let store = AnimationClipStore::new();
        let loads = Cell::new(0usize);
        let body = two_clip_test_body();
        let idle = store
            .load_ycd_clip("shared/character/two.ycd@idle", |_| {
                loads.set(loads.get() + 1);
                Ok(body.clone())
            })
            .expect("load idle");
        let walk = store
            .load_ycd_clip("shared/character/two.ycd@walk", |_| {
                loads.set(loads.get() + 1);
                Err("dictionary should already be decoded".to_owned())
            })
            .expect("load walk from cached dictionary");
        assert_eq!(loads.get(), 1);
        assert_eq!(idle.name, "idle");
        assert_eq!(walk.name, "walk");
        assert!(!std::sync::Arc::ptr_eq(&idle, &walk));
        assert_eq!(
            store.stats().unwrap(),
            AnimationClipStoreStats {
                dictionaries: 1,
                clips: 2,
            }
        );
    }

    #[test]
    fn sparse_bound_sampling_can_preserve_untracked_live_pose() {
        let clip = AnimationClip {
            name: "sparse-preserve-live".to_owned(),
            skeleton_ref: "skeleton.ymt@body".to_owned(),
            source: "test".to_owned(),
            duration_seconds: 1.0,
            sample_rate_hz: 2.0,
            looped: true,
            joint_tags: vec![20],
            events: Vec::new(),
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
        let runtime = AnimationSkeletonRuntime::compile(&skeleton, Mat4::IDENTITY.to_cols_array())
            .expect("compile skeleton");
        let binding = clip.bind_to_skeleton(&runtime).expect("bind sparse clip");
        let mut live_pose = runtime.bind_locals().to_vec();
        live_pose[0].translation = [9.0, 8.0, 7.0];
        clip.sample_local_pose_bound_preserve_untracked(0.25, &runtime, &binding, &mut live_pose)
            .expect("sample sparse over live pose");

        assert_eq!(
            live_pose[0].translation,
            [9.0, 8.0, 7.0],
            "untracked root must remain the last live pose, never bind/default pose"
        );
        assert!((live_pose[1].translation[0] - 1.0).abs() < 1.0e-6);
    }
}
