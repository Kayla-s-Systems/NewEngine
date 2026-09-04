use super::*;
use newengine_asset_format_nef8::{
    YsncdBlendPoint, YsncdLayerNodeRef, YsncdSoundGraph, YsncdSoundGraphNode, YsncdWeightedNodeRef,
};

fn runtime_without_device() -> AudioRuntimeState {
    let _context = newengine_plugin_host::create_host_context_with_environment_snapshot(
        std::iter::empty::<(std::ffi::OsString, std::ffi::OsString)>(),
    );
    let mut ysncd = newengine_assets_api::AssetFileTypeDescriptor {
        module_id: "test.audio.ysncd-format".to_owned(),
        family: "audio".to_owned(),
        extension: "ysncd".to_owned(),
        asset_kind: "sound_cue_dictionary".to_owned(),
        semantic_gateway: "engine.audio".to_owned(),
        gateway: "engine.audio".to_owned(),
        handler_service: "test.audio.ysncd-format".to_owned(),
        runtime_ready: true,
        requires_magic: false,
        ..newengine_assets_api::AssetFileTypeDescriptor::default()
    };
    ysncd.normalize_layer_contract();
    newengine_service_kit::register_engine_gateway_provider_service(
        newengine_service_kit::EngineGatewayProviderDecl {
            gateway: newengine_assets_api::ENGINE_ASSET_TYPES_SERVICE_ID,
            service_kind: newengine_service_api::EngineServiceKind::AssetTypes,
            provider_service: newengine_assets_api::ASSET_TYPES_SERVICE_ID,
            provider_route: "engine.assets.types.test",
            capability: newengine_assets_api::ASSET_TYPES_BACKEND_CAPABILITY_ID,
            priority: 0,
            owner: "newengine-audio-runtime.tests",
            service: newengine_assets::asset_types_gateway_service_seeded([ysncd]),
        },
    )
    .expect("isolated test asset-types gateway");
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    AudioRuntimeState::open_default(assets).expect("runtime")
}

fn clip(id: &str, clip: &str) -> YsncdSoundGraphNode {
    YsncdSoundGraphNode::Clip {
        id: id.to_owned(),
        clip: clip.to_owned(),
        gain: 1.0,
        pitch: 1.0,
    }
}

#[test]
fn random_is_deterministic_for_seed_and_sequence_advances_per_trigger() {
    let graph = YsncdSoundGraph {
        root: "layer".to_owned(),
        nodes: vec![
            clip("a", "a"),
            clip("b", "b"),
            YsncdSoundGraphNode::Random {
                id: "random".to_owned(),
                children: vec![
                    YsncdWeightedNodeRef {
                        node: "a".to_owned(),
                        weight: 1.0,
                    },
                    YsncdWeightedNodeRef {
                        node: "b".to_owned(),
                        weight: 1.0,
                    },
                ],
            },
            YsncdSoundGraphNode::Sequence {
                id: "sequence".to_owned(),
                children: vec!["a".to_owned(), "b".to_owned()],
            },
            YsncdSoundGraphNode::Layer {
                id: "layer".to_owned(),
                children: vec![
                    YsncdLayerNodeRef {
                        node: "random".to_owned(),
                        ..Default::default()
                    },
                    YsncdLayerNodeRef {
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
    assert_eq!(
        first[0].clip_name, second[0].clip_name,
        "random must be seed-stable"
    );
    assert_eq!(first[1].clip_name, "a");
    assert_eq!(second[1].clip_name, "b");
}

#[test]
fn switch_blend_parameter_envelope_and_layer_compose_without_engine_known_names() {
    let graph = YsncdSoundGraph {
        root: "switch".to_owned(),
        nodes: vec![
            clip("slow", "slow"),
            clip("fast", "fast"),
            YsncdSoundGraphNode::Parameter {
                id: "p".to_owned(),
                parameter: "project.machine.rpm".to_owned(),
                default: 0.0,
                min: 0.0,
                max: 10_000.0,
            },
            YsncdSoundGraphNode::Envelope {
                id: "curve".to_owned(),
                input: "p".to_owned(),
                points: vec![[0.0, 0.0], [10_000.0, 1.0]],
            },
            YsncdSoundGraphNode::Blend1d {
                id: "blend".to_owned(),
                input: "curve".to_owned(),
                points: vec![
                    YsncdBlendPoint {
                        value: 0.0,
                        node: "slow".to_owned(),
                    },
                    YsncdBlendPoint {
                        value: 1.0,
                        node: "fast".to_owned(),
                    },
                ],
            },
            YsncdSoundGraphNode::Layer {
                id: "fallback".to_owned(),
                children: vec![YsncdLayerNodeRef {
                    node: "slow".to_owned(),
                    gain: 0.25,
                    pitch: 0.8,
                }],
            },
            YsncdSoundGraphNode::Switch {
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
    let graph = YsncdSoundGraph {
        root: "root".to_owned(),
        nodes: vec![
            clip("a", "a"),
            clip("b", "b"),
            YsncdSoundGraphNode::Sequence {
                id: "seq".to_owned(),
                children: vec!["a".to_owned(), "b".to_owned()],
            },
            YsncdSoundGraphNode::Switch {
                id: "required_switch".to_owned(),
                switch: "project.required".to_owned(),
                cases: [("go".to_owned(), "a".to_owned())].into_iter().collect(),
                default: None,
            },
            YsncdSoundGraphNode::Layer {
                id: "root".to_owned(),
                children: vec![
                    YsncdLayerNodeRef {
                        node: "seq".to_owned(),
                        ..Default::default()
                    },
                    YsncdLayerNodeRef {
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
        .evaluate_sound_graph(
            "test@transactional",
            &graph,
            &AudioParameterSet::default(),
            1,
            None
        )
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
    let graph = YsncdSoundGraph {
        root: "blend".to_owned(),
        nodes: vec![
            clip("a", "a"),
            clip("b", "b"),
            clip("c", "c"),
            YsncdSoundGraphNode::Parameter {
                id: "p".to_owned(),
                parameter: "project.value".to_owned(),
                default: 1.0,
                min: 0.0,
                max: 2.0,
            },
            YsncdSoundGraphNode::Blend1d {
                id: "blend".to_owned(),
                input: "p".to_owned(),
                points: vec![
                    YsncdBlendPoint {
                        value: 0.0,
                        node: "a".to_owned(),
                    },
                    YsncdBlendPoint {
                        value: 1.0,
                        node: "b".to_owned(),
                    },
                    YsncdBlendPoint {
                        value: 2.0,
                        node: "c".to_owned(),
                    },
                ],
            },
        ],
    };
    graph.validate(["a", "b", "c"]).expect("graph");
    let mut runtime = runtime_without_device();
    let plans = runtime
        .evaluate_sound_graph(
            "test@blend-exact",
            &graph,
            &AudioParameterSet::default(),
            1,
            None,
        )
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
    let graph = YsncdSoundGraph {
        root: "blend".to_owned(),
        nodes: vec![
            clip("a", "a"),
            clip("b", "b"),
            YsncdSoundGraphNode::Parameter {
                id: "p".to_owned(),
                parameter: "project.test.mix".to_owned(),
                default: 0.5,
                min: 0.0,
                max: 1.0,
            },
            YsncdSoundGraphNode::Blend1d {
                id: "blend".to_owned(),
                input: "p".to_owned(),
                points: vec![
                    YsncdBlendPoint {
                        value: 0.0,
                        node: "a".to_owned(),
                    },
                    YsncdBlendPoint {
                        value: 1.0,
                        node: "b".to_owned(),
                    },
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
    let canonical = "shared/audio/test_graph.ysncd@test".to_owned();
    runtime.cues.insert(
        canonical.clone(),
        SoundCue {
            clips: vec![clip_a.clone(), clip_b.clone()],
            concurrency_group: "project.graph.test".to_owned(),
            concurrency_limit: 4,
            looping: true,
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
        .map(|voice_id| {
            runtime
                .voices
                .get(voice_id)
                .expect("voice")
                .policy_instance_id
        })
        .collect::<HashSet<_>>();
    assert_eq!(policy_instances.len(), 1);
    assert!(ack.voice_ids.iter().all(|voice_id| runtime
        .voices
        .get(voice_id)
        .is_some_and(VoiceEntry::is_virtual)));
}

#[test]
fn sequence_cursor_is_isolated_by_audio_object_scope() {
    let graph = YsncdSoundGraph {
        root: "seq".to_owned(),
        nodes: vec![
            clip("a", "a"),
            clip("b", "b"),
            YsncdSoundGraphNode::Sequence {
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
