#[cfg(test)]
mod transition_tests {
    use super::*;

    #[test]
    fn fall_presentation_selects_authored_height_bands_deterministically() {
        let select = |distance| select_fall_presentation_band(distance, true, true, true, 2.0, 5.0);
        assert_eq!(select(0.0), Some(FallPresentationBand::Low));
        assert_eq!(select(1.999), Some(FallPresentationBand::Low));
        assert_eq!(select(2.0), Some(FallPresentationBand::Medium));
        assert_eq!(select(4.999), Some(FallPresentationBand::Medium));
        assert_eq!(select(5.0), Some(FallPresentationBand::High));
        assert_eq!(select(25.0), Some(FallPresentationBand::High));
    }

    #[test]
    fn fall_presentation_never_substitutes_a_different_authored_severity() {
        assert_eq!(
            select_fall_presentation_band(0.5, false, true, true, 2.0, 5.0),
            None,
            "missing low must hold the current pose, not play medium/high"
        );
        assert_eq!(
            select_fall_presentation_band(3.0, true, false, true, 2.0, 5.0),
            None,
            "missing medium must not substitute low"
        );
        assert_eq!(
            select_fall_presentation_band(7.0, true, true, false, 2.0, 5.0),
            None,
            "missing high must not substitute medium/low"
        );
        assert_eq!(
            select_fall_presentation_band(7.0, false, false, false, 2.0, 5.0),
            None
        );
    }

    #[test]
    fn runtime_locomotion_selection_never_falls_back_to_another_state() {
        let clips: [Option<PlayerAnimationRuntimeClip>; 8] = std::array::from_fn(|_| None);
        assert_eq!(
            resolve_runtime_locomotion_slot(
                &clips,
                newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Fall,
            ),
            None
        );
        assert_eq!(
            resolve_runtime_locomotion_slot(
                &clips,
                newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Sprint,
            ),
            None
        );
    }

    #[test]
    fn pose_continuity_bridge_starts_blending_on_the_source_change_frame() {
        let previous = vec![JointLocalPose {
            translation: [0.35, 1.1, -0.22],
            rotation: {
                let q = Quat::from_rotation_y(32.0_f32.to_radians());
                [q.x, q.y, q.z, q.w]
            },
            scale: Some([1.0, 1.0, 1.0]),
        }];
        let mut target = vec![JointLocalPose {
            translation: [0.0, 0.9, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: Some([1.0, 1.0, 1.0]),
        }];
        let mut bridge = PoseContinuityBridge::new(&previous);
        let idle = PoseContinuityKey {
            clip_hash: animation_source_hash("idle"),
            ..PoseContinuityKey::default()
        };
        bridge.apply(idle, &mut target, 1.0 / 60.0);
        bridge.commit_visible_pose(&previous);

        // Establish initial source without altering it.
        target[0] = previous[0];
        let mut next = vec![JointLocalPose {
            translation: [0.0, 0.9, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: Some([1.0, 1.0, 1.0]),
        }];
        let walk = PoseContinuityKey {
            clip_hash: animation_source_hash("walk"),
            ..PoseContinuityKey::default()
        };
        bridge.apply(walk, &mut next, 1.0 / 60.0);

        assert_ne!(
            next[0].translation, previous[0].translation,
            "transition must make visible progress on the same frame"
        );
        assert_ne!(
            next[0].translation,
            [0.0, 0.9, 0.0],
            "transition must blend, not snap to the destination"
        );
    }

    #[test]
    fn pose_continuity_bridge_converges_without_root_translation_reset() {
        let previous = vec![JointLocalPose {
            translation: [0.4, 1.0, -0.3],
            rotation: {
                let q = Quat::from_rotation_y(45.0_f32.to_radians());
                [q.x, q.y, q.z, q.w]
            },
            scale: Some([1.0, 1.0, 1.0]),
        }];
        let authored_target = JointLocalPose {
            translation: [0.0, 0.8, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: Some([1.0, 1.0, 1.0]),
        };
        let mut bridge = PoseContinuityBridge::new(&previous);
        let key_a = PoseContinuityKey {
            clip_hash: animation_source_hash("a"),
            ..PoseContinuityKey::default()
        };
        let mut establish = previous.clone();
        bridge.apply(key_a, &mut establish, 1.0 / 60.0);
        bridge.commit_visible_pose(&previous);

        let key_b = PoseContinuityKey {
            clip_hash: animation_source_hash("b"),
            ..PoseContinuityKey::default()
        };
        let mut visible = previous.clone();
        for _ in 0..10 {
            let mut target = vec![authored_target];
            bridge.apply(key_b, &mut target, 1.0 / 60.0);
            // Root position moves continuously; it is never reset to the target in one frame.
            let dx = (target[0].translation[0] - visible[0].translation[0]).abs();
            assert!(dx <= 0.2);
            visible = target;
            bridge.commit_visible_pose(&visible);
        }
        assert!((visible[0].translation[0] - authored_target.translation[0]).abs() < 1.0e-5);
        assert!((visible[0].translation[1] - authored_target.translation[1]).abs() < 1.0e-5);
        assert!((visible[0].translation[2] - authored_target.translation[2]).abs() < 1.0e-5);
    }

    #[test]
    fn repeated_same_turn_clip_uses_sequence_to_force_pose_continuity() {
        let clip_hash = animation_source_hash("turn-45-left");
        let first = PoseContinuityKey {
            clip_hash,
            turn_sequence: 1,
            ..PoseContinuityKey::default()
        };
        let second = PoseContinuityKey {
            clip_hash,
            turn_sequence: 2,
            ..PoseContinuityKey::default()
        };
        assert_ne!(first, second);
    }

    #[test]
    fn native_turn_in_place_selects_nearest_available_signed_step() {
        let all = |_slot: TurnInPlaceSlot| true;
        assert_eq!(
            nearest_turn_in_place_slot(40.0_f32.to_radians(), all),
            Some(TurnInPlaceSlot::Left45)
        );
        assert_eq!(
            nearest_turn_in_place_slot(-52.0_f32.to_radians(), all),
            Some(TurnInPlaceSlot::Right45)
        );
        assert_eq!(
            nearest_turn_in_place_slot(65.0_f32.to_radians(), all),
            Some(TurnInPlaceSlot::Left45)
        );
        assert_eq!(
            nearest_turn_in_place_slot(96.0_f32.to_radians(), all),
            Some(TurnInPlaceSlot::Left90)
        );
        assert_eq!(
            nearest_turn_in_place_slot(-168.0_f32.to_radians(), all),
            Some(TurnInPlaceSlot::Right180)
        );
        assert_eq!(
            nearest_turn_in_place_slot(20.0_f32.to_radians(), all),
            Some(TurnInPlaceSlot::Left45)
        );
        assert_eq!(nearest_turn_in_place_slot(0.0, all), None);

        // Missing 45° content must degrade to the nearest authored step on the same side,
        // never to a rigid world-root turn.
        let only_left_90 = |slot: TurnInPlaceSlot| slot == TurnInPlaceSlot::Left90;
        assert_eq!(
            nearest_turn_in_place_slot(42.0_f32.to_radians(), only_left_90),
            Some(TurnInPlaceSlot::Left90)
        );
        assert_eq!(
            nearest_turn_in_place_slot(70.0_f32.to_radians(), only_left_90),
            Some(TurnInPlaceSlot::Left90)
        );
        assert_eq!(
            nearest_turn_in_place_slot(-90.0_f32.to_radians(), only_left_90),
            None
        );
    }

    #[test]
    fn active_body_turn_never_captures_free_look_and_replans_only_after_opposite_limit_crossing() {
        let hysteresis = 10.0_f32.to_radians();
        assert!(
            !live_view_residual_requires_turn_replan(
                TurnInPlaceSlot::Left45,
                4.0_f32.to_radians(),
                hysteresis,
            ),
            "returning inside the authored look envelope must not pop the planted turn step"
        );
        assert!(
            !live_view_residual_requires_turn_replan(
                TurnInPlaceSlot::Left45,
                35.0_f32.to_radians(),
                hysteresis,
            ),
            "continuing to look left keeps the current authored left step"
        );
        assert!(
            !live_view_residual_requires_turn_replan(
                TurnInPlaceSlot::Left45,
                -8.0_f32.to_radians(),
                hysteresis,
            ),
            "small opposite residual remains inside hysteresis and must not thrash turn direction"
        );
        assert!(
            live_view_residual_requires_turn_replan(
                TurnInPlaceSlot::Left45,
                -25.0_f32.to_radians(),
                hysteresis,
            ),
            "free-look crossing the opposite authored limit must re-plan the body turn"
        );
        assert!(
            live_view_residual_requires_turn_replan(
                TurnInPlaceSlot::Right90,
                30.0_f32.to_radians(),
                hysteresis,
            ),
            "right turn must likewise yield to live left free-look residual"
        );
    }

    #[test]
    fn native_turn_in_place_yaw_is_eased_and_hard_bounded_against_snap_turns() {
        let midpoint = turn_in_place_target_yaw(TurnInPlaceSlot::Left90, 0.5, 1.0);
        assert!((midpoint - 45.0_f32.to_radians()).abs() <= 1.0e-6);

        let positive = bounded_turn_in_place_step(90.0_f32.to_radians());
        let negative = bounded_turn_in_place_step(-180.0_f32.to_radians());
        assert!((positive - 6.0_f32.to_radians()).abs() <= 1.0e-6);
        assert!((negative + 6.0_f32.to_radians()).abs() <= 1.0e-6);
        assert_eq!(
            bounded_turn_in_place_step(0.25_f32.to_radians()),
            0.25_f32.to_radians()
        );
    }

    #[test]
    fn native_turn_in_place_accumulation_remains_continuous_across_pi_for_180_steps() {
        let applied = 179.0_f32.to_radians();
        let next =
            accumulate_turn_in_place_yaw(applied, 179.0_f32.to_radians(), -179.0_f32.to_radians());
        assert!((next - 181.0_f32.to_radians()).abs() <= 1.0e-5);
    }

    #[test]
    fn extracted_world_turn_is_removed_from_skeleton_root_without_double_spin() {
        let mut pose = [JointLocalPose {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: Some([1.0, 1.0, 1.0]),
        }];
        let extracted = 30.0_f32.to_radians();
        compensate_turn_root_yaw(&mut pose, Some(0), extracted);
        let local = Quat::from_xyzw(
            pose[0].rotation[0],
            pose[0].rotation[1],
            pose[0].rotation[2],
            pose[0].rotation[3],
        )
        .normalize_or_identity();
        let world = (Quat::from_rotation_y(extracted) * local).normalize_or_identity();
        assert!(world.dot(Quat::IDENTITY).abs() > 0.99999);
    }
    fn test_look_delta(yaw: f32) -> AuthoredLookJointDelta {
        let q = Quat::from_rotation_y(yaw);
        AuthoredLookJointDelta {
            translation: [0.0; 3],
            rotation: [q.x, q.y, q.z, q.w],
            scale_ratio: [1.0; 3],
        }
    }

    fn test_look_space() -> AuthoredLookPoseSpace {
        AuthoredLookPoseSpace {
            role: "test",
            joints: vec![2],
            samples: vec![
                AuthoredLookSample {
                    coord: [0.0, 0.0],
                    deltas: vec![test_look_delta(0.0)],
                },
                AuthoredLookSample {
                    coord: [0.5, 0.0],
                    deltas: vec![test_look_delta(0.5)],
                },
                AuthoredLookSample {
                    coord: [0.0, 0.5],
                    deltas: vec![test_look_delta(0.0)],
                },
                AuthoredLookSample {
                    coord: [-0.5, 0.0],
                    deltas: vec![test_look_delta(-0.5)],
                },
                AuthoredLookSample {
                    coord: [0.0, -0.5],
                    deltas: vec![test_look_delta(0.0)],
                },
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 1]],
            turn_hysteresis_radians: 0.1,
        }
    }

    #[test]
    fn authored_look_range_frame_composes_additive_delta_over_authored_base() {
        use newengine_model_skeleton_api::{ModelSkeletonAnchors, ModelSkeletonJointMetadata};

        let skeleton = ModelSkeletonMetadata {
            source: "authored-look-additive-test".to_owned(),
            source_format: "test".to_owned(),
            container_magic: "TEST".to_owned(),
            byte_len: 0,
            content_hash: String::new(),
            decode_status: "ok".to_owned(),
            joints: vec![
                ModelSkeletonJointMetadata {
                    index: 0,
                    tag: 0,
                    name: "root".to_owned(),
                    parent: None,
                    parent_index: None,
                    position_ls: [0.0, 0.0, 0.0],
                    rotation_ls: [0.0, 0.0, 0.0, 1.0],
                    scale_ls: [1.0, 1.0, 1.0],
                    flags: Vec::new(),
                },
                ModelSkeletonJointMetadata {
                    index: 1,
                    tag: 1,
                    name: "head".to_owned(),
                    parent: Some("root".to_owned()),
                    parent_index: Some(0),
                    position_ls: [0.0, 0.5, 0.0],
                    rotation_ls: [0.0, 0.0, 0.0, 1.0],
                    scale_ls: [1.0, 1.0, 1.0],
                    flags: Vec::new(),
                },
            ],
            anchors: ModelSkeletonAnchors {
                root: "root".to_owned(),
                hips: "root".to_owned(),
                head: "head".to_owned(),
                left_hand: "head".to_owned(),
                right_hand: "head".to_owned(),
                left_foot: "root".to_owned(),
                right_foot: "root".to_owned(),
                eye: "head".to_owned(),
                eye_height: 1.0,
            },
        };
        let animation_runtime =
            AnimationSkeletonRuntime::compile(&skeleton, Mat4::IDENTITY.to_cols_array())
                .expect("compile additive look skeleton");
        let base_rotation = Quat::from_rotation_y(0.25);
        let base_pose = vec![
            JointLocalPose {
                translation: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: Some([1.0, 1.0, 1.0]),
            },
            JointLocalPose {
                translation: [0.0, 0.75, 0.2],
                rotation: [
                    base_rotation.x,
                    base_rotation.y,
                    base_rotation.z,
                    base_rotation.w,
                ],
                scale: Some([2.0, 3.0, 4.0]),
            },
        ];
        let delta_rotation = Quat::from_rotation_x(-0.4);
        let raw_clip = AnimationClip {
            name: "look-range-add".to_owned(),
            skeleton_ref: "test".to_owned(),
            source: "test".to_owned(),
            duration_seconds: 1.0 / 30.0,
            sample_rate_hz: 30.0,
            looped: false,
            joint_tags: vec![1],
            events: Vec::new(),
            poses: vec![JointLocalPose {
                translation: [0.1, -0.05, 0.025],
                rotation: [
                    delta_rotation.x,
                    delta_rotation.y,
                    delta_rotation.z,
                    delta_rotation.w,
                ],
                scale: Some([0.5, 2.0, 0.25]),
            }],
        };
        let binding = raw_clip
            .bind_to_skeleton(&animation_runtime)
            .expect("bind additive look clip");
        let clip = PlayerAnimationRuntimeClip {
            clip_ref: "test@look-range-add".to_owned(),
            clip: raw_clip.into(),
            binding,
            event_cursor: AnimationEventCursor::default(),
        };

        let raw = sample_look_range_raw_frame(&clip, 0, &animation_runtime)
            .expect("sample additive look frame");
        let raw_frames = vec![raw.clone()];
        let channels = vec![(1, look_channel_policy(1, &base_pose, &raw_frames))];
        let composed = compose_look_range_frame(&base_pose, &raw, &channels);
        assert_eq!(
            composed[0], base_pose[0],
            "untracked root must remain authored base"
        );
        assert!((composed[1].translation[0] - 0.1).abs() <= 1.0e-6);
        assert!((composed[1].translation[1] - 0.70).abs() <= 1.0e-6);
        assert!((composed[1].translation[2] - 0.225).abs() <= 1.0e-6);
        let expected_rotation = (base_rotation * delta_rotation).normalize_or_identity();
        let actual_rotation = look_quat(&composed[1]);
        assert!(actual_rotation.dot(expected_rotation).abs() > 0.999999);
        let scale = composed[1].scale.expect("composed scale");
        assert!((scale[0] - 1.0).abs() <= 1.0e-6);
        assert!((scale[1] - 6.0).abs() <= 1.0e-6);
        assert!((scale[2] - 1.0).abs() <= 1.0e-6);
    }

    #[test]
    fn authored_look_mixed_absolute_fillers_are_not_reapplied_as_additive_channels() {
        let base_neck_rotation = Quat::from_rotation_y(0.35);
        let base_eye_rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let base_pose = vec![
            JointLocalPose {
                translation: [0.0, -0.003, 0.156],
                rotation: [
                    base_neck_rotation.x,
                    base_neck_rotation.y,
                    base_neck_rotation.z,
                    base_neck_rotation.w,
                ],
                scale: Some([1.0, 1.0, 1.0]),
            },
            JointLocalPose {
                translation: [0.032, -0.070, 0.055],
                rotation: [
                    base_eye_rotation.x,
                    base_eye_rotation.y,
                    base_eye_rotation.z,
                    base_eye_rotation.w,
                ],
                scale: Some([1.0, 1.0, 1.0]),
            },
        ];
        let eye_delta_a = Quat::from_rotation_y(-0.25);
        let eye_delta_b = Quat::from_rotation_y(0.25);
        let make_raw = |eye_delta: Quat| {
            vec![
                // Generic male eyes-look records repeat the absolute bind-local neck channels.
                // They are fillers and must not be added/multiplied onto the live pose.
                base_pose[0],
                JointLocalPose {
                    // The eye channel is authored around additive neutral instead.
                    translation: [0.001, 0.0, -0.001],
                    rotation: [eye_delta.x, eye_delta.y, eye_delta.z, eye_delta.w],
                    scale: Some([1.0, 1.0, 1.0]),
                },
            ]
        };
        let raw_frames = vec![make_raw(eye_delta_a), make_raw(eye_delta_b)];
        let neck_policy = look_channel_policy(0, &base_pose, &raw_frames);
        let eye_policy = look_channel_policy(1, &base_pose, &raw_frames);
        assert!(!neck_policy.translation_additive);
        assert!(!neck_policy.rotation_additive);
        assert!(!neck_policy.scale_multiplicative);
        assert!(eye_policy.translation_additive);
        assert!(eye_policy.rotation_additive);

        let channels = vec![(0, neck_policy), (1, eye_policy)];
        let composed = compose_look_range_frame(&base_pose, &raw_frames[0], &channels);
        assert_eq!(
            composed[0], base_pose[0],
            "absolute neck filler must stay neutral"
        );
        assert!((composed[1].translation[0] - 0.033).abs() <= 1.0e-6);
        assert!((composed[1].translation[2] - 0.054).abs() <= 1.0e-6);
        let expected_eye = (base_eye_rotation * eye_delta_a).normalize_or_identity();
        assert!(look_quat(&composed[1]).dot(expected_eye).abs() > 0.999999);
    }

    #[test]
    fn explicit_look_context_overrides_standard_locomotion_and_equipment_selection() {
        use newengine_engine_runtime::gameplay::{
            PlayerLocomotionAnimation as L, PlayerLookContext as C,
        };

        assert_eq!(
            resolve_authored_look_state(L::CrouchIdle, EquipmentPresentationStance::Aim, C::Prone),
            AuthoredLookState::Prone
        );
        assert_eq!(
            resolve_authored_look_state(L::Idle, EquipmentPresentationStance::Ready, C::Rope),
            AuthoredLookState::Rope
        );
        assert_eq!(
            resolve_authored_look_state(
                L::CrouchWalk,
                EquipmentPresentationStance::None,
                C::Standard
            ),
            AuthoredLookState::Crouch
        );
        assert_eq!(
            resolve_authored_look_state(L::Idle, EquipmentPresentationStance::Aim, C::Standard),
            AuthoredLookState::Tense
        );
    }

    #[test]
    fn contextual_look_state_without_authored_range_fails_closed_instead_of_using_relaxed() {
        let binding = AuthoredLookRuntimeBinding {
            relaxed: Some(test_look_space()),
            ..AuthoredLookRuntimeBinding::default()
        };
        assert!(binding.body_space(AuthoredLookState::Relaxed).is_some());
        assert!(binding.body_space(AuthoredLookState::Prone).is_none());
        assert!(binding.body_space(AuthoredLookState::Rope).is_none());
        assert!(AuthoredLookState::Prone.contextual());
        assert!(!AuthoredLookState::Relaxed.contextual());
    }

    #[test]
    fn fall_full_body_override_requires_authored_fall_event_and_fall_locomotion() {
        use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation;

        assert!(!authoritative_fall_presentation_requested(
            false,
            true,
            PlayerLocomotionAnimation::Walk,
        ));
        assert!(authoritative_fall_presentation_requested(
            false,
            true,
            PlayerLocomotionAnimation::Fall,
        ));
        assert!(!authoritative_fall_presentation_requested(
            false,
            false,
            PlayerLocomotionAnimation::Fall,
        ));
        assert!(!authoritative_fall_presentation_requested(
            true,
            true,
            PlayerLocomotionAnimation::Fall,
        ));
    }

    #[test]
    fn authored_look_pose_space_consumes_view_inside_native_hull() {
        let space = test_look_space();
        let binding = AuthoredLookRuntimeBinding {
            relaxed: Some(space),
            ..AuthoredLookRuntimeBinding::default()
        };
        let projection = binding
            .projection(AuthoredLookState::Relaxed, 0.2, 0.1)
            .expect("authored look projection");
        assert!((projection.body_projected[0] - 0.2).abs() <= 1.0e-5);
        assert!((projection.body_projected[1] - 0.1).abs() <= 1.0e-5);
        assert!(projection.residual[0].abs() <= 1.0e-5);
        assert!(projection.residual[1].abs() <= 1.0e-5);
    }

    #[test]
    fn authored_look_pose_space_hands_only_uncovered_residual_to_body_turn() {
        let space = test_look_space();
        let binding = AuthoredLookRuntimeBinding {
            relaxed: Some(space),
            ..AuthoredLookRuntimeBinding::default()
        };
        let projection = binding
            .projection(AuthoredLookState::Relaxed, 0.9, 0.0)
            .expect("authored look projection");
        assert!((projection.body_projected[0] - 0.5).abs() <= 1.0e-5);
        assert!((projection.residual[0] - 0.4).abs() <= 1.0e-5);
        assert!(projection.residual[0].abs() > projection.turn_hysteresis_radians);
    }

    #[test]
    fn authored_look_pose_space_modifies_only_authored_joints() {
        let space = test_look_space();
        let mut pose = vec![
            JointLocalPose {
                translation: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: Some([1.0; 3]),
            };
            4
        ];
        let hips_before = pose[1];
        let head_before = pose[2];
        let blend = space.solve([0.25, 0.0]);
        space.apply_blend(blend, &mut pose);
        assert_eq!(
            pose[1], hips_before,
            "hips are not part of the authored look range"
        );
        assert_ne!(
            pose[2].rotation, head_before.rotation,
            "authored head joint must receive range delta"
        );
    }

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
        let authored_pose = pose.clone();
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
        let authored_handle = crate::weapon_grip::weapon_handle_anchor_from_right_palm(
            &presentation,
            frames[rig.right_palm],
        )
        .expect("authored firing-hand handle anchor");
        let right_upper_len_before = (frames[rig.right_elbow].transform_point3(Vec3::ZERO)
            - frames[rig.right_shoulder].transform_point3(Vec3::ZERO))
        .length();
        let right_lower_len_before = (frames[rig.right_wrist].transform_point3(Vec3::ZERO)
            - frames[rig.right_elbow].transform_point3(Vec3::ZERO))
        .length();
        let left_upper_len_before = (frames[rig.left_elbow].transform_point3(Vec3::ZERO)
            - frames[rig.left_shoulder].transform_point3(Vec3::ZERO))
        .length();
        let left_lower_len_before = (frames[rig.left_wrist].transform_point3(Vec3::ZERO)
            - frames[rig.left_elbow].transform_point3(Vec3::ZERO))
        .length();

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
            false,
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
        assert!(
            left_after_error <= left_before_error + 1.0e-6,
            "support-hand correction may improve contact but must never worsen it"
        );
        if (left_after_error - left_before_error).abs() <= 1.0e-6 {
            assert!(
                left_after.distance(left_before) <= 1.0e-6,
                "unreachable support grip must preserve authored hand position instead of locking the arm at full extension"
            );
        }
        let resolved_handle =
            crate::weapon_grip::weapon_handle_position(&presentation, final_result.base_root);
        assert!(
            (resolved_handle - authored_handle).length() <= 1.0e-5,
            "third-person Ready/Aim must preserve authored firing-hand translation"
        );
        let right_upper_len_after = (frames[rig.right_elbow].transform_point3(Vec3::ZERO)
            - frames[rig.right_shoulder].transform_point3(Vec3::ZERO))
        .length();
        let right_lower_len_after = (frames[rig.right_wrist].transform_point3(Vec3::ZERO)
            - frames[rig.right_elbow].transform_point3(Vec3::ZERO))
        .length();
        let left_upper_len_after = (frames[rig.left_elbow].transform_point3(Vec3::ZERO)
            - frames[rig.left_shoulder].transform_point3(Vec3::ZERO))
        .length();
        let left_lower_len_after = (frames[rig.left_wrist].transform_point3(Vec3::ZERO)
            - frames[rig.left_elbow].transform_point3(Vec3::ZERO))
        .length();
        for (label, before, after) in [
            ("right upper", right_upper_len_before, right_upper_len_after),
            ("right lower", right_lower_len_before, right_lower_len_after),
            ("left upper", left_upper_len_before, left_upper_len_after),
            ("left lower", left_lower_len_before, left_lower_len_after),
        ] {
            assert!(
                (after - before).abs() <= 1.0e-5,
                "{label} arm segment stretched before={before:.6} after={after:.6}"
            );
        }
        assert!(final_result.error_m.is_finite());

        // A prop attachment joint appearing in a generic authored hand pose is not sufficient to
        // claim weapon-root ownership. The socket -> handle basis belongs to a qualified authored
        // grip domain, so an unrelated attachment frame must be ignored until that domain opts in.
        let unqualified_socket_rig = WeaponArmIkRig {
            right_prop_attachment: Some(rig.right_wrist),
            ..rig
        };
        let mut unqualified_socket_pose = authored_pose.clone();
        let mut unqualified_socket_frames = Vec::new();
        rebuild_model_joint_frames(
            &animation_runtime,
            &unqualified_socket_pose,
            &mut unqualified_socket_frames,
        )
        .expect("unqualified socket initial frames");
        let incompatible_socket_root = crate::weapon_grip::weapon_root_from_authored_prop_frame(
            &presentation,
            unqualified_socket_frames[unqualified_socket_rig
                .right_prop_attachment
                .expect("unqualified socket joint")],
        )
        .expect("incompatible authored socket root");
        let unqualified_socket_result = apply_equipped_weapon_support_ik(
            &presentation,
            Some(&unqualified_socket_rig),
            &skeleton,
            &animation_runtime,
            &mut unqualified_socket_pose,
            &mut unqualified_socket_frames,
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
            false,
            true,
            true,
        )
        .expect("unqualified prop socket presentation")
        .expect("fallback root");
        assert!(
            unqualified_socket_result
                .base_root
                .position
                .distance(incompatible_socket_root.position)
                > 1.0e-4,
            "generic authored hand contact must not apply a socket basis from another authored domain"
        );
        assert_eq!(unqualified_socket_result.socket_position_error_m, 0.0);
        assert_eq!(unqualified_socket_result.socket_angular_error_deg, 0.0);

        // A native equipment prop socket owns the weapon root. Both physical hands are still a
        // correction layer over that root: the firing hand is an identity solve at the authored
        // contact, while the support hand may reduce residual without moving the weapon.
        let socket_rig = WeaponArmIkRig {
            right_prop_attachment: Some(rig.right_palm),
            ..rig
        };
        let mut socket_presentation = presentation.clone();
        socket_presentation.authored_socket_to_weapon_handle_basis = [0.0, 0.0, 0.0, 1.0];
        socket_presentation.handle_rotation_from_root = [0.0, 0.0, 0.0, 1.0];
        socket_presentation.ready_right_palm_to_weapon = [0.0, 0.0, 0.0, 1.0];
        socket_presentation.right_palm_to_handle = [0.0; 3];
        let mut socket_pose = authored_pose.clone();
        let mut socket_frames = Vec::new();
        rebuild_model_joint_frames(&animation_runtime, &socket_pose, &mut socket_frames)
            .expect("socket initial frames");
        let expected_socket_root = crate::weapon_grip::weapon_root_from_authored_prop_frame(
            &socket_presentation,
            socket_frames[socket_rig.right_prop_attachment.expect("socket joint")],
        )
        .expect("authored socket root");
        let socket_pose_before = socket_pose.clone();
        let socket_left_before = socket_frames[rig.left_palm].transform_point3(Vec3::ZERO);
        let socket_left_target = crate::weapon_grip::weapon_ready_left_palm_position(
            &socket_presentation,
            expected_socket_root,
        );
        let socket_left_before_error = socket_left_before.distance(socket_left_target);
        let socket_result = apply_equipped_weapon_support_ik(
            &socket_presentation,
            Some(&socket_rig),
            &skeleton,
            &animation_runtime,
            &mut socket_pose,
            &mut socket_frames,
            None,
            None,
            None,
            false,
            1.0,
            0.0,
            0.0,
            0.0,
            Vec3::ZERO,
            true,
            true,
            true,
            true,
        )
        .expect("authored socket presentation")
        .expect("socket root");
        assert!(
            socket_result
                .base_root
                .position
                .distance(expected_socket_root.position)
                <= 1.0e-6
        );
        assert!(
            socket_result
                .base_root
                .rotation
                .dot(expected_socket_root.rotation)
                .abs()
                > 0.999_999
        );
        for index in [
            rig.right_shoulder,
            rig.right_elbow,
            rig.right_wrist,
            rig.right_palm,
        ] {
            assert_eq!(
                socket_pose[index], socket_pose_before[index],
                "canonical firing-hand contact must be an identity correction"
            );
        }
        let socket_left_after = socket_frames[rig.left_palm].transform_point3(Vec3::ZERO);
        assert!(
            socket_left_after.distance(socket_left_target) <= socket_left_before_error + 1.0e-6,
            "prop-owned root must still admit a non-worsening support-hand correction"
        );
        assert!(socket_result.right_error_m <= 1.0e-6);
        assert!(socket_result.socket_position_error_m <= 1.0e-6);
        assert!(socket_result.socket_angular_error_deg <= 0.001);

        // Full-body first person keeps the authored right-palm handle as the kinematic weapon
        // owner, but ADS is allowed to rotate the real arm chains so both palms follow the final
        // sight-aligned weapon contacts. Limb translations/segment lengths remain anatomical.
        let mut fpp_pose = authored_pose.clone();
        let mut fpp_frames = Vec::new();
        rebuild_model_joint_frames(&animation_runtime, &fpp_pose, &mut fpp_frames)
            .expect("FPP initial frames");
        let fpp_handle = crate::weapon_grip::weapon_handle_anchor_from_right_palm(
            &presentation,
            fpp_frames[rig.right_palm],
        )
        .expect("FPP authored handle");
        let authored_root = crate::weapon_grip::weapon_root_from_right_palm(
            &presentation,
            fpp_frames[rig.right_palm],
        )
        .expect("FPP authored weapon root");
        let authored_sight = crate::weapon_grip::weapon_sight_forward(&presentation, authored_root);
        let aim_delta = Quat::from_rotation_y(0.04) * Quat::from_rotation_x(-0.02);
        let aimed_forward = (aim_delta * authored_sight).normalize_or_zero();
        let view = Quat::from_rotation_arc(-Vec3::Z, aimed_forward).normalize_or_identity();
        let expected_fpp_root = crate::weapon_grip::weapon_first_person_hand_anchored_root(
            &presentation,
            fpp_frames[rig.right_palm],
            view,
            1.0,
            0.0,
            0.0,
        )
        .expect("expected FPP hand-owned root");
        let fpp_left_before = fpp_frames[rig.left_palm].transform_point3(Vec3::ZERO);
        let fpp_left_target_before =
            crate::weapon_grip::weapon_ready_left_palm_position(&presentation, expected_fpp_root);
        let fpp_left_before_error = fpp_left_before.distance(fpp_left_target_before);
        let fpp_result = apply_equipped_weapon_support_ik(
            &presentation,
            Some(&rig),
            &skeleton,
            &animation_runtime,
            &mut fpp_pose,
            &mut fpp_frames,
            Some((view * -Vec3::Z).normalize_or_zero()),
            Some(view),
            Some(Vec3::new(0.0, 1.62, 0.0)),
            true,
            1.0,
            0.0,
            0.0,
            0.0,
            Vec3::ZERO,
            true,
            false,
            true,
            true,
        )
        .expect("FPP authored hand presentation")
        .expect("FPP weapon root");
        let right_target = crate::weapon_grip::weapon_hand_owned_right_palm_position(
            &presentation,
            fpp_result.base_root,
        );
        let left_target = crate::weapon_grip::weapon_ready_left_palm_position(
            &presentation,
            fpp_result.base_root,
        );
        let right_actual = fpp_frames[rig.right_palm].transform_point3(Vec3::ZERO);
        let left_actual = fpp_frames[rig.left_palm].transform_point3(Vec3::ZERO);
        assert!((right_actual.distance(right_target) - fpp_result.right_error_m).abs() <= 1.0e-5);
        assert!((left_actual.distance(left_target) - fpp_result.left_error_m).abs() <= 1.0e-5);
        assert!(
            fpp_result.right_error_m <= 0.005,
            "FPP firing-hand contact residual too large: {}",
            fpp_result.right_error_m,
        );
        assert!(
            fpp_result.left_error_m <= fpp_left_before_error + 1.0e-6,
            "FPP support-hand solve may improve an authored contact but must never worsen it before={} after={}",
            fpp_left_before_error,
            fpp_result.left_error_m,
        );
        if (fpp_result.left_error_m - fpp_left_before_error).abs() <= 1.0e-6 {
            assert!(
                left_actual.distance(fpp_left_before) <= 1.0e-6,
                "unreachable FPP support grip must preserve authored hand position instead of stretching the arm",
            );
        }
        let resolved_handle =
            crate::weapon_grip::weapon_handle_position(&presentation, fpp_result.base_root);
        assert!(resolved_handle.distance(fpp_handle) <= 1.0e-5);
        let sight_forward =
            crate::weapon_grip::weapon_sight_forward(&presentation, fpp_result.base_root);
        assert!(sight_forward.dot((view * -Vec3::Z).normalize_or_zero()) > 0.9999);
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
    fn terminal_helper_sync_reprojects_post_ik_wrist_and_finger_locals() {
        let identity = JointLocalPose {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: Some([1.0, 1.0, 1.0]),
        };
        let mut pose = vec![identity; 4];
        let rules = [
            ResolvedJointCopyRule {
                source_index: 0,
                target_index: 1,
                channels: newengine_engine_runtime::gameplay::PlayerJointChannels::all(),
            },
            ResolvedJointCopyRule {
                source_index: 2,
                target_index: 3,
                channels: newengine_engine_runtime::gameplay::PlayerJointChannels::all(),
            },
        ];

        // Initial pre-IK helper projection.
        synchronize_helper_pose(&rules, &mut pose);

        // Simulate terminal weapon contact mutations on the anatomical wrist/finger branch.
        let wrist_rotation = Quat::from_rotation_y(0.61) * Quat::from_rotation_x(-0.24);
        pose[0].translation = [0.03, -0.02, 0.01];
        pose[0].rotation = [
            wrist_rotation.x,
            wrist_rotation.y,
            wrist_rotation.z,
            wrist_rotation.w,
        ];
        let finger_rotation = Quat::from_rotation_x(0.93);
        pose[2].translation = [0.0, 0.0, 0.045];
        pose[2].rotation = [
            finger_rotation.x,
            finger_rotation.y,
            finger_rotation.z,
            finger_rotation.w,
        ];

        assert_ne!(pose[0], pose[1], "pre-IK helper wrist must now be stale");
        assert_ne!(pose[2], pose[3], "pre-IK helper finger must now be stale");

        synchronize_helper_pose(&rules, &mut pose);

        assert_eq!(
            pose[0], pose[1],
            "final wrist helper must match post-IK wrist"
        );
        assert_eq!(
            pose[2], pose[3],
            "final finger helper must match post-IK finger"
        );
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
    fn native_eye_palette_enforces_parent_deformation_invariant() {
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

#[cfg(test)]
fn resolve_equipment_presentation_stance(
    weapon_type: Option<newengine_engine_runtime::gameplay::WeaponType>,
    weapon_state: Option<newengine_engine_runtime::gameplay::PlayerWeaponState>,
    authored_presentation: bool,
) -> EquipmentPresentationStance {
    if !authored_presentation
        || weapon_type != Some(newengine_engine_runtime::gameplay::WeaponType::Firearm)
    {
        return EquipmentPresentationStance::None;
    }
    let Some(state) = weapon_state else {
        return EquipmentPresentationStance::Ready;
    };
    if state.reload_remaining > 0.0 {
        EquipmentPresentationStance::Reload
    } else if state.aiming {
        EquipmentPresentationStance::Aim
    } else {
        EquipmentPresentationStance::Ready
    }
}

#[cfg(test)]
mod equipment_stance_tests {
    use super::*;
    use newengine_engine_runtime::gameplay::{PlayerWeaponState, WeaponType};

    #[test]
    fn firearm_equipment_stance_resolves_ready_aim_reload() {
        let mut state = PlayerWeaponState::melee();
        assert_eq!(
            resolve_equipment_presentation_stance(Some(WeaponType::Firearm), Some(state), true),
            EquipmentPresentationStance::Ready
        );
        state.aiming = true;
        assert_eq!(
            resolve_equipment_presentation_stance(Some(WeaponType::Firearm), Some(state), true),
            EquipmentPresentationStance::Aim
        );
        state.reload_remaining = 0.5;
        assert_eq!(
            resolve_equipment_presentation_stance(Some(WeaponType::Firearm), Some(state), true),
            EquipmentPresentationStance::Reload
        );
    }

    #[test]
    fn unarmed_and_melee_never_activate_firearm_presentation() {
        for weapon_type in [WeaponType::Unarmed, WeaponType::Melee] {
            assert_eq!(
                resolve_equipment_presentation_stance(
                    Some(weapon_type),
                    Some(PlayerWeaponState::melee()),
                    true,
                ),
                EquipmentPresentationStance::None
            );
        }
        assert_eq!(
            resolve_equipment_presentation_stance(
                Some(WeaponType::Firearm),
                Some(PlayerWeaponState::melee()),
                false,
            ),
            EquipmentPresentationStance::None
        );
    }
}
