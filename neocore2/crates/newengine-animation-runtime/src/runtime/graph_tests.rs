#[cfg(test)]
mod graph_runtime_tests {
    use super::*;
    use newengine_model_skeleton_api::{
        ModelSkeletonAnchors, ModelSkeletonJointMetadata, ModelSkeletonMetadata,
    };
    use std::cell::Cell;

    fn joint(
        index: u32,
        tag: u32,
        name: &str,
        parent_index: Option<u32>,
        position: [f32; 3],
    ) -> ModelSkeletonJointMetadata {
        ModelSkeletonJointMetadata {
            index,
            tag,
            name: name.to_owned(),
            parent: parent_index.map(|index| if index == 0 { "root" } else { "upper" }.to_owned()),
            parent_index,
            position_ls: position,
            rotation_ls: [0.0, 0.0, 0.0, 1.0],
            scale_ls: [1.0, 1.0, 1.0],
            flags: Vec::new(),
        }
    }

    fn skeleton_metadata() -> ModelSkeletonMetadata {
        ModelSkeletonMetadata {
            source: "test.skel".to_owned(),
            source_format: "test".to_owned(),
            container_magic: "TEST".to_owned(),
            byte_len: 0,
            content_hash: "test".to_owned(),
            decode_status: "decoded".to_owned(),
            joints: vec![
                joint(0, 10, "root", None, [0.0, 0.0, 0.0]),
                joint(1, 20, "upper", Some(0), [0.0, 1.0, 0.0]),
            ],
            anchors: ModelSkeletonAnchors {
                root: "root".to_owned(),
                hips: "root".to_owned(),
                head: "upper".to_owned(),
                left_hand: "upper".to_owned(),
                right_hand: "upper".to_owned(),
                left_foot: "root".to_owned(),
                right_foot: "root".to_owned(),
                eye: "upper".to_owned(),
                eye_height: 1.0,
            },
        }
    }

    fn skeleton_runtime() -> AnimationSkeletonRuntime {
        AnimationSkeletonRuntime::compile(&skeleton_metadata(), Mat4::IDENTITY.to_cols_array())
            .expect("compile test skeleton")
    }

    fn pose(x: f32, y: f32) -> JointLocalPose {
        JointLocalPose {
            translation: [x, y, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: Some([1.0; 3]),
        }
    }

    fn clip(
        name: &str,
        duration: f32,
        sample_rate: f32,
        looped: bool,
        root_end_x: f32,
        upper_end_x: f32,
        events: Vec<AnimationEvent>,
    ) -> Arc<AnimationClip> {
        Arc::new(AnimationClip {
            name: name.to_owned(),
            skeleton_ref: "test.skel".to_owned(),
            source: format!("{name}.test"),
            duration_seconds: duration,
            sample_rate_hz: sample_rate,
            looped,
            joint_tags: vec![10, 20],
            events,
            poses: vec![
                pose(0.0, 0.0),
                pose(0.0, 1.0),
                pose(root_end_x, 0.0),
                pose(upper_end_x, 1.0),
            ],
        })
    }

    fn clip_motion(reference: &str) -> AnimationMotionDefinition {
        AnimationMotionDefinition::Clip(AnimationClipMotionDefinition::new(reference))
    }

    fn locomotion_sync_group() -> AnimationSyncGroupDefinition {
        AnimationSyncGroupDefinition {
            name: "locomotion".to_owned(),
            markers: vec![
                "foot.left.contact".to_owned(),
                "foot.right.contact".to_owned(),
            ],
        }
    }

    fn marker_events(left: f32, right: f32) -> Vec<AnimationEvent> {
        let mut events = vec![
            AnimationEvent::new(left, "foot.left.contact"),
            AnimationEvent::new(right, "foot.right.contact"),
        ];
        events.sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
        events
    }

    fn base_graph(state: AnimationGraphStateDefinition) -> AnimationGraphDefinition {
        AnimationGraphDefinition {
            name: "test.graph".to_owned(),
            entry_state: state.name.clone(),
            parameters: Vec::new(),
            states: vec![state],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        }
    }

    fn compile_with_clips(
        definition: AnimationGraphDefinition,
        skeleton: &AnimationSkeletonRuntime,
        clips: &[(&str, Arc<AnimationClip>)],
    ) -> CompiledAnimationGraph {
        CompiledAnimationGraph::compile(definition, skeleton, |reference| {
            clips
                .iter()
                .find(|(candidate, _)| candidate.eq_ignore_ascii_case(reference))
                .map(|(_, clip)| clip.clone())
                .ok_or_else(|| format!("missing test clip '{reference}'"))
        })
        .expect("compile graph")
    }

    fn graph_state(name: &str, clip_ref: &str) -> AnimationGraphStateDefinition {
        AnimationGraphStateDefinition {
            name: name.to_owned(),
            motion: clip_motion(clip_ref),
            speed: 1.0,
            root_motion: AnimationRootMotionMode::Disabled,
        }
    }

    fn bool_parameter(name: &str) -> AnimationGraphParameterDefinition {
        AnimationGraphParameterDefinition {
            name: name.to_owned(),
            default: AnimationGraphParameterValue::Bool(false),
        }
    }

    fn bool_transition(
        from: &str,
        to: &str,
        parameter: &str,
        blend_seconds: f32,
        priority: i32,
        group: Option<&str>,
        interruption: AnimationTransitionInterruptionPolicy,
    ) -> AnimationGraphTransitionDefinition {
        AnimationGraphTransitionDefinition {
            from: from.to_owned(),
            to: to.to_owned(),
            conditions: vec![AnimationTransitionCondition::Bool {
                parameter: parameter.to_owned(),
                equals: true,
            }],
            exit_time_normalized: None,
            blend_seconds,
            priority,
            group: group.map(str::to_owned),
            interruption,
        }
    }

    #[test]
    fn graph_compile_deduplicates_clip_bindings_across_states_and_layers() {
        let skeleton = skeleton_runtime();
        let idle = clip("idle", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let loads = Cell::new(0usize);
        let definition = AnimationGraphDefinition {
            name: "dedupe".to_owned(),
            entry_state: "Idle".to_owned(),
            parameters: Vec::new(),
            states: vec![AnimationGraphStateDefinition {
                name: "Idle".to_owned(),
                motion: clip_motion("test.ycd@idle"),
                speed: 1.0,
                root_motion: AnimationRootMotionMode::Disabled,
            }],
            transitions: Vec::new(),
            layers: vec![AnimationGraphLayerDefinition {
                name: "Upper".to_owned(),
                motion: clip_motion("TEST.YCD@IDLE"),
                mode: AnimationLayerBlendMode::Override,
                weight: 0.5,
                weight_parameter: None,
                mask: None,
                event_weight_threshold: 0.5,
            }],
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = CompiledAnimationGraph::compile(definition, &skeleton, |_| {
            loads.set(loads.get() + 1);
            Ok(idle.clone())
        })
        .unwrap();
        assert_eq!(graph.clip_count(), 1);
        assert_eq!(loads.get(), 1);
    }

    #[test]
    fn state_machine_crossfades_after_typed_condition() {
        let skeleton = skeleton_runtime();
        let idle = clip("idle", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let walk = clip("walk", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "locomotion".to_owned(),
            entry_state: "Idle".to_owned(),
            parameters: vec![AnimationGraphParameterDefinition {
                name: "moving".to_owned(),
                default: AnimationGraphParameterValue::Bool(false),
            }],
            states: vec![
                AnimationGraphStateDefinition {
                    name: "Idle".to_owned(),
                    motion: clip_motion("locomotion.ycd@idle"),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
                AnimationGraphStateDefinition {
                    name: "Walk".to_owned(),
                    motion: clip_motion("locomotion.ycd@walk"),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
            ],
            transitions: vec![AnimationGraphTransitionDefinition {
                from: "Idle".to_owned(),
                to: "Walk".to_owned(),
                conditions: vec![AnimationTransitionCondition::Bool {
                    parameter: "moving".to_owned(),
                    equals: true,
                }],
                exit_time_normalized: None,
                blend_seconds: 0.2,
                priority: 0,
                group: None,
                interruption: AnimationTransitionInterruptionPolicy::Never,
            }],
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("locomotion.ycd@idle", idle),
                ("locomotion.ycd@walk", walk),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        instance.set_bool(&graph, "moving", true).unwrap();
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.1, &mut evaluation).unwrap();
        assert_eq!(evaluation.transition.unwrap().alpha, 0.0);
        instance.evaluate(&graph, &skeleton, 0.1, &mut evaluation).unwrap();
        let alpha = evaluation.transition.unwrap().alpha;
        assert!((alpha - 0.5).abs() < 1.0e-5);
        // Walk target has advanced to 0.1 => upper X=1.0, then transition alpha 0.5 => 0.5.
        assert!((evaluation.local_pose[1].translation[0] - 0.5).abs() < 1.0e-4);
        instance.evaluate(&graph, &skeleton, 0.1, &mut evaluation).unwrap();
        assert_eq!(instance.active_state_index(), graph.state_index("Walk").unwrap());
        assert!(instance.transition().is_none());
    }

    #[test]
    fn transition_priority_wins_before_authored_order() {
        let skeleton = skeleton_runtime();
        let a = clip("a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let b = clip("b", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let c = clip("c", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "priority".to_owned(),
            entry_state: "A".to_owned(),
            parameters: vec![bool_parameter("go")],
            states: vec![
                graph_state("A", "priority.ycd@a"),
                graph_state("B", "priority.ycd@b"),
                graph_state("C", "priority.ycd@c"),
            ],
            transitions: vec![
                bool_transition(
                    "A",
                    "B",
                    "go",
                    0.0,
                    1,
                    None,
                    AnimationTransitionInterruptionPolicy::Never,
                ),
                bool_transition(
                    "A",
                    "C",
                    "go",
                    0.0,
                    10,
                    None,
                    AnimationTransitionInterruptionPolicy::Never,
                ),
            ],
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("priority.ycd@a", a),
                ("priority.ycd@b", b),
                ("priority.ycd@c", c),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        instance.set_bool(&graph, "go", true).unwrap();
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        assert_eq!(instance.active_state_index(), graph.state_index("C").unwrap());
    }

    #[test]
    fn equal_transition_priority_uses_authored_order() {
        let skeleton = skeleton_runtime();
        let a = clip("a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let b = clip("b", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let c = clip("c", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "tie".to_owned(),
            entry_state: "A".to_owned(),
            parameters: vec![bool_parameter("go")],
            states: vec![
                graph_state("A", "tie.ycd@a"),
                graph_state("B", "tie.ycd@b"),
                graph_state("C", "tie.ycd@c"),
            ],
            transitions: vec![
                bool_transition(
                    "A",
                    "B",
                    "go",
                    0.0,
                    7,
                    None,
                    AnimationTransitionInterruptionPolicy::Never,
                ),
                bool_transition(
                    "A",
                    "C",
                    "go",
                    0.0,
                    7,
                    None,
                    AnimationTransitionInterruptionPolicy::Never,
                ),
            ],
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("tie.ycd@a", a),
                ("tie.ycd@b", b),
                ("tie.ycd@c", c),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        instance.set_bool(&graph, "go", true).unwrap();
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        assert_eq!(instance.active_state_index(), graph.state_index("B").unwrap());
    }

    #[test]
    fn authored_transition_never_policy_blocks_automatic_interruption() {
        let skeleton = skeleton_runtime();
        let a = clip("a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let b = clip("b", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let c = clip("c", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "never".to_owned(),
            entry_state: "A".to_owned(),
            parameters: vec![bool_parameter("go_b"), bool_parameter("go_c")],
            states: vec![
                graph_state("A", "never.ycd@a"),
                graph_state("B", "never.ycd@b"),
                graph_state("C", "never.ycd@c"),
            ],
            transitions: vec![
                bool_transition(
                    "A",
                    "B",
                    "go_b",
                    1.0,
                    0,
                    Some("locomotion"),
                    AnimationTransitionInterruptionPolicy::Never,
                ),
                bool_transition(
                    "B",
                    "C",
                    "go_c",
                    0.5,
                    100,
                    Some("locomotion"),
                    AnimationTransitionInterruptionPolicy::Any,
                ),
            ],
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("never.ycd@a", a),
                ("never.ycd@b", b),
                ("never.ycd@c", c),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.set_bool(&graph, "go_b", true).unwrap();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.set_bool(&graph, "go_c", true).unwrap();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        let transition = evaluation.transition.unwrap();
        assert_eq!(transition.from_state, graph.state_index("A").unwrap());
        assert_eq!(transition.to_state, graph.state_index("B").unwrap());
    }

    #[test]
    fn same_group_interruption_filters_before_priority_arbitration() {
        let skeleton = skeleton_runtime();
        let a = clip("a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let b = clip("b", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let c = clip("c", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let d = clip("d", 1.0, 1.0, true, 0.0, 30.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "same-group".to_owned(),
            entry_state: "A".to_owned(),
            parameters: vec![bool_parameter("go_b"), bool_parameter("interrupt")],
            states: vec![
                graph_state("A", "group.ycd@a"),
                graph_state("B", "group.ycd@b"),
                graph_state("C", "group.ycd@c"),
                graph_state("D", "group.ycd@d"),
            ],
            transitions: vec![
                bool_transition(
                    "A",
                    "B",
                    "go_b",
                    1.0,
                    0,
                    Some("Locomotion"),
                    AnimationTransitionInterruptionPolicy::SameGroup,
                ),
                bool_transition(
                    "B",
                    "C",
                    "interrupt",
                    0.5,
                    100,
                    Some("airborne"),
                    AnimationTransitionInterruptionPolicy::Any,
                ),
                bool_transition(
                    "B",
                    "D",
                    "interrupt",
                    0.5,
                    1,
                    Some("LOCOMOTION"),
                    AnimationTransitionInterruptionPolicy::Any,
                ),
            ],
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("group.ycd@a", a),
                ("group.ycd@b", b),
                ("group.ycd@c", c),
                ("group.ycd@d", d),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.set_bool(&graph, "go_b", true).unwrap();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        let before = evaluation.local_pose.clone();
        instance.set_bool(&graph, "interrupt", true).unwrap();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        let transition = evaluation.transition.unwrap();
        assert_eq!(transition.from_state, graph.state_index("B").unwrap());
        assert_eq!(transition.to_state, graph.state_index("D").unwrap());
        assert_eq!(evaluation.local_pose, before);
    }

    #[test]
    fn any_interruption_preserves_current_blended_pose_across_groups() {
        let skeleton = skeleton_runtime();
        let a = clip("a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let b = clip("b", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let c = clip("c", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "any".to_owned(),
            entry_state: "A".to_owned(),
            parameters: vec![bool_parameter("go_b"), bool_parameter("go_c")],
            states: vec![
                graph_state("A", "any.ycd@a"),
                graph_state("B", "any.ycd@b"),
                graph_state("C", "any.ycd@c"),
            ],
            transitions: vec![
                bool_transition(
                    "A",
                    "B",
                    "go_b",
                    1.0,
                    0,
                    Some("locomotion"),
                    AnimationTransitionInterruptionPolicy::Any,
                ),
                bool_transition(
                    "B",
                    "C",
                    "go_c",
                    0.5,
                    0,
                    Some("airborne"),
                    AnimationTransitionInterruptionPolicy::Never,
                ),
            ],
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("any.ycd@a", a),
                ("any.ycd@b", b),
                ("any.ycd@c", c),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.set_bool(&graph, "go_b", true).unwrap();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        let before = evaluation.local_pose.clone();
        assert!(before[1].translation[0] > 0.0);
        assert!(before[1].translation[0] < 10.0);
        instance.set_bool(&graph, "go_c", true).unwrap();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        let transition = evaluation.transition.unwrap();
        assert_eq!(transition.from_state, graph.state_index("B").unwrap());
        assert_eq!(transition.to_state, graph.state_index("C").unwrap());
        assert_eq!(evaluation.local_pose, before);
    }

    #[test]
    fn explicit_blend_interruption_uses_last_evaluated_pose_as_source() {
        let skeleton = skeleton_runtime();
        let a = clip("a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let b = clip("b", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let c = clip("c", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "explicit-interrupt".to_owned(),
            entry_state: "A".to_owned(),
            parameters: Vec::new(),
            states: vec![
                graph_state("A", "explicit.ycd@a"),
                graph_state("B", "explicit.ycd@b"),
                graph_state("C", "explicit.ycd@c"),
            ],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("explicit.ycd@a", a),
                ("explicit.ycd@b", b),
                ("explicit.ycd@c", c),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        instance.blend_to_state(&graph, "B", 1.0).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        let before = evaluation.local_pose.clone();
        instance.blend_to_state(&graph, "C", 0.5).unwrap();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        let transition = evaluation.transition.unwrap();
        assert_eq!(transition.from_state, graph.state_index("B").unwrap());
        assert_eq!(transition.to_state, graph.state_index("C").unwrap());
        assert_eq!(evaluation.local_pose, before);
    }

    #[test]
    fn same_group_interruption_requires_authored_group() {
        let skeleton = skeleton_runtime();
        let a = clip("a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let b = clip("b", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "bad-group".to_owned(),
            entry_state: "A".to_owned(),
            parameters: vec![bool_parameter("go")],
            states: vec![
                graph_state("A", "bad-group.ycd@a"),
                graph_state("B", "bad-group.ycd@b"),
            ],
            transitions: vec![bool_transition(
                "A",
                "B",
                "go",
                0.2,
                0,
                None,
                AnimationTransitionInterruptionPolicy::SameGroup,
            )],
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let result = CompiledAnimationGraph::compile(definition, &skeleton, |reference| match reference {
            "bad-group.ycd@a" => Ok(a.clone()),
            "bad-group.ycd@b" => Ok(b.clone()),
            _ => Err("unknown".to_owned()),
        });
        assert!(result
            .unwrap_err()
            .contains("uses same_group interruption without a group"));
    }

    #[test]
    fn transition_v2_schema_defaults_remain_backward_compatible() {
        let bytes = br#"{
            "$schema": "northstar.animation_graph.v1",
            "name": "compat",
            "entry_state": "A",
            "parameters": [],
            "states": [
                {
                    "name": "A",
                    "motion": {"clip": {"clip_ref": "compat.ycd@a", "speed": 1.0, "sync_group": null}},
                    "speed": 1.0,
                    "root_motion": "disabled"
                },
                {
                    "name": "B",
                    "motion": {"clip": {"clip_ref": "compat.ycd@b", "speed": 1.0, "sync_group": null}},
                    "speed": 1.0,
                    "root_motion": "disabled"
                }
            ],
            "transitions": [{
                "from": "A",
                "to": "B",
                "conditions": [],
                "exit_time_normalized": null,
                "blend_seconds": 0.2
            }],
            "layers": [],
            "root_motion_joint_tag": null
        }"#;
        let decoded = decode_animation_graph_asset_v1(bytes).unwrap();
        assert!(decoded.sync_groups.is_empty());
        assert_eq!(decoded.transitions[0].priority, 0);
        assert_eq!(decoded.transitions[0].group, None);
        assert_eq!(
            decoded.transitions[0].interruption,
            AnimationTransitionInterruptionPolicy::Never
        );
    }

    #[test]
    fn blend_tree_interpolates_between_neighbor_samples() {
        let skeleton = skeleton_runtime();
        let idle = clip("idle", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let run = clip("run", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "blend".to_owned(),
            entry_state: "Move".to_owned(),
            parameters: vec![AnimationGraphParameterDefinition {
                name: "speed".to_owned(),
                default: AnimationGraphParameterValue::Float(0.5),
            }],
            states: vec![AnimationGraphStateDefinition {
                name: "Move".to_owned(),
                motion: AnimationMotionDefinition::Blend1D(AnimationBlendTree1DDefinition {
                    parameter: "speed".to_owned(),
                    samples: vec![
                        AnimationBlendSample1D {
                            threshold: 0.0,
                            clip_ref: "move.ycd@idle".to_owned(),
                            speed: 1.0,
                        },
                        AnimationBlendSample1D {
                            threshold: 1.0,
                            clip_ref: "move.ycd@run".to_owned(),
                            speed: 1.0,
                        },
                    ],
                    sync_group: None,
                }),
                speed: 1.0,
                root_motion: AnimationRootMotionMode::Disabled,
            }],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[("move.ycd@idle", idle), ("move.ycd@run", run)],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        // idle upper X=0, run upper X=5 at t=.5, blend alpha=.5 => 2.5.
        assert!((evaluation.local_pose[1].translation[0] - 2.5).abs() < 1.0e-4);
    }

    #[test]
    fn override_layer_respects_compiled_descendant_bone_mask() {
        let skeleton = skeleton_runtime();
        let base = clip("base", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let upper = clip("upper", 1.0, 1.0, true, 10.0, 20.0, Vec::new());
        let mut definition = base_graph(AnimationGraphStateDefinition {
            name: "Base".to_owned(),
            motion: clip_motion("base.ycd@base"),
            speed: 1.0,
            root_motion: AnimationRootMotionMode::Disabled,
        });
        definition.layers.push(AnimationGraphLayerDefinition {
            name: "UpperBody".to_owned(),
            motion: clip_motion("upper.ycd@aim"),
            mode: AnimationLayerBlendMode::Override,
            weight: 1.0,
            weight_parameter: None,
            mask: Some(AnimationBoneMaskDefinition {
                roots: vec![AnimationBoneMaskRoot {
                    joint_tag: 20,
                    weight: 1.0,
                    include_descendants: true,
                }],
            }),
            event_weight_threshold: 0.5,
        });
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[("base.ycd@base", base), ("upper.ycd@aim", upper)],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert!(evaluation.local_pose[0].translation[0].abs() < 1.0e-6);
        assert!((evaluation.local_pose[1].translation[0] - 10.0).abs() < 1.0e-4);
    }

    #[test]
    fn sync_group_maps_same_phase_across_different_clip_durations() {
        let skeleton = skeleton_runtime();
        let short = clip("short", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let long = clip("long", 2.0, 0.5, true, 0.0, 10.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "sync".to_owned(),
            entry_state: "Move".to_owned(),
            parameters: vec![AnimationGraphParameterDefinition {
                name: "blend".to_owned(),
                default: AnimationGraphParameterValue::Float(0.5),
            }],
            states: vec![AnimationGraphStateDefinition {
                name: "Move".to_owned(),
                motion: AnimationMotionDefinition::Blend1D(AnimationBlendTree1DDefinition {
                    parameter: "blend".to_owned(),
                    samples: vec![
                        AnimationBlendSample1D {
                            threshold: 0.0,
                            clip_ref: "sync.ycd@short".to_owned(),
                            speed: 1.0,
                        },
                        AnimationBlendSample1D {
                            threshold: 1.0,
                            clip_ref: "sync.ycd@long".to_owned(),
                            speed: 1.0,
                        },
                    ],
                    sync_group: Some("locomotion".to_owned()),
                }),
                speed: 1.0,
                root_motion: AnimationRootMotionMode::Disabled,
            }],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[("sync.ycd@short", short), ("sync.ycd@long", long)],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        // Both clips are sampled at normalized phase .5, therefore each produces upper X=5.
        assert!((evaluation.local_pose[1].translation[0] - 5.0).abs() < 1.0e-4);
    }

    #[test]
    fn graph_timeline_events_are_exactly_once_across_loop_boundary() {
        let skeleton = skeleton_runtime();
        let event_clip = clip(
            "events",
            1.0,
            1.0,
            true,
            0.0,
            0.0,
            vec![
                AnimationEvent::new(0.0, "foot.left.contact"),
                AnimationEvent::new(0.5, "foot.right.contact"),
            ],
        );
        let graph = compile_with_clips(
            base_graph(AnimationGraphStateDefinition {
                name: "Move".to_owned(),
                motion: clip_motion("events.ycd@move"),
                speed: 1.0,
                root_motion: AnimationRootMotionMode::Disabled,
            }),
            &skeleton,
            &[("events.ycd@move", event_clip)],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        assert_eq!(evaluation.events.len(), 1);
        assert_eq!(evaluation.events[0].event_index, 0);
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert!(evaluation.events.is_empty());
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert_eq!(evaluation.events.len(), 1);
        assert_eq!(evaluation.events[0].event_index, 1);
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert!(evaluation.events.is_empty());
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert_eq!(evaluation.events.len(), 1);
        assert_eq!(evaluation.events[0].event_index, 0);
        assert_eq!(evaluation.events[0].loop_index, 1);
    }

    #[test]
    fn root_motion_is_unwrapped_across_loop_and_removed_from_pose() {
        let skeleton = skeleton_runtime();
        let locomotion = clip("root", 1.0, 1.0, true, 1.0, 0.0, Vec::new());
        let mut definition = base_graph(AnimationGraphStateDefinition {
            name: "Move".to_owned(),
            motion: clip_motion("root.ycd@move"),
            speed: 1.0,
            root_motion: AnimationRootMotionMode::ExtractAndRemove,
        });
        definition.root_motion_joint_tag = Some(10);
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[("root.ycd@move", locomotion)],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        assert!(!evaluation.root_motion.valid);
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert!(evaluation.root_motion.valid);
        assert!((evaluation.root_motion.translation[0] - 0.25).abs() < 1.0e-3);
        assert!(evaluation.local_pose[0].translation[0].abs() < 1.0e-6);
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.05, &mut evaluation).unwrap();
        assert!(evaluation.root_motion.valid);
        assert!((evaluation.root_motion.translation[0] - 0.05).abs() < 2.0e-3);
        assert!(evaluation.local_pose[0].translation[0].abs() < 1.0e-6);
    }

    #[test]
    fn explicit_state_blend_preserves_phase_for_matching_sync_group() {
        let skeleton = skeleton_runtime();
        let short = clip("short", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let long = clip("long", 2.0, 0.5, true, 0.0, 20.0, Vec::new());
        let mut short_motion = AnimationClipMotionDefinition::new("syncstate.ycd@short");
        short_motion.sync_group = Some("locomotion".to_owned());
        let mut long_motion = AnimationClipMotionDefinition::new("syncstate.ycd@long");
        long_motion.sync_group = Some("locomotion".to_owned());
        let definition = AnimationGraphDefinition {
            name: "explicit-sync".to_owned(),
            entry_state: "Short".to_owned(),
            parameters: Vec::new(),
            states: vec![
                AnimationGraphStateDefinition {
                    name: "Short".to_owned(),
                    motion: AnimationMotionDefinition::Clip(short_motion),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
                AnimationGraphStateDefinition {
                    name: "Long".to_owned(),
                    motion: AnimationMotionDefinition::Clip(long_motion),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
            ],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("syncstate.ycd@short", short),
                ("syncstate.ycd@long", long),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.blend_to_state(&graph, "Long", 0.0).unwrap();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        // Short was at phase .5; Long is sampled at t=1.0 => upper X=10.
        assert!((evaluation.local_pose[1].translation[0] - 10.0).abs() < 1.0e-3);
    }

    #[test]
    fn compiler_rejects_synchronized_blend_with_divergent_sample_speeds() {
        let skeleton = skeleton_runtime();
        let a = clip("a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let b = clip("b", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "bad-sync".to_owned(),
            entry_state: "Move".to_owned(),
            parameters: vec![AnimationGraphParameterDefinition {
                name: "speed".to_owned(),
                default: AnimationGraphParameterValue::Float(0.0),
            }],
            states: vec![AnimationGraphStateDefinition {
                name: "Move".to_owned(),
                motion: AnimationMotionDefinition::Blend1D(AnimationBlendTree1DDefinition {
                    parameter: "speed".to_owned(),
                    samples: vec![
                        AnimationBlendSample1D {
                            threshold: 0.0,
                            clip_ref: "bad.ycd@a".to_owned(),
                            speed: 1.0,
                        },
                        AnimationBlendSample1D {
                            threshold: 1.0,
                            clip_ref: "bad.ycd@b".to_owned(),
                            speed: 2.0,
                        },
                    ],
                    sync_group: Some("locomotion".to_owned()),
                }),
                speed: 1.0,
                root_motion: AnimationRootMotionMode::Disabled,
            }],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let result = CompiledAnimationGraph::compile(definition, &skeleton, |reference| {
            match reference {
                "bad.ycd@a" => Ok(a.clone()),
                "bad.ycd@b" => Ok(b.clone()),
                _ => Err("unknown".to_owned()),
            }
        });
        assert!(result
            .unwrap_err()
            .contains("requires equal sample speeds"));
    }
    #[test]
    fn authored_graph_asset_roundtrips_and_compiled_store_shares_arc() {
        let skeleton = skeleton_runtime();
        let idle = clip("idle", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let definition = base_graph(AnimationGraphStateDefinition {
            name: "Idle".to_owned(),
            motion: clip_motion("test.ycd@idle"),
            speed: 1.0,
            root_motion: AnimationRootMotionMode::Disabled,
        });
        let bytes = encode_animation_graph_asset_v1(&definition).expect("encode graph asset");
        let decoded = decode_animation_graph_asset_v1(&bytes).expect("decode graph asset");
        assert_eq!(decoded, definition);

        let store = CompiledAnimationGraphStore::new();
        let first = store
            .load_or_compile(
                "graphs/test.animation_graph.json",
                &skeleton,
                |_| Ok(bytes.clone()),
                |reference| {
                    assert_eq!(reference, "test.ycd@idle");
                    Ok(idle.clone())
                },
            )
            .expect("first graph compile");
        let second = store
            .load_or_compile(
                "GRAPHS\\TEST.ANIMATION_GRAPH.JSON",
                &skeleton,
                |_| panic!("cache hit must not reload authored graph bytes"),
                |_| panic!("cache hit must not reload clips"),
            )
            .expect("cached graph");
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            store.stats().unwrap(),
            CompiledAnimationGraphStoreStats {
                compiled_graphs: 1,
                asset_paths: 1,
            }
        );
        assert_eq!(store.invalidate_asset_path("graphs/test.animation_graph.json").unwrap(), 1);
        assert_eq!(store.stats().unwrap().compiled_graphs, 0);
    }

    #[test]
    fn compiled_graph_store_specializes_by_skeleton_contract() {
        let skeleton_a = skeleton_runtime();
        let skeleton_b = AnimationSkeletonRuntime::compile(
            &skeleton_metadata(),
            Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)).to_cols_array(),
        )
        .expect("compile alternate skeleton contract");
        assert_ne!(skeleton_a.compatibility_key(), skeleton_b.compatibility_key());

        let idle = clip("idle", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let definition = base_graph(AnimationGraphStateDefinition {
            name: "Idle".to_owned(),
            motion: clip_motion("test.ycd@idle"),
            speed: 1.0,
            root_motion: AnimationRootMotionMode::Disabled,
        });
        let bytes = encode_animation_graph_asset_v1(&definition).unwrap();
        let store = CompiledAnimationGraphStore::new();
        let graph_a = store
            .load_or_compile(
                "graphs/test.animation_graph.json",
                &skeleton_a,
                |_| Ok(bytes.clone()),
                |_| Ok(idle.clone()),
            )
            .unwrap();
        let graph_b = store
            .load_or_compile(
                "graphs/test.animation_graph.json",
                &skeleton_b,
                |_| Ok(bytes.clone()),
                |_| Ok(idle.clone()),
            )
            .unwrap();
        assert!(!Arc::ptr_eq(&graph_a, &graph_b));
        assert_eq!(store.stats().unwrap().compiled_graphs, 2);
        assert_eq!(store.stats().unwrap().asset_paths, 1);
    }

    fn intent_for(
        kind: newengine_animation_api::AnimationIntentKind,
        parameters: serde_json::Value,
    ) -> newengine_animation_api::AnimationIntentDtoV1 {
        newengine_animation_api::AnimationIntentDtoV1 {
            entity: Default::default(),
            intent: kind,
            graph: Some(newengine_animation_api::AnimationGraphRef("intent.graph".to_owned())),
            clip: None,
            task: None,
            tags: Vec::new(),
            parameters,
        }
    }

    #[test]
    fn set_parameter_intent_is_typed_and_atomic() {
        let skeleton = skeleton_runtime();
        let idle = clip("idle", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "intent.graph".to_owned(),
            entry_state: "Idle".to_owned(),
            parameters: vec![
                AnimationGraphParameterDefinition {
                    name: "moving".to_owned(),
                    default: AnimationGraphParameterValue::Bool(false),
                },
                AnimationGraphParameterDefinition {
                    name: "speed".to_owned(),
                    default: AnimationGraphParameterValue::Float(0.0),
                },
            ],
            states: vec![AnimationGraphStateDefinition {
                name: "Idle".to_owned(),
                motion: clip_motion("intent.ycd@idle"),
                speed: 1.0,
                root_motion: AnimationRootMotionMode::Disabled,
            }],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(definition, &skeleton, &[("intent.ycd@idle", idle)]);
        let mut instance = AnimationGraphInstance::new(&graph);

        let invalid = intent_for(
            newengine_animation_api::AnimationIntentKind::SetParameter,
            serde_json::json!({"moving": true, "speed": "fast"}),
        );
        assert!(apply_animation_intent_to_graph_instance(&graph, &mut instance, &invalid).is_err());
        assert_eq!(
            instance.parameter(&graph, "moving"),
            Some(AnimationGraphParameterValue::Bool(false))
        );
        assert_eq!(
            instance.parameter(&graph, "speed"),
            Some(AnimationGraphParameterValue::Float(0.0))
        );

        let valid = intent_for(
            newengine_animation_api::AnimationIntentKind::SetParameter,
            serde_json::json!({"moving": true, "speed": 0.75}),
        );
        assert_eq!(
            apply_animation_intent_to_graph_instance(&graph, &mut instance, &valid).unwrap(),
            AnimationGraphIntentApplyResult::SetParameters { count: 2 }
        );
        assert_eq!(
            instance.parameter(&graph, "moving"),
            Some(AnimationGraphParameterValue::Bool(true))
        );
        assert_eq!(
            instance.parameter(&graph, "speed"),
            Some(AnimationGraphParameterValue::Float(0.75))
        );
    }

    #[test]
    fn blend_to_state_intent_targets_compiled_graph_instance() {
        let skeleton = skeleton_runtime();
        let idle = clip("idle", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let walk = clip("walk", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let definition = AnimationGraphDefinition {
            name: "intent.graph".to_owned(),
            entry_state: "Idle".to_owned(),
            parameters: Vec::new(),
            states: vec![
                AnimationGraphStateDefinition {
                    name: "Idle".to_owned(),
                    motion: clip_motion("intent.ycd@idle"),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
                AnimationGraphStateDefinition {
                    name: "Walk".to_owned(),
                    motion: clip_motion("intent.ycd@walk"),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
            ],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("intent.ycd@idle", idle),
                ("intent.ycd@walk", walk),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let blend = intent_for(
            newengine_animation_api::AnimationIntentKind::BlendToState,
            serde_json::json!({"state":"walk", "blend_seconds":0.2}),
        );
        let result =
            apply_animation_intent_to_graph_instance(&graph, &mut instance, &blend).unwrap();
        assert_eq!(
            result,
            AnimationGraphIntentApplyResult::BlendToState {
                state_index: graph.state_index("Walk").unwrap(),
                blend_seconds: 0.2,
            }
        );
        assert!(instance.transition().is_some());

        let wrong_graph = newengine_animation_api::AnimationIntentDtoV1 {
            graph: Some(newengine_animation_api::AnimationGraphRef("other.graph".to_owned())),
            ..blend
        };
        assert!(apply_animation_intent_to_graph_instance(&graph, &mut instance, &wrong_graph)
            .unwrap_err()
            .contains("instance owns"));
    }

    #[test]
    fn compiled_graph_store_separates_product_binding_variants() {
        let skeleton = skeleton_runtime();
        let definition = base_graph(AnimationGraphStateDefinition {
            name: "Idle".to_owned(),
            motion: clip_motion("slot://idle"),
            speed: 1.0,
            root_motion: AnimationRootMotionMode::Disabled,
        });
        let bytes = encode_animation_graph_asset_v1(&definition).unwrap();
        let idle_a = clip("idle-a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let idle_b = clip("idle-b", 1.0, 1.0, true, 0.0, 1.0, Vec::new());
        let store = CompiledAnimationGraphStore::new();

        let graph_a = store
            .load_or_compile_with_variant(
                "graphs/product.animation_graph.json",
                &skeleton,
                11,
                |_| Ok(bytes.clone()),
                |_| Ok(idle_a.clone()),
            )
            .unwrap();
        let graph_b = store
            .load_or_compile_with_variant(
                "graphs/product.animation_graph.json",
                &skeleton,
                12,
                |_| Ok(bytes.clone()),
                |_| Ok(idle_b.clone()),
            )
            .unwrap();

        assert!(!Arc::ptr_eq(&graph_a, &graph_b));
        assert_eq!(
            store.stats().unwrap(),
            CompiledAnimationGraphStoreStats {
                compiled_graphs: 2,
                asset_paths: 1,
            }
        );
    }


    #[test]
    fn marker_sync_transition_remaps_semantic_interval_instead_of_normalized_phase() {
        let skeleton = skeleton_runtime();
        let source = clip(
            "walk",
            1.0,
            1.0,
            true,
            0.0,
            10.0,
            marker_events(0.1, 0.6),
        );
        let target = clip(
            "run",
            1.0,
            1.0,
            true,
            0.0,
            10.0,
            marker_events(0.2, 0.8),
        );
        let mut source_motion = AnimationClipMotionDefinition::new("marker.ycd@walk");
        source_motion.sync_group = Some("locomotion".to_owned());
        let mut target_motion = AnimationClipMotionDefinition::new("marker.ycd@run");
        target_motion.sync_group = Some("LOCOMOTION".to_owned());
        let definition = AnimationGraphDefinition {
            name: "marker-transition".to_owned(),
            entry_state: "Walk".to_owned(),
            parameters: Vec::new(),
            states: vec![
                AnimationGraphStateDefinition {
                    name: "Walk".to_owned(),
                    motion: AnimationMotionDefinition::Clip(source_motion),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
                AnimationGraphStateDefinition {
                    name: "Run".to_owned(),
                    motion: AnimationMotionDefinition::Clip(target_motion),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
            ],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: vec![locomotion_sync_group()],
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[("marker.ycd@walk", source), ("marker.ycd@run", target)],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.10, &mut evaluation).unwrap();
        instance.blend_to_state(&graph, "Run", 0.0).unwrap();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        // Walk t=.35 is halfway through left(.1)->right(.6). Run's matching interval is
        // left(.2)->right(.8), therefore semantic remapping enters Run at t=.5, not t=.35.
        assert!((evaluation.local_pose[1].translation[0] - 5.0).abs() < 1.0e-4);
    }

    #[test]
    fn marker_sync_wrap_interval_remaps_across_cycle_boundary() {
        let skeleton = skeleton_runtime();
        let source = clip(
            "walk-wrap",
            1.0,
            1.0,
            true,
            0.0,
            10.0,
            marker_events(0.2, 0.7),
        );
        let target = clip(
            "run-wrap",
            1.0,
            1.0,
            true,
            0.0,
            10.0,
            marker_events(0.1, 0.6),
        );
        let mut source_motion = AnimationClipMotionDefinition::new("wrap.ycd@walk");
        source_motion.sync_group = Some("locomotion".to_owned());
        let mut target_motion = AnimationClipMotionDefinition::new("wrap.ycd@run");
        target_motion.sync_group = Some("locomotion".to_owned());
        let definition = AnimationGraphDefinition {
            name: "marker-wrap".to_owned(),
            entry_state: "Walk".to_owned(),
            parameters: Vec::new(),
            states: vec![
                AnimationGraphStateDefinition {
                    name: "Walk".to_owned(),
                    motion: AnimationMotionDefinition::Clip(source_motion),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
                AnimationGraphStateDefinition {
                    name: "Run".to_owned(),
                    motion: AnimationMotionDefinition::Clip(target_motion),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
            ],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: vec![locomotion_sync_group()],
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[("wrap.ycd@walk", source), ("wrap.ycd@run", target)],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        for dt in [0.25, 0.25, 0.25, 0.20] {
            instance.evaluate(&graph, &skeleton, dt, &mut evaluation).unwrap();
        }
        instance.blend_to_state(&graph, "Run", 0.0).unwrap();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        // Walk .95 is halfway through right(.7)->left(1.2). Run maps the same semantic interval
        // to right(.6)->left(1.1), which is t=.85 after wrapping into the target cycle.
        assert!((evaluation.local_pose[1].translation[0] - 8.5).abs() < 1.0e-4);
    }

    #[test]
    fn marker_sync_blend1d_remaps_each_sample_from_semantic_leader() {
        let skeleton = skeleton_runtime();
        let leader = clip(
            "leader",
            1.0,
            1.0,
            true,
            0.0,
            10.0,
            marker_events(0.1, 0.6),
        );
        let follower = clip(
            "follower",
            1.0,
            1.0,
            true,
            0.0,
            10.0,
            marker_events(0.2, 0.8),
        );
        let definition = AnimationGraphDefinition {
            name: "marker-blend".to_owned(),
            entry_state: "Move".to_owned(),
            parameters: vec![AnimationGraphParameterDefinition {
                name: "blend".to_owned(),
                default: AnimationGraphParameterValue::Float(0.5),
            }],
            states: vec![AnimationGraphStateDefinition {
                name: "Move".to_owned(),
                motion: AnimationMotionDefinition::Blend1D(AnimationBlendTree1DDefinition {
                    parameter: "blend".to_owned(),
                    samples: vec![
                        AnimationBlendSample1D {
                            threshold: 0.0,
                            clip_ref: "markerblend.ycd@leader".to_owned(),
                            speed: 1.0,
                        },
                        AnimationBlendSample1D {
                            threshold: 1.0,
                            clip_ref: "markerblend.ycd@follower".to_owned(),
                            speed: 1.0,
                        },
                    ],
                    sync_group: Some("locomotion".to_owned()),
                }),
                speed: 1.0,
                root_motion: AnimationRootMotionMode::Disabled,
            }],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: vec![locomotion_sync_group()],
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("markerblend.ycd@leader", leader),
                ("markerblend.ycd@follower", follower),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.10, &mut evaluation).unwrap();
        // Leader is x=3.5 at t=.35. Follower is marker-remapped to t=.5 => x=5.
        // Blend parameter .5 gives x=4.25. Normalized-phase V1 would have produced x=3.5.
        assert!((evaluation.local_pose[1].translation[0] - 4.25).abs() < 1.0e-4);
    }

    #[test]
    fn marker_sync_layer_uses_base_semantic_interval() {
        let skeleton = skeleton_runtime();
        let base = clip(
            "base-marker",
            1.0,
            1.0,
            true,
            0.0,
            0.0,
            marker_events(0.1, 0.6),
        );
        let layer = clip(
            "layer-marker",
            1.0,
            1.0,
            true,
            0.0,
            10.0,
            marker_events(0.2, 0.8),
        );
        let mut base_motion = AnimationClipMotionDefinition::new("markerlayer.ycd@base");
        base_motion.sync_group = Some("locomotion".to_owned());
        let mut layer_motion = AnimationClipMotionDefinition::new("markerlayer.ycd@layer");
        layer_motion.sync_group = Some("locomotion".to_owned());
        let definition = AnimationGraphDefinition {
            name: "marker-layer".to_owned(),
            entry_state: "Move".to_owned(),
            parameters: Vec::new(),
            states: vec![AnimationGraphStateDefinition {
                name: "Move".to_owned(),
                motion: AnimationMotionDefinition::Clip(base_motion),
                speed: 1.0,
                root_motion: AnimationRootMotionMode::Disabled,
            }],
            transitions: Vec::new(),
            layers: vec![AnimationGraphLayerDefinition {
                name: "Synced".to_owned(),
                motion: AnimationMotionDefinition::Clip(layer_motion),
                mode: AnimationLayerBlendMode::Override,
                weight: 1.0,
                weight_parameter: None,
                mask: None,
                event_weight_threshold: 0.5,
            }],
            sync_groups: vec![locomotion_sync_group()],
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[("markerlayer.ycd@base", base), ("markerlayer.ycd@layer", layer)],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.10, &mut evaluation).unwrap();
        assert!((evaluation.local_pose[1].translation[0] - 5.0).abs() < 1.0e-4);
    }

    #[test]
    fn marker_sync_compiler_rejects_incomplete_marker_track() {
        let skeleton = skeleton_runtime();
        let bad = clip(
            "bad-marker",
            1.0,
            1.0,
            true,
            0.0,
            0.0,
            vec![AnimationEvent::new(0.1, "foot.left.contact")],
        );
        let mut motion = AnimationClipMotionDefinition::new("badmarker.ycd@move");
        motion.sync_group = Some("locomotion".to_owned());
        let definition = AnimationGraphDefinition {
            name: "bad-marker".to_owned(),
            entry_state: "Move".to_owned(),
            parameters: Vec::new(),
            states: vec![AnimationGraphStateDefinition {
                name: "Move".to_owned(),
                motion: AnimationMotionDefinition::Clip(motion),
                speed: 1.0,
                root_motion: AnimationRootMotionMode::Disabled,
            }],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: vec![locomotion_sync_group()],
            root_motion_joint_tag: None,
        };
        let result = CompiledAnimationGraph::compile(definition, &skeleton, |_| Ok(bad.clone()));
        assert!(result
            .unwrap_err()
            .contains("requires exactly one 'foot.right.contact' marker per cycle"));
    }


    #[test]
    fn authored_transition_uses_marker_interval_matching() {
        let skeleton = skeleton_runtime();
        let source = clip(
            "authored-walk",
            1.0,
            1.0,
            true,
            0.0,
            10.0,
            marker_events(0.1, 0.6),
        );
        let target = clip(
            "authored-run",
            1.0,
            1.0,
            true,
            0.0,
            10.0,
            marker_events(0.2, 0.8),
        );
        let mut source_motion = AnimationClipMotionDefinition::new("authmarker.ycd@walk");
        source_motion.sync_group = Some("locomotion".to_owned());
        let mut target_motion = AnimationClipMotionDefinition::new("authmarker.ycd@run");
        target_motion.sync_group = Some("locomotion".to_owned());
        let definition = AnimationGraphDefinition {
            name: "authored-marker-transition".to_owned(),
            entry_state: "Walk".to_owned(),
            parameters: Vec::new(),
            states: vec![
                AnimationGraphStateDefinition {
                    name: "Walk".to_owned(),
                    motion: AnimationMotionDefinition::Clip(source_motion),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
                AnimationGraphStateDefinition {
                    name: "Run".to_owned(),
                    motion: AnimationMotionDefinition::Clip(target_motion),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
            ],
            transitions: vec![AnimationGraphTransitionDefinition {
                from: "Walk".to_owned(),
                to: "Run".to_owned(),
                conditions: Vec::new(),
                exit_time_normalized: None,
                blend_seconds: 0.0,
                priority: 0,
                group: None,
                interruption: AnimationTransitionInterruptionPolicy::Never,
            }],
            layers: Vec::new(),
            sync_groups: vec![locomotion_sync_group()],
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[("authmarker.ycd@walk", source), ("authmarker.ycd@run", target)],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        // Source .25 is 30% through left(.1)->right(.6). Target enters 30% through
        // left(.2)->right(.8): .2 + .6*.3 = .38.
        assert!((evaluation.local_pose[1].translation[0] - 3.8).abs() < 1.0e-4);
        assert_eq!(instance.active_state_index(), graph.state_index("Run").unwrap());
    }

    #[test]
    fn marker_sync_rotation_preserves_unwrapped_target_event_cycle() {
        let skeleton = skeleton_runtime();
        let source = clip(
            "rotation-source",
            1.0,
            1.0,
            true,
            0.0,
            0.0,
            marker_events(0.2, 0.7),
        );
        // File cycle begins with right, while authored semantic order remains left -> right.
        let target = clip(
            "rotation-target",
            1.0,
            1.0,
            true,
            0.0,
            10.0,
            marker_events(0.6, 0.1),
        );
        let mut source_motion = AnimationClipMotionDefinition::new("rotation.ycd@source");
        source_motion.sync_group = Some("locomotion".to_owned());
        let mut target_motion = AnimationClipMotionDefinition::new("rotation.ycd@target");
        target_motion.sync_group = Some("locomotion".to_owned());
        let definition = AnimationGraphDefinition {
            name: "rotation-sync".to_owned(),
            entry_state: "Source".to_owned(),
            parameters: Vec::new(),
            states: vec![
                AnimationGraphStateDefinition {
                    name: "Source".to_owned(),
                    motion: AnimationMotionDefinition::Clip(source_motion),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
                AnimationGraphStateDefinition {
                    name: "Target".to_owned(),
                    motion: AnimationMotionDefinition::Clip(target_motion),
                    speed: 1.0,
                    root_motion: AnimationRootMotionMode::Disabled,
                },
            ],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: vec![locomotion_sync_group()],
            root_motion_joint_tag: None,
        };
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[("rotation.ycd@source", source), ("rotation.ycd@target", target)],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        for dt in [0.25, 0.25, 0.25, 0.20] {
            instance.evaluate(&graph, &skeleton, dt, &mut evaluation).unwrap();
        }
        instance.blend_to_state(&graph, "Target", 0.0).unwrap();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();
        // Source .95 is halfway through right(.7)->left(1.2). In the rotated target the matching
        // occurrence is right(1.1)->left(1.6), so target time is 1.35, not local .35.
        assert!((evaluation.local_pose[1].translation[0] - 3.5).abs() < 1.0e-4);
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert_eq!(evaluation.events.len(), 1);
        let occurrence = evaluation.events[0];
        let clip = graph.clip(occurrence.clip_index).unwrap();
        assert_eq!(clip.events[occurrence.event_index].tag, "foot.left.contact");
        assert_eq!(occurrence.loop_index, 1);
    }


    #[test]
    fn marker_sync_compiler_rejects_non_looped_participant() {
        let skeleton = skeleton_runtime();
        let bad = clip(
            "non-looped-marker",
            1.0,
            1.0,
            false,
            0.0,
            0.0,
            marker_events(0.1, 0.6),
        );
        let mut motion = AnimationClipMotionDefinition::new("nonloop.ycd@move");
        motion.sync_group = Some("locomotion".to_owned());
        let definition = AnimationGraphDefinition {
            name: "non-looped-marker".to_owned(),
            entry_state: "Move".to_owned(),
            parameters: Vec::new(),
            states: vec![AnimationGraphStateDefinition {
                name: "Move".to_owned(),
                motion: AnimationMotionDefinition::Clip(motion),
                speed: 1.0,
                root_motion: AnimationRootMotionMode::Disabled,
            }],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: vec![locomotion_sync_group()],
            root_motion_joint_tag: None,
        };
        let result = CompiledAnimationGraph::compile(definition, &skeleton, |_| Ok(bad.clone()));
        assert!(result.unwrap_err().contains("requires looped clip"));
    }

    #[test]
    fn marker_sync_compiler_rejects_non_cyclic_marker_order() {
        let skeleton = skeleton_runtime();
        let bad = clip(
            "bad-order",
            1.0,
            1.0,
            true,
            0.0,
            0.0,
            vec![
                AnimationEvent::new(0.1, "phase.a"),
                AnimationEvent::new(0.4, "phase.c"),
                AnimationEvent::new(0.7, "phase.b"),
            ],
        );
        let mut motion = AnimationClipMotionDefinition::new("badorder.ycd@move");
        motion.sync_group = Some("cycle".to_owned());
        let definition = AnimationGraphDefinition {
            name: "bad-order".to_owned(),
            entry_state: "Move".to_owned(),
            parameters: Vec::new(),
            states: vec![AnimationGraphStateDefinition {
                name: "Move".to_owned(),
                motion: AnimationMotionDefinition::Clip(motion),
                speed: 1.0,
                root_motion: AnimationRootMotionMode::Disabled,
            }],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: vec![AnimationSyncGroupDefinition {
                name: "cycle".to_owned(),
                markers: vec![
                    "phase.a".to_owned(),
                    "phase.b".to_owned(),
                    "phase.c".to_owned(),
                ],
            }],
            root_motion_joint_tag: None,
        };
        let result = CompiledAnimationGraph::compile(definition, &skeleton, |_| Ok(bad.clone()));
        assert!(result
            .unwrap_err()
            .contains("marker order is not a cyclic rotation"));
    }


    fn blend2d_definition(
        name: &str,
        mode: AnimationBlend2DMode,
        x: f32,
        y: f32,
        samples: Vec<AnimationBlendSample2D>,
        sync_group: Option<&str>,
    ) -> AnimationGraphDefinition {
        AnimationGraphDefinition {
            name: name.to_owned(),
            entry_state: "Move".to_owned(),
            parameters: vec![
                AnimationGraphParameterDefinition {
                    name: "move_x".to_owned(),
                    default: AnimationGraphParameterValue::Float(x),
                },
                AnimationGraphParameterDefinition {
                    name: "move_y".to_owned(),
                    default: AnimationGraphParameterValue::Float(y),
                },
            ],
            states: vec![AnimationGraphStateDefinition {
                name: "Move".to_owned(),
                motion: AnimationMotionDefinition::Blend2D(AnimationBlendTree2DDefinition {
                    parameter_x: "move_x".to_owned(),
                    parameter_y: "move_y".to_owned(),
                    mode,
                    samples,
                    sync_group: sync_group.map(str::to_owned),
                }),
                speed: 1.0,
                root_motion: AnimationRootMotionMode::Disabled,
            }],
            transitions: Vec::new(),
            layers: Vec::new(),
            sync_groups: Vec::new(),
            root_motion_joint_tag: None,
        }
    }

    fn blend2d_sample(x: f32, y: f32, reference: &str) -> AnimationBlendSample2D {
        AnimationBlendSample2D {
            position: [x, y],
            clip_ref: reference.to_owned(),
            speed: 1.0,
        }
    }

    #[test]
    fn blend2d_cartesian_uses_bilinear_weights() {
        let skeleton = skeleton_runtime();
        let a = clip("a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let b = clip("b", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let c = clip("c", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let d = clip("d", 1.0, 1.0, true, 0.0, 30.0, Vec::new());
        let definition = blend2d_definition(
            "cartesian",
            AnimationBlend2DMode::Cartesian,
            0.0,
            0.0,
            vec![
                blend2d_sample(-1.0, -1.0, "cart.ycd@a"),
                blend2d_sample(1.0, -1.0, "cart.ycd@b"),
                blend2d_sample(-1.0, 1.0, "cart.ycd@c"),
                blend2d_sample(1.0, 1.0, "cart.ycd@d"),
            ],
            None,
        );
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("cart.ycd@a", a),
                ("cart.ycd@b", b),
                ("cart.ycd@c", c),
                ("cart.ycd@d", d),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        // At t=.5 the four upper-X samples are 0, 5, 10 and 15. Query (0,0) is 25% each.
        assert!((evaluation.local_pose[1].translation[0] - 7.5).abs() < 1.0e-4);
    }

    #[test]
    fn blend2d_cartesian_clamps_to_domain_boundary() {
        let skeleton = skeleton_runtime();
        let a = clip("a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let b = clip("b", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let c = clip("c", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let d = clip("d", 1.0, 1.0, true, 0.0, 30.0, Vec::new());
        let definition = blend2d_definition(
            "cartesian-clamp",
            AnimationBlend2DMode::Cartesian,
            4.0,
            0.0,
            vec![
                blend2d_sample(-1.0, -1.0, "cartclamp.ycd@a"),
                blend2d_sample(1.0, -1.0, "cartclamp.ycd@b"),
                blend2d_sample(-1.0, 1.0, "cartclamp.ycd@c"),
                blend2d_sample(1.0, 1.0, "cartclamp.ycd@d"),
            ],
            None,
        );
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("cartclamp.ycd@a", a),
                ("cartclamp.ycd@b", b),
                ("cartclamp.ycd@c", c),
                ("cartclamp.ycd@d", d),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        // x clamps to +1; y=0 mixes only the right-edge samples b=5 and d=15.
        assert!((evaluation.local_pose[1].translation[0] - 10.0).abs() < 1.0e-4);
    }

    #[test]
    fn blend2d_directional_combines_angle_and_radial_center_weight() {
        let skeleton = skeleton_runtime();
        let center = clip("center", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let right = clip("right", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let forward = clip("forward", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let left = clip("left", 1.0, 1.0, true, 0.0, 30.0, Vec::new());
        let back = clip("back", 1.0, 1.0, true, 0.0, 40.0, Vec::new());
        let diagonal_half = 0.5_f32 / 2.0_f32.sqrt();
        let definition = blend2d_definition(
            "directional",
            AnimationBlend2DMode::Directional,
            diagonal_half,
            diagonal_half,
            vec![
                blend2d_sample(0.0, 0.0, "dir.ycd@center"),
                blend2d_sample(1.0, 0.0, "dir.ycd@right"),
                blend2d_sample(0.0, 1.0, "dir.ycd@forward"),
                blend2d_sample(-1.0, 0.0, "dir.ycd@left"),
                blend2d_sample(0.0, -1.0, "dir.ycd@back"),
            ],
            None,
        );
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("dir.ycd@center", center),
                ("dir.ycd@right", right),
                ("dir.ycd@forward", forward),
                ("dir.ycd@left", left),
                ("dir.ycd@back", back),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        // Radius=.5 => center=.5. 45 degrees => right=.25 and forward=.25.
        // At t=.5 their upper-X values are 0, 5 and 10 => 3.75.
        assert!((evaluation.local_pose[1].translation[0] - 3.75).abs() < 1.0e-4);
    }

    #[test]
    fn blend2d_triangulated_uses_barycentric_weights() {
        let skeleton = skeleton_runtime();
        let a = clip("a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let b = clip("b", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let c = clip("c", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let definition = blend2d_definition(
            "triangulated",
            AnimationBlend2DMode::Triangulated,
            0.25,
            0.25,
            vec![
                blend2d_sample(0.0, 0.0, "tri.ycd@a"),
                blend2d_sample(1.0, 0.0, "tri.ycd@b"),
                blend2d_sample(0.0, 1.0, "tri.ycd@c"),
            ],
            None,
        );
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[("tri.ycd@a", a), ("tri.ycd@b", b), ("tri.ycd@c", c)],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        // Barycentric weights are .5, .25, .25. At t=.5 values are 0,5,10 => 3.75.
        assert!((evaluation.local_pose[1].translation[0] - 3.75).abs() < 1.0e-4);
    }

    #[test]
    fn blend2d_triangulated_clamps_outside_convex_hull() {
        let skeleton = skeleton_runtime();
        let a = clip("a", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let b = clip("b", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let c = clip("c", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let definition = blend2d_definition(
            "triangulated-clamp",
            AnimationBlend2DMode::Triangulated,
            2.0,
            0.0,
            vec![
                blend2d_sample(0.0, 0.0, "triclamp.ycd@a"),
                blend2d_sample(1.0, 0.0, "triclamp.ycd@b"),
                blend2d_sample(0.0, 1.0, "triclamp.ycd@c"),
            ],
            None,
        );
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("triclamp.ycd@a", a),
                ("triclamp.ycd@b", b),
                ("triclamp.ycd@c", c),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert!((evaluation.local_pose[1].translation[0] - 5.0).abs() < 1.0e-4);
    }

    #[test]
    fn marker_sync_blend2d_remaps_active_samples_from_fixed_leader() {
        let skeleton = skeleton_runtime();
        let leader = clip(
            "leader2d",
            1.0,
            1.0,
            true,
            0.0,
            0.0,
            marker_events(0.1, 0.6),
        );
        let follower = clip(
            "follower2d",
            1.0,
            1.0,
            true,
            0.0,
            10.0,
            marker_events(0.2, 0.8),
        );
        let third = clip(
            "third2d",
            1.0,
            1.0,
            true,
            0.0,
            20.0,
            marker_events(0.1, 0.6),
        );
        let mut definition = blend2d_definition(
            "marker-blend2d",
            AnimationBlend2DMode::Triangulated,
            0.5,
            0.0,
            vec![
                blend2d_sample(0.0, 0.0, "marker2d.ycd@leader"),
                blend2d_sample(1.0, 0.0, "marker2d.ycd@follower"),
                blend2d_sample(0.0, 1.0, "marker2d.ycd@third"),
            ],
            Some("locomotion"),
        );
        definition.sync_groups = vec![locomotion_sync_group()];
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("marker2d.ycd@leader", leader),
                ("marker2d.ycd@follower", follower),
                ("marker2d.ycd@third", third),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.10, &mut evaluation).unwrap();
        // Query is 50/50 leader/follower. Leader t=.35 maps follower to t=.5 through markers.
        // Upper-X is therefore (0 + 5) / 2 = 2.5, not normalized-sync 1.75.
        assert!((evaluation.local_pose[1].translation[0] - 2.5).abs() < 1.0e-4);
    }

    #[test]
    fn blend2d_asset_roundtrip_preserves_mode_and_samples() {
        let definition = blend2d_definition(
            "asset-blend2d",
            AnimationBlend2DMode::Directional,
            0.25,
            -0.5,
            vec![
                blend2d_sample(1.0, 0.0, "asset2d.ycd@right"),
                blend2d_sample(0.0, 1.0, "asset2d.ycd@forward"),
                blend2d_sample(-1.0, 0.0, "asset2d.ycd@left"),
            ],
            Some("locomotion"),
        );
        let encoded = encode_animation_graph_asset_v1(&definition).unwrap();
        let decoded = decode_animation_graph_asset_v1(&encoded).unwrap();
        assert_eq!(decoded, definition);
    }

    #[test]
    fn blend2d_compiler_rejects_incomplete_cartesian_lattice() {
        let skeleton = skeleton_runtime();
        let clip = clip("shared", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let definition = blend2d_definition(
            "bad-cartesian",
            AnimationBlend2DMode::Cartesian,
            0.0,
            0.0,
            vec![
                blend2d_sample(-1.0, -1.0, "bad2d.ycd@a"),
                blend2d_sample(1.0, -1.0, "bad2d.ycd@b"),
                blend2d_sample(-1.0, 1.0, "bad2d.ycd@c"),
            ],
            None,
        );
        let result = CompiledAnimationGraph::compile(definition, &skeleton, |_| Ok(clip.clone()));
        assert!(result.unwrap_err().contains("complete rectangular lattice"));
    }

    #[test]
    fn blend2d_compiler_rejects_collinear_triangulation() {
        let skeleton = skeleton_runtime();
        let clip = clip("shared", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let definition = blend2d_definition(
            "bad-triangle",
            AnimationBlend2DMode::Triangulated,
            0.0,
            0.0,
            vec![
                blend2d_sample(0.0, 0.0, "badtri.ycd@a"),
                blend2d_sample(1.0, 0.0, "badtri.ycd@b"),
                blend2d_sample(2.0, 0.0, "badtri.ycd@c"),
            ],
            None,
        );
        let result = CompiledAnimationGraph::compile(definition, &skeleton, |_| Ok(clip.clone()));
        assert!(result.unwrap_err().contains("collinear"));
    }

    #[test]
    fn blend2d_compiler_rejects_insufficient_directional_ring() {
        let skeleton = skeleton_runtime();
        let clip = clip("shared", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let definition = blend2d_definition(
            "bad-directional",
            AnimationBlend2DMode::Directional,
            0.0,
            0.0,
            vec![
                blend2d_sample(0.0, 0.0, "baddir.ycd@center"),
                blend2d_sample(1.0, 0.0, "baddir.ycd@right"),
                blend2d_sample(-1.0, 0.0, "baddir.ycd@left"),
            ],
            None,
        );
        let result = CompiledAnimationGraph::compile(definition, &skeleton, |_| Ok(clip.clone()));
        assert!(result
            .unwrap_err()
            .contains("at least three non-center directions"));
    }

    #[test]
    fn blend2d_compiler_rejects_duplicate_sample_position() {
        let skeleton = skeleton_runtime();
        let clip = clip("shared", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let definition = blend2d_definition(
            "bad-duplicate",
            AnimationBlend2DMode::Triangulated,
            0.0,
            0.0,
            vec![
                blend2d_sample(0.0, 0.0, "baddup.ycd@a"),
                blend2d_sample(0.0, 0.0, "baddup.ycd@b"),
                blend2d_sample(0.0, 1.0, "baddup.ycd@c"),
            ],
            None,
        );
        let result = CompiledAnimationGraph::compile(definition, &skeleton, |_| Ok(clip.clone()));
        assert!(result.unwrap_err().contains("duplicate sample position"));
    }

    #[test]
    fn blend2d_compiler_rejects_same_parameter_on_both_axes() {
        let skeleton = skeleton_runtime();
        let shared = clip("shared", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let mut definition = blend2d_definition(
            "bad-parameters",
            AnimationBlend2DMode::Triangulated,
            0.0,
            0.0,
            vec![
                blend2d_sample(0.0, 0.0, "badparam.ycd@a"),
                blend2d_sample(1.0, 0.0, "badparam.ycd@b"),
                blend2d_sample(0.0, 1.0, "badparam.ycd@c"),
            ],
            None,
        );
        let AnimationMotionDefinition::Blend2D(tree) = &mut definition.states[0].motion else {
            panic!("expected blend2d");
        };
        tree.parameter_y = "move_x".to_owned();
        let result = CompiledAnimationGraph::compile(definition, &skeleton, |_| Ok(shared.clone()));
        assert!(result.unwrap_err().contains("distinct x/y parameters"));
    }

    #[test]
    fn synchronized_blend2d_requires_equal_sample_speeds() {
        let skeleton = skeleton_runtime();
        let shared = clip("shared", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let mut samples = vec![
            blend2d_sample(0.0, 0.0, "badspeed2d.ycd@a"),
            blend2d_sample(1.0, 0.0, "badspeed2d.ycd@b"),
            blend2d_sample(0.0, 1.0, "badspeed2d.ycd@c"),
        ];
        samples[1].speed = 2.0;
        let definition = blend2d_definition(
            "bad-speed2d",
            AnimationBlend2DMode::Triangulated,
            0.0,
            0.0,
            samples,
            Some("locomotion"),
        );
        let result = CompiledAnimationGraph::compile(definition, &skeleton, |_| Ok(shared.clone()));
        assert!(result
            .unwrap_err()
            .contains("synchronized blend2d requires equal sample speeds"));
    }


    #[test]
    fn blend2d_triangulated_scattered_domain_interpolates_planar_field() {
        let skeleton = skeleton_runtime();
        let clips = [
            clip("p00", 1.0, 1.0, true, 0.0, 0.0, Vec::new()),
            clip("p10", 1.0, 1.0, true, 0.0, 10.0, Vec::new()),
            clip("p11", 1.0, 1.0, true, 0.0, 30.0, Vec::new()),
            clip("p01", 1.0, 1.0, true, 0.0, 20.0, Vec::new()),
            clip("pc", 1.0, 1.0, true, 0.0, 15.0, Vec::new()),
        ];
        let definition = blend2d_definition(
            "triangulated-scattered",
            AnimationBlend2DMode::Triangulated,
            0.8,
            0.7,
            vec![
                blend2d_sample(0.0, 0.0, "scatter.ycd@p00"),
                blend2d_sample(1.0, 0.0, "scatter.ycd@p10"),
                blend2d_sample(1.0, 1.0, "scatter.ycd@p11"),
                blend2d_sample(0.0, 1.0, "scatter.ycd@p01"),
                blend2d_sample(0.5, 0.5, "scatter.ycd@pc"),
            ],
            None,
        );
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("scatter.ycd@p00", clips[0].clone()),
                ("scatter.ycd@p10", clips[1].clone()),
                ("scatter.ycd@p11", clips[2].clone()),
                ("scatter.ycd@p01", clips[3].clone()),
                ("scatter.ycd@pc", clips[4].clone()),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        // Authored endpoint field is x*10 + y*20 => 22 at (.8,.7), sampled at t=.5 => 11.
        assert!((evaluation.local_pose[1].translation[0] - 11.0).abs() < 1.0e-4);
    }

    #[test]
    fn blend2d_event_dominance_is_deterministic_and_non_dominant_cursor_does_not_replay() {
        let skeleton = skeleton_runtime();
        let a = clip(
            "event-a",
            1.0,
            1.0,
            true,
            0.0,
            0.0,
            vec![AnimationEvent::new(0.5, "event.a")],
        );
        let b = clip(
            "event-b",
            1.0,
            1.0,
            true,
            0.0,
            10.0,
            vec![AnimationEvent::new(0.5, "event.b")],
        );
        let c = clip("event-c", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let definition = blend2d_definition(
            "blend2d-events",
            AnimationBlend2DMode::Triangulated,
            0.5,
            0.0,
            vec![
                blend2d_sample(0.0, 0.0, "events2d.ycd@a"),
                blend2d_sample(1.0, 0.0, "events2d.ycd@b"),
                blend2d_sample(0.0, 1.0, "events2d.ycd@c"),
            ],
            None,
        );
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[("events2d.ycd@a", a), ("events2d.ycd@b", b), ("events2d.ycd@c", c)],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert!(evaluation.events.is_empty());
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert_eq!(evaluation.events.len(), 1);
        let occurrence = evaluation.events[0];
        let event_clip = graph.clip(occurrence.clip_index).unwrap();
        assert_eq!(event_clip.events[occurrence.event_index].tag, "event.a");
        // Move dominance to B. Its cursor already crossed t=.5 while non-dominant, so no replay.
        instance.set_float(&graph, "move_x", 0.75).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert!(evaluation.events.is_empty());
    }

    #[test]
    fn blend2d_root_motion_is_weighted_across_active_samples() {
        let skeleton = skeleton_runtime();
        let slow = clip("root-slow", 1.0, 1.0, true, 1.0, 0.0, Vec::new());
        let fast = clip("root-fast", 1.0, 1.0, true, 3.0, 0.0, Vec::new());
        let side = clip("root-side", 1.0, 1.0, true, 1.0, 0.0, Vec::new());
        let mut definition = blend2d_definition(
            "blend2d-root",
            AnimationBlend2DMode::Triangulated,
            0.5,
            0.0,
            vec![
                blend2d_sample(0.0, 0.0, "root2d.ycd@slow"),
                blend2d_sample(1.0, 0.0, "root2d.ycd@fast"),
                blend2d_sample(0.0, 1.0, "root2d.ycd@side"),
            ],
            None,
        );
        definition.states[0].root_motion = AnimationRootMotionMode::Extract;
        definition.root_motion_joint_tag = Some(10);
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("root2d.ycd@slow", slow),
                ("root2d.ycd@fast", fast),
                ("root2d.ycd@side", side),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert!(!evaluation.root_motion.valid);
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert!(evaluation.root_motion.valid);
        // 50% of velocity 1 + 50% of velocity 3 over dt=.25 => .5 units.
        assert!((evaluation.root_motion.translation[0] - 0.5).abs() < 2.0e-3);
    }

    #[test]
    fn blend2d_root_motion_survives_dominance_boundary_without_zero_frame() {
        let skeleton = skeleton_runtime();
        let slow = clip("root-slow", 1.0, 1.0, true, 1.0, 0.0, Vec::new());
        let fast = clip("root-fast", 1.0, 1.0, true, 3.0, 0.0, Vec::new());
        let side = clip("root-side", 1.0, 1.0, true, 1.0, 0.0, Vec::new());
        let mut definition = blend2d_definition(
            "blend2d-root-crossing",
            AnimationBlend2DMode::Triangulated,
            0.49,
            0.0,
            vec![
                blend2d_sample(0.0, 0.0, "rootcross.ycd@slow"),
                blend2d_sample(1.0, 0.0, "rootcross.ycd@fast"),
                blend2d_sample(0.0, 1.0, "rootcross.ycd@side"),
            ],
            None,
        );
        definition.states[0].root_motion = AnimationRootMotionMode::Extract;
        definition.root_motion_joint_tag = Some(10);
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("rootcross.ycd@slow", slow),
                ("rootcross.ycd@fast", fast),
                ("rootcross.ycd@side", side),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.set_float(&graph, "move_x", 0.51).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        assert!(evaluation.root_motion.valid);
        // Current weights .49 slow + .51 fast. Weighted velocity=2.02; dt=.25 => .505.
        assert!((evaluation.root_motion.translation[0] - 0.505).abs() < 2.0e-3);
    }


    fn clip_with_static_upper_rotation(name: &str, angle_radians: f32) -> Arc<AnimationClip> {
        let rotation = newengine_math::Quat::from_rotation_z(angle_radians);
        let upper = JointLocalPose {
            translation: [0.0, 1.0, 0.0],
            rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
            scale: Some([1.0; 3]),
        };
        Arc::new(AnimationClip {
            name: name.to_owned(),
            skeleton_ref: "test.skel".to_owned(),
            source: format!("{name}.test"),
            duration_seconds: 1.0,
            sample_rate_hz: 1.0,
            looped: true,
            joint_tags: vec![10, 20],
            events: Vec::new(),
            poses: vec![pose(0.0, 0.0), upper, pose(0.0, 0.0), upper],
        })
    }

    #[test]
    fn blend2d_pose_rotation_uses_normalized_weighted_quaternion_accumulation() {
        let skeleton = skeleton_runtime();
        let rotations = [
            0.0_f32,
            std::f32::consts::FRAC_PI_3,
            std::f32::consts::FRAC_PI_3 * 2.0,
            std::f32::consts::PI * 5.0 / 6.0,
        ];
        let clips = [
            clip_with_static_upper_rotation("rot-a", rotations[0]),
            clip_with_static_upper_rotation("rot-b", rotations[1]),
            clip_with_static_upper_rotation("rot-c", rotations[2]),
            clip_with_static_upper_rotation("rot-d", rotations[3]),
        ];
        let definition = blend2d_definition(
            "cartesian-rotation",
            AnimationBlend2DMode::Cartesian,
            0.25,
            0.5,
            vec![
                blend2d_sample(-1.0, -1.0, "rot2d.ycd@a"),
                blend2d_sample(1.0, -1.0, "rot2d.ycd@b"),
                blend2d_sample(-1.0, 1.0, "rot2d.ycd@c"),
                blend2d_sample(1.0, 1.0, "rot2d.ycd@d"),
            ],
            None,
        );
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("rot2d.ycd@a", clips[0].clone()),
                ("rot2d.ycd@b", clips[1].clone()),
                ("rot2d.ycd@c", clips[2].clone()),
                ("rot2d.ycd@d", clips[3].clone()),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.0, &mut evaluation).unwrap();

        // Cartesian weights at x=.25/y=.5 on [-1,1]^2.
        let weights = [0.09375_f32, 0.15625, 0.28125, 0.46875];
        let mut expected_z = 0.0_f32;
        let mut expected_w = 0.0_f32;
        for (angle, weight) in rotations.into_iter().zip(weights) {
            let half = angle * 0.5;
            expected_z += half.sin() * weight;
            expected_w += half.cos() * weight;
        }
        let expected = newengine_math::Quat::from_xyzw(0.0, 0.0, expected_z, expected_w)
            .normalize_or_identity();
        let actual_array = evaluation.local_pose[1].rotation;
        let actual = newengine_math::Quat::from_xyzw(
            actual_array[0],
            actual_array[1],
            actual_array[2],
            actual_array[3],
        )
        .normalize_or_identity();
        assert!(
            actual.dot(expected).abs() > 0.999_99,
            "actual={actual:?} expected={expected:?} dot={}",
            actual.dot(expected)
        );
    }

    #[test]
    fn blend2d_directional_wraps_across_tau_seam() {
        let skeleton = skeleton_runtime();
        let right = clip("right-seam", 1.0, 1.0, true, 0.0, 10.0, Vec::new());
        let forward = clip("forward-seam", 1.0, 1.0, true, 0.0, 20.0, Vec::new());
        let left = clip("left-seam", 1.0, 1.0, true, 0.0, 30.0, Vec::new());
        let back = clip("back-seam", 1.0, 1.0, true, 0.0, 40.0, Vec::new());
        let diagonal = 1.0_f32 / 2.0_f32.sqrt();
        let definition = blend2d_definition(
            "directional-seam",
            AnimationBlend2DMode::Directional,
            diagonal,
            -diagonal,
            vec![
                blend2d_sample(1.0, 0.0, "dirseam.ycd@right"),
                blend2d_sample(0.0, 1.0, "dirseam.ycd@forward"),
                blend2d_sample(-1.0, 0.0, "dirseam.ycd@left"),
                blend2d_sample(0.0, -1.0, "dirseam.ycd@back"),
            ],
            None,
        );
        let graph = compile_with_clips(
            definition,
            &skeleton,
            &[
                ("dirseam.ycd@right", right),
                ("dirseam.ycd@forward", forward),
                ("dirseam.ycd@left", left),
                ("dirseam.ycd@back", back),
            ],
        );
        let mut instance = AnimationGraphInstance::new(&graph);
        let mut evaluation = AnimationGraphEvaluation::default();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        instance.evaluate(&graph, &skeleton, 0.25, &mut evaluation).unwrap();
        // -45 degrees is exactly between Back and Right across the 2PI/0 seam.
        // Their upper-X samples at t=.5 are 20 and 5, so the result is 12.5.
        assert!((evaluation.local_pose[1].translation[0] - 12.5).abs() < 1.0e-4);
    }


    #[test]
    fn blend2d_triangulation_is_independent_of_authored_sample_order() {
        let skeleton = skeleton_runtime();
        let p00 = clip("order-p00", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let p10 = clip("order-p10", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let p01 = clip("order-p01", 1.0, 1.0, true, 0.0, 0.0, Vec::new());
        let p11 = clip("order-p11", 1.0, 1.0, true, 0.0, 40.0, Vec::new());
        let authored_a = vec![
            blend2d_sample(0.0, 0.0, "order.ycd@p00"),
            blend2d_sample(1.0, 0.0, "order.ycd@p10"),
            blend2d_sample(0.0, 1.0, "order.ycd@p01"),
            blend2d_sample(1.0, 1.0, "order.ycd@p11"),
        ];
        let authored_b = vec![
            blend2d_sample(1.0, 1.0, "order.ycd@p11"),
            blend2d_sample(0.0, 1.0, "order.ycd@p01"),
            blend2d_sample(1.0, 0.0, "order.ycd@p10"),
            blend2d_sample(0.0, 0.0, "order.ycd@p00"),
        ];
        let clips = [
            ("order.ycd@p00", p00),
            ("order.ycd@p10", p10),
            ("order.ycd@p01", p01),
            ("order.ycd@p11", p11),
        ];
        let graph_a = compile_with_clips(
            blend2d_definition(
                "order-a",
                AnimationBlend2DMode::Triangulated,
                0.25,
                0.75,
                authored_a,
                None,
            ),
            &skeleton,
            &clips,
        );
        let graph_b = compile_with_clips(
            blend2d_definition(
                "order-b",
                AnimationBlend2DMode::Triangulated,
                0.25,
                0.75,
                authored_b,
                None,
            ),
            &skeleton,
            &clips,
        );
        let mut instance_a = AnimationGraphInstance::new(&graph_a);
        let mut instance_b = AnimationGraphInstance::new(&graph_b);
        let mut evaluation_a = AnimationGraphEvaluation::default();
        let mut evaluation_b = AnimationGraphEvaluation::default();
        for dt in [0.25, 0.25] {
            instance_a.evaluate(&graph_a, &skeleton, dt, &mut evaluation_a).unwrap();
            instance_b.evaluate(&graph_b, &skeleton, dt, &mut evaluation_b).unwrap();
        }
        assert!((evaluation_a.local_pose[1].translation[0]
            - evaluation_b.local_pose[1].translation[0])
            .abs()
            < 1.0e-5);
    }

}
