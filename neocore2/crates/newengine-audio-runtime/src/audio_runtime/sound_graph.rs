#[derive(Clone, Debug)]
struct SoundGraphVoicePlan {
    clip_name: String,
    gain: f32,
    pitch: f32,
    label: String,
}

struct SoundGraphEvalContext<'a> {
    canonical: &'a str,
    seed: u64,
    scope_id: Option<u64>,
    parameters: &'a AudioParameterSet,
}

impl AudioRuntimeState {
    fn evaluate_sound_graph(
        &mut self,
        canonical: &str,
        graph: &newengine_asset_format_nef8::YscdSoundGraph,
        parameters: &AudioParameterSet,
        seed: u64,
        scope_id: Option<u64>,
    ) -> Result<Vec<SoundGraphVoicePlan>, String> {
        let nodes = graph
            .nodes
            .iter()
            .map(|node| (node.id().trim().to_ascii_lowercase(), node))
            .collect::<HashMap<_, _>>();
        let ctx = SoundGraphEvalContext {
            canonical,
            seed,
            scope_id: scope_id.filter(|id| *id != 0),
            parameters,
        };
        let mut stack = Vec::<String>::new();
        let mut staged_sequences = HashMap::<String, u64>::new();
        let mut plans = self.eval_sound_graph_voice_node(
            graph.root.as_str(),
            &nodes,
            &ctx,
            &mut stack,
            &mut staged_sequences,
        )?;
        if plans.is_empty() {
            return Err(format!(
                "YSCD SoundGraph '{}' evaluated to no logical voices",
                canonical
            ));
        }
        if plans.len() > 64 {
            return Err(format!(
                "YSCD SoundGraph '{}' emitted {} voices; max is 64",
                canonical,
                plans.len()
            ));
        }
        for plan in &mut plans {
            plan.gain = sanitize_gain(plan.gain);
            plan.pitch = sanitize_speed(plan.pitch);
        }
        for (key, next) in staged_sequences {
            self.sound_graph_sequences.insert(key, next);
        }
        Ok(plans)
    }

    fn eval_sound_graph_voice_node<'a>(
        &mut self,
        node_id: &str,
        nodes: &HashMap<String, &'a newengine_asset_format_nef8::YscdSoundGraphNode>,
        ctx: &SoundGraphEvalContext<'_>,
        stack: &mut Vec<String>,
        staged_sequences: &mut HashMap<String, u64>,
    ) -> Result<Vec<SoundGraphVoicePlan>, String> {
        let key = node_id.trim().to_ascii_lowercase();
        if stack.len() >= 64 {
            return Err("YSCD SoundGraph runtime traversal exceeded depth 64".to_owned());
        }
        if stack.iter().any(|entry| entry == &key) {
            return Err(format!(
                "YSCD SoundGraph runtime cycle detected at node '{}'",
                node_id
            ));
        }
        let node = nodes
            .get(&key)
            .copied()
            .ok_or_else(|| format!("YSCD SoundGraph node '{}' does not resolve", node_id))?;
        if node.output_kind() != newengine_asset_format_nef8::YscdSoundGraphValueKind::Voices {
            return Err(format!(
                "YSCD SoundGraph node '{}' was evaluated as voices but produces scalar",
                node_id
            ));
        }
        stack.push(key.clone());
        let result = match node {
            newengine_asset_format_nef8::YscdSoundGraphNode::Clip {
                id,
                clip,
                gain,
                pitch,
            } => Ok(vec![SoundGraphVoicePlan {
                clip_name: clip.trim().to_ascii_lowercase(),
                gain: *gain,
                pitch: *pitch,
                label: id.clone(),
            }]),
            newengine_asset_format_nef8::YscdSoundGraphNode::Random { id, children } => {
                let total = children.iter().map(|child| child.weight).sum::<f32>();
                if !total.is_finite() || total <= 0.0 {
                    return Err(format!(
                        "YSCD SoundGraph Random node '{}' has invalid weight total",
                        id
                    ));
                }
                let random = unit_f32(splitmix64(ctx.seed ^ stable_text_hash(id)));
                let mut cursor = random * total;
                let mut selected = children.last().expect("validated Random children");
                for child in children {
                    if cursor < child.weight {
                        selected = child;
                        break;
                    }
                    cursor -= child.weight;
                }
                self.eval_sound_graph_voice_node(&selected.node, nodes, ctx, stack, staged_sequences)
            }
            newengine_asset_format_nef8::YscdSoundGraphNode::Sequence { id, children } => {
                let state_key = match ctx.scope_id {
                    Some(scope_id) => format!(
                        "{}#{}#object:{}",
                        ctx.canonical,
                        id.trim().to_ascii_lowercase(),
                        scope_id
                    ),
                    None => format!(
                        "{}#{}#global",
                        ctx.canonical,
                        id.trim().to_ascii_lowercase()
                    ),
                };
                let cursor = staged_sequences
                    .get(&state_key)
                    .copied()
                    .or_else(|| self.sound_graph_sequences.get(&state_key).copied())
                    .unwrap_or(0);
                let selected = children[(cursor as usize) % children.len()].clone();
                staged_sequences.insert(state_key, cursor.wrapping_add(1));
                self.eval_sound_graph_voice_node(&selected, nodes, ctx, stack, staged_sequences)
            }
            newengine_asset_format_nef8::YscdSoundGraphNode::Switch {
                id,
                switch,
                cases,
                default,
            } => {
                let value = ctx
                    .parameters
                    .switches
                    .get(switch)
                    .map(String::as_str)
                    .unwrap_or("");
                let selected = cases
                    .iter()
                    .find(|(candidate, _)| candidate.eq_ignore_ascii_case(value))
                    .map(|(_, target)| target)
                    .or(default.as_ref())
                    .ok_or_else(|| {
                        format!(
                            "YSCD SoundGraph Switch node '{}' has no case for switch '{}' value '{}' and no default",
                            id, switch, value
                        )
                    })?;
                self.eval_sound_graph_voice_node(selected, nodes, ctx, stack, staged_sequences)
            }
            newengine_asset_format_nef8::YscdSoundGraphNode::Blend1d { id, input, points } => {
                let value = self.eval_sound_graph_scalar_node(input, nodes, ctx, stack)?;
                if points.len() == 1 || value <= points[0].value {
                    self.eval_sound_graph_voice_node(&points[0].node, nodes, ctx, stack, staged_sequences)
                } else if value >= points[points.len() - 1].value {
                    self.eval_sound_graph_voice_node(&points[points.len() - 1].node, nodes, ctx, stack, staged_sequences)
                } else {
                    let upper = points
                        .iter()
                        .position(|point| point.value >= value)
                        .ok_or_else(|| format!("YSCD SoundGraph Blend1D node '{}' bracket failed", id))?;
                    let lower = upper.saturating_sub(1);
                    let a = &points[lower];
                    let b = &points[upper];
                    let span = b.value - a.value;
                    if !span.is_finite() || span <= 0.0 {
                        return Err(format!(
                            "YSCD SoundGraph Blend1D node '{}' has invalid point span",
                            id
                        ));
                    }
                    let t = ((value - a.value) / span).clamp(0.0, 1.0);
                    if t <= 1.0e-6 {
                        self.eval_sound_graph_voice_node(&a.node, nodes, ctx, stack, staged_sequences)
                    } else if t >= 1.0 - 1.0e-6 {
                        self.eval_sound_graph_voice_node(&b.node, nodes, ctx, stack, staged_sequences)
                    } else {
                        let mut left = self.eval_sound_graph_voice_node(&a.node, nodes, ctx, stack, staged_sequences)?;
                        let mut right = self.eval_sound_graph_voice_node(&b.node, nodes, ctx, stack, staged_sequences)?;
                        for plan in &mut left {
                            plan.gain *= 1.0 - t;
                            plan.label = format!("{}:a:{}", id, plan.label);
                        }
                        for plan in &mut right {
                            plan.gain *= t;
                            plan.label = format!("{}:b:{}", id, plan.label);
                        }
                        left.extend(right);
                        Ok(left)
                    }
                }
            }
            newengine_asset_format_nef8::YscdSoundGraphNode::Layer { id, children } => {
                let mut output = Vec::new();
                for child in children {
                    let mut child_output =
                        self.eval_sound_graph_voice_node(&child.node, nodes, ctx, stack, staged_sequences)?;
                    for plan in &mut child_output {
                        plan.gain *= child.gain;
                        plan.pitch *= child.pitch;
                        plan.label = format!("{}:{}", id, plan.label);
                    }
                    output.extend(child_output);
                    if output.len() > 64 {
                        return Err(format!(
                            "YSCD SoundGraph Layer node '{}' exceeds 64 emitted voices",
                            id
                        ));
                    }
                }
                Ok(output)
            }
            newengine_asset_format_nef8::YscdSoundGraphNode::Parameter { .. }
            | newengine_asset_format_nef8::YscdSoundGraphNode::Envelope { .. } => unreachable!(
                "typed graph validation prevents scalar node on voice path"
            ),
        };
        stack.pop();
        result
    }

    fn eval_sound_graph_scalar_node<'a>(
        &mut self,
        node_id: &str,
        nodes: &HashMap<String, &'a newengine_asset_format_nef8::YscdSoundGraphNode>,
        ctx: &SoundGraphEvalContext<'_>,
        stack: &mut Vec<String>,
    ) -> Result<f32, String> {
        let key = node_id.trim().to_ascii_lowercase();
        if stack.len() >= 64 {
            return Err("YSCD SoundGraph scalar traversal exceeded depth 64".to_owned());
        }
        if stack.iter().any(|entry| entry == &key) {
            return Err(format!(
                "YSCD SoundGraph runtime cycle detected at scalar node '{}'",
                node_id
            ));
        }
        let node = nodes
            .get(&key)
            .copied()
            .ok_or_else(|| format!("YSCD SoundGraph scalar node '{}' does not resolve", node_id))?;
        if node.output_kind() != newengine_asset_format_nef8::YscdSoundGraphValueKind::Scalar {
            return Err(format!(
                "YSCD SoundGraph node '{}' was evaluated as scalar but produces voices",
                node_id
            ));
        }
        stack.push(key);
        let result = match node {
            newengine_asset_format_nef8::YscdSoundGraphNode::Parameter {
                parameter,
                default,
                min,
                max,
                ..
            } => Ok(ctx
                .parameters
                .scalars
                .get(parameter)
                .copied()
                .unwrap_or(*default)
                .clamp(*min, *max)),
            newengine_asset_format_nef8::YscdSoundGraphNode::Envelope { id, input, points } => {
                let value = self.eval_sound_graph_scalar_node(input, nodes, ctx, stack)?;
                if points.len() == 1 || value <= points[0][0] {
                    Ok(points[0][1])
                } else if value >= points[points.len() - 1][0] {
                    Ok(points[points.len() - 1][1])
                } else {
                    let upper = points
                        .iter()
                        .position(|point| point[0] >= value)
                        .ok_or_else(|| format!("YSCD SoundGraph Envelope node '{}' bracket failed", id))?;
                    let lower = upper.saturating_sub(1);
                    let [x0, y0] = points[lower];
                    let [x1, y1] = points[upper];
                    let span = x1 - x0;
                    if !span.is_finite() || span <= 0.0 {
                        return Err(format!(
                            "YSCD SoundGraph Envelope node '{}' has invalid point span",
                            id
                        ));
                    }
                    let t = ((value - x0) / span).clamp(0.0, 1.0);
                    Ok(y0 + (y1 - y0) * t)
                }
            }
            _ => unreachable!("typed graph validation prevents voice node on scalar path"),
        };
        stack.pop();
        result
    }
}

#[cfg(test)]
mod sound_graph_runtime_tests {
    use super::*;
    use newengine_asset_format_nef8::{
        YscdBlendPoint, YscdLayerNodeRef, YscdSoundGraph, YscdSoundGraphNode,
        YscdWeightedNodeRef,
    };

    fn runtime_without_device() -> AudioRuntimeState {
        let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        AudioRuntimeState::open_default(assets).expect("runtime")
    }

    fn clip(id: &str, clip: &str) -> YscdSoundGraphNode {
        YscdSoundGraphNode::Clip {
            id: id.to_owned(),
            clip: clip.to_owned(),
            gain: 1.0,
            pitch: 1.0,
        }
    }

    #[test]
    fn random_is_deterministic_for_seed_and_sequence_advances_per_trigger() {
        let graph = YscdSoundGraph {
            root: "layer".to_owned(),
            nodes: vec![
                clip("a", "a"),
                clip("b", "b"),
                YscdSoundGraphNode::Random {
                    id: "random".to_owned(),
                    children: vec![
                        YscdWeightedNodeRef {
                            node: "a".to_owned(),
                            weight: 1.0,
                        },
                        YscdWeightedNodeRef {
                            node: "b".to_owned(),
                            weight: 1.0,
                        },
                    ],
                },
                YscdSoundGraphNode::Sequence {
                    id: "sequence".to_owned(),
                    children: vec!["a".to_owned(), "b".to_owned()],
                },
                YscdSoundGraphNode::Layer {
                    id: "layer".to_owned(),
                    children: vec![
                        YscdLayerNodeRef {
                            node: "random".to_owned(),
                            ..Default::default()
                        },
                        YscdLayerNodeRef {
                            node: "sequence".to_owned(),
                            ..Default::default()
                        },
                    ],
                },
            ],
        };
        graph.validate(["a", "b"]).expect("graph");
        let mut runtime = runtime_without_device();
        let first = runtime
            .evaluate_sound_graph("test@cue", &graph, &AudioParameterSet::default(), 7, None)
            .expect("first");
        let second = runtime
            .evaluate_sound_graph("test@cue", &graph, &AudioParameterSet::default(), 7, None)
            .expect("second");
        assert_eq!(first[0].clip_name, second[0].clip_name, "random must be seed-stable");
        assert_eq!(first[1].clip_name, "a");
        assert_eq!(second[1].clip_name, "b");
    }

    #[test]
    fn switch_blend_parameter_envelope_and_layer_compose_without_engine_known_names() {
        let graph = YscdSoundGraph {
            root: "switch".to_owned(),
            nodes: vec![
                clip("slow", "slow"),
                clip("fast", "fast"),
                YscdSoundGraphNode::Parameter {
                    id: "p".to_owned(),
                    parameter: "project.machine.rpm".to_owned(),
                    default: 0.0,
                    min: 0.0,
                    max: 10_000.0,
                },
                YscdSoundGraphNode::Envelope {
                    id: "curve".to_owned(),
                    input: "p".to_owned(),
                    points: vec![[0.0, 0.0], [10_000.0, 1.0]],
                },
                YscdSoundGraphNode::Blend1d {
                    id: "blend".to_owned(),
                    input: "curve".to_owned(),
                    points: vec![
                        YscdBlendPoint {
                            value: 0.0,
                            node: "slow".to_owned(),
                        },
                        YscdBlendPoint {
                            value: 1.0,
                            node: "fast".to_owned(),
                        },
                    ],
                },
                YscdSoundGraphNode::Layer {
                    id: "fallback".to_owned(),
                    children: vec![YscdLayerNodeRef {
                        node: "slow".to_owned(),
                        gain: 0.25,
                        pitch: 0.8,
                    }],
                },
                YscdSoundGraphNode::Switch {
                    id: "switch".to_owned(),
                    switch: "project.machine.mode".to_owned(),
                    cases: [("active".to_owned(), "blend".to_owned())]
                        .into_iter()
                        .collect(),
                    default: Some("fallback".to_owned()),
                },
            ],
        };
        graph.validate(["slow", "fast"]).expect("graph");
        let mut parameters = AudioParameterSet::default();
        parameters
            .set_scalar("project.machine.rpm", 5_000.0)
            .expect("scalar");
        parameters
            .set_switch("project.machine.mode", "active")
            .expect("switch");
        let mut runtime = runtime_without_device();
        let plans = runtime
            .evaluate_sound_graph("test@machine", &graph, &parameters, 11, None)
            .expect("plans");
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].clip_name, "slow");
        assert_eq!(plans[1].clip_name, "fast");
        assert!((plans[0].gain - 0.5).abs() < 1.0e-6);
        assert!((plans[1].gain - 0.5).abs() < 1.0e-6);
    }
    #[test]
    fn failed_graph_evaluation_does_not_advance_sequence_state() {
        let graph = YscdSoundGraph {
            root: "root".to_owned(),
            nodes: vec![
                clip("a", "a"),
                clip("b", "b"),
                YscdSoundGraphNode::Sequence {
                    id: "seq".to_owned(),
                    children: vec!["a".to_owned(), "b".to_owned()],
                },
                YscdSoundGraphNode::Switch {
                    id: "required_switch".to_owned(),
                    switch: "project.required".to_owned(),
                    cases: [("go".to_owned(), "a".to_owned())].into_iter().collect(),
                    default: None,
                },
                YscdSoundGraphNode::Layer {
                    id: "root".to_owned(),
                    children: vec![
                        YscdLayerNodeRef {
                            node: "seq".to_owned(),
                            ..Default::default()
                        },
                        YscdLayerNodeRef {
                            node: "required_switch".to_owned(),
                            ..Default::default()
                        },
                    ],
                },
            ],
        };
        graph.validate(["a", "b"]).expect("graph");
        let mut runtime = runtime_without_device();
        assert!(runtime
            .evaluate_sound_graph("test@transactional", &graph, &AudioParameterSet::default(), 1, None)
            .is_err());
        assert!(runtime.sound_graph_sequences.is_empty());

        let mut parameters = AudioParameterSet::default();
        parameters
            .set_switch("project.required", "go")
            .expect("switch");
        let plans = runtime
            .evaluate_sound_graph("test@transactional", &graph, &parameters, 1, None)
            .expect("successful retry");
        assert_eq!(plans[0].clip_name, "a");
    }

    #[test]
    fn blend_exact_point_does_not_emit_zero_gain_ghost_voice() {
        let graph = YscdSoundGraph {
            root: "blend".to_owned(),
            nodes: vec![
                clip("a", "a"),
                clip("b", "b"),
                clip("c", "c"),
                YscdSoundGraphNode::Parameter {
                    id: "p".to_owned(),
                    parameter: "project.value".to_owned(),
                    default: 1.0,
                    min: 0.0,
                    max: 2.0,
                },
                YscdSoundGraphNode::Blend1d {
                    id: "blend".to_owned(),
                    input: "p".to_owned(),
                    points: vec![
                        YscdBlendPoint { value: 0.0, node: "a".to_owned() },
                        YscdBlendPoint { value: 1.0, node: "b".to_owned() },
                        YscdBlendPoint { value: 2.0, node: "c".to_owned() },
                    ],
                },
            ],
        };
        graph.validate(["a", "b", "c"]).expect("graph");
        let mut runtime = runtime_without_device();
        let plans = runtime
            .evaluate_sound_graph("test@blend-exact", &graph, &AudioParameterSet::default(), 1, None)
            .expect("plans");
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].clip_name, "b");
        assert!((plans[0].gain - 1.0).abs() < 1.0e-6);
    }

    fn mono_pcm16_wav(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16_u32.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        out.extend_from_slice(&2_u16.to_le_bytes());
        out.extend_from_slice(&16_u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out
    }

    #[test]
    fn sound_graph_playback_emits_logical_voices_under_one_policy_instance() {
        let graph = YscdSoundGraph {
            root: "blend".to_owned(),
            nodes: vec![
                clip("a", "a"),
                clip("b", "b"),
                YscdSoundGraphNode::Parameter {
                    id: "p".to_owned(),
                    parameter: "project.test.mix".to_owned(),
                    default: 0.5,
                    min: 0.0,
                    max: 1.0,
                },
                YscdSoundGraphNode::Blend1d {
                    id: "blend".to_owned(),
                    input: "p".to_owned(),
                    points: vec![
                        YscdBlendPoint { value: 0.0, node: "a".to_owned() },
                        YscdBlendPoint { value: 1.0, node: "b".to_owned() },
                    ],
                },
            ],
        };
        graph.validate(["a", "b"]).expect("graph");
        let mut runtime = runtime_without_device();
        runtime.max_physical_voices = 0;
        let samples = vec![0_i16; 4_800];
        let wav = mono_pcm16_wav(48_000, &samples);
        let uri_a = "shared/audio/graph_a.wav".to_owned();
        let uri_b = "shared/audio/graph_b.wav".to_owned();
        runtime
            .cache_clip_bytes(uri_a.clone(), wav.clone())
            .expect("cache a");
        runtime
            .cache_clip_bytes(uri_b.clone(), wav)
            .expect("cache b");
        let clip_a = SoundCueClip {
            clip: newengine_audio_api::AudioClipRef::new(uri_a),
            weight: 1.0,
            gain: 1.0,
            pitch: 1.0,
        };
        let clip_b = SoundCueClip {
            clip: newengine_audio_api::AudioClipRef::new(uri_b),
            weight: 1.0,
            gain: 1.0,
            pitch: 1.0,
        };
        let canonical = "shared/audio/test_graph.yscd@test".to_owned();
        runtime.cues.insert(
            canonical.clone(),
            SoundCue {
                clips: vec![clip_a.clone(), clip_b.clone()],
                concurrency_group: "project.graph.test".to_owned(),
                concurrency_limit: 4,
                ..SoundCue::default()
            }
            .sanitized()
            .expect("cue"),
        );
        runtime.cue_clips_by_name.insert(
            canonical.clone(),
            [("a".to_owned(), clip_a), ("b".to_owned(), clip_b)]
                .into_iter()
                .collect(),
        );
        runtime
            .cue_sound_graphs
            .insert(canonical.clone(), Arc::new(graph));

        let mut request = AudioCuePlayRequest::new(canonical);
        request
            .parameters
            .set_scalar("project.test.mix", 0.5)
            .expect("parameter");
        let ack = runtime.play_cue(request).expect("play graph");
        assert!(ack.accepted);
        assert_eq!(ack.voice_ids.len(), 2);
        assert!(ack.virtualized);
        let policy_instances = ack
            .voice_ids
            .iter()
            .map(|voice_id| runtime.voices.get(voice_id).expect("voice").policy_instance_id)
            .collect::<HashSet<_>>();
        assert_eq!(policy_instances.len(), 1);
        assert!(ack
            .voice_ids
            .iter()
            .all(|voice_id| runtime.voices.get(voice_id).is_some_and(VoiceEntry::is_virtual)));
    }

    #[test]
    fn sequence_cursor_is_isolated_by_audio_object_scope() {
        let graph = YscdSoundGraph {
            root: "seq".to_owned(),
            nodes: vec![
                clip("a", "a"),
                clip("b", "b"),
                YscdSoundGraphNode::Sequence {
                    id: "seq".to_owned(),
                    children: vec!["a".to_owned(), "b".to_owned()],
                },
            ],
        };
        graph.validate(["a", "b"]).expect("graph");
        let mut runtime = runtime_without_device();
        let parameters = AudioParameterSet::default();
        let object1_first = runtime
            .evaluate_sound_graph("test@scoped-seq", &graph, &parameters, 1, Some(11))
            .expect("object1 first");
        let object2_first = runtime
            .evaluate_sound_graph("test@scoped-seq", &graph, &parameters, 1, Some(22))
            .expect("object2 first");
        let object1_second = runtime
            .evaluate_sound_graph("test@scoped-seq", &graph, &parameters, 1, Some(11))
            .expect("object1 second");
        assert_eq!(object1_first[0].clip_name, "a");
        assert_eq!(object2_first[0].clip_name, "a");
        assert_eq!(object1_second[0].clip_name, "b");
    }

}
