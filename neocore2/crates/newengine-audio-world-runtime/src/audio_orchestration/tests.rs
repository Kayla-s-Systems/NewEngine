use super::*;
use newengine_audio_api::{
    AudioMixBusSpec, AudioMixPatch, AudioMixSnapshotSpec, AudioMusicLayerSpec,
    AudioMusicSelectorCondition, AudioMusicSelectorSpec, AudioMusicStateSpec, AudioMusicStemSpec,
    AudioMusicTransitionSpec, AudioVoiceStealRule,
};

#[test]
fn provider_clock_mapping_is_rational_and_rate_independent() {
    let handle = AudioOrchestrationHandle::default();
    let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
    runtime.provider_clock_anchor = Some(ProviderClockAnchor {
        transport_sample: 4_800,
        provider_sample: 9_600,
        provider_rate: 96_000,
    });
    assert_eq!(runtime.transport.sample_rate(), 48_000);
    assert_eq!(runtime.provider_sample_for_transport(7_200), Some(14_400));
    assert_eq!(runtime.provider_duration_for_transport(2_400), Some(4_800));
}

#[test]
fn provider_clock_drives_transport_by_rendered_frames() {
    let handle = AudioOrchestrationHandle::default();
    let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
    runtime.provider_clock_anchor = Some(ProviderClockAnchor {
        transport_sample: 0,
        provider_sample: 0,
        provider_rate: 96_000,
    });
    runtime.provider_clock = Some(AudioRenderClock {
        ready: true,
        sample_rate: 96_000,
        sample: 1_920,
        block_frames: 256,
    });
    let (_, due) = runtime.advance_transport_clock(10.0);
    assert!(due.is_empty());
    assert_eq!(runtime.transport.sample(), 960);
}

#[test]
fn prearmed_play_due_handoff_does_not_replay_provider_start() {
    let handle = AudioOrchestrationHandle::default();
    let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
    let instance_id = AudioInstanceId(41);
    let action_id = AudioTransportActionId(73);
    runtime.instances.insert(
        instance_id,
        RuntimeInstance {
            object_id: AudioObjectId(9),
            voice_ids: Vec::new(),
            route: AudioRouteId::default(),
            tags: Vec::new(),
            gain: 1.0,
            spatial: false,
            parameters: AudioParameterSet::default(),
            transport_start_sample: 24_000,
            transport_dispatch_sample: 0,
            render_armed: true,
        },
    );
    runtime
        .prearmed_transport_actions
        .insert(action_id, PrearmedTransportAction::Play { instance_id });
    runtime.apply_due_transport_action(DueTransportAction {
        id: action_id,
        intended_sample: 24_000,
        dispatch_sample: 24_000,
        lateness_samples: 0,
        action: AudioTransportAction::Play {
            instance_id,
            object_id: AudioObjectId(9),
            request: AudioPlayInstanceRequest::new("project/audio/exact.yscd@cue"),
        },
    });
    let instance = runtime
        .instances
        .get(&instance_id)
        .expect("instance retained");
    assert!(!instance.render_armed);
    assert_eq!(instance.transport_dispatch_sample, 24_000);
    assert!(!runtime.prearmed_transport_actions.contains_key(&action_id));
}

#[test]
fn prearm_lead_uses_configured_provider_block_count() {
    let config = AudioOrchestrationRuntimeConfig {
        provider_prearm_blocks: 3,
        ..AudioOrchestrationRuntimeConfig::default()
    };
    let handle = AudioOrchestrationHandle::with_config(config).expect("config");
    let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
    runtime.provider_clock = Some(AudioRenderClock {
        ready: true,
        sample_rate: 48_000,
        sample: 10_000,
        block_frames: 256,
    });
    assert!(!runtime.provider_target_has_prearm_lead(10_767));
    assert!(runtime.provider_target_has_prearm_lead(10_768));
}

#[test]
fn command_scratch_reuses_capacity_after_peak_batch() {
    let config = AudioOrchestrationRuntimeConfig {
        command_capacity: 4,
        command_initial_reserve: 1,
        ..AudioOrchestrationRuntimeConfig::default()
    };
    let handle = AudioOrchestrationHandle::with_config(config).expect("config");
    let mut runtime = AudioOrchestrationRuntimeModule::new(handle.clone());

    for _ in 0..4 {
        handle
            .create_object(AudioObjectState::default())
            .expect("first command batch");
    }
    runtime.process_commands();
    let peak_capacity = runtime.command_scratch.capacity();
    assert!(peak_capacity >= 4);

    for _ in 0..4 {
        handle
            .create_object(AudioObjectState::default())
            .expect("second command batch");
    }
    runtime.process_commands();
    assert_eq!(runtime.command_scratch.capacity(), peak_capacity);
}

#[test]
fn command_queue_is_bounded_and_reports_drops() {
    let handle = AudioOrchestrationHandle::with_capacity(1);
    let first = handle
        .create_object(AudioObjectState::default())
        .expect("first");
    assert_eq!(first.0, 1);
    assert!(handle.create_object(AudioObjectState::default()).is_err());
    assert_eq!(handle.dropped_commands(), 1);
}

#[test]
fn snapshot_transition_changes_project_route_gain_without_provider_bus_semantics() {
    let handle = AudioOrchestrationHandle::default();
    let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
    runtime.mix_graph = Some(AudioMixGraph {
        buses: vec![AudioMixBusSpec {
            id: AudioRouteId::new("my.project.any.route"),
            parent: None,
            gain_db: 0.0,
        }],
        snapshots: vec![AudioMixSnapshotSpec {
            id: "duck".to_owned(),
            transition_seconds: 1.0,
            patches: vec![AudioMixPatch {
                route: AudioRouteId::new("my.project.any.route"),
                gain_db: -12.0,
            }],
        }],
        ..Default::default()
    });
    runtime.activate_snapshot("duck", 1.0, Some(1.0));
    runtime.advance_snapshots(0.5);
    let gain = runtime.route_gain(&AudioRouteId::new("my.project.any.route"));
    assert!(gain < 1.0 && gain > 0.25, "half-transition gain={gain}");
    runtime.advance_snapshots(0.5);
    let gain = runtime.route_gain(&AudioRouteId::new("my.project.any.route"));
    assert!((gain - 10.0_f32.powf(-12.0 / 20.0)).abs() < 1.0e-4);
}

#[test]
fn sample_domain_snapshot_transition_is_exact_at_authored_samples() {
    let mut snapshot = ActiveSnapshot::new(0.0, 0.0);
    snapshot.retarget_samples(1.0, 100, 200);
    snapshot.advance_to_sample(100);
    assert!((snapshot.current - 0.0).abs() < 1.0e-6);
    snapshot.advance_to_sample(200);
    assert!((snapshot.current - 0.5).abs() < 1.0e-6);
    snapshot.advance_to_sample(300);
    assert!((snapshot.current - 1.0).abs() < 1.0e-6);
    assert!(snapshot.sample_transition.is_none());
}

#[test]
fn transport_instance_state_uses_intended_start_not_dispatch_frame() {
    let handle = AudioOrchestrationHandle::default();
    let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
    runtime.transport = AudioTransportRuntime::default();
    let _ = runtime.transport.advance_seconds(0.020);
    runtime.instances.insert(
        AudioInstanceId(9),
        RuntimeInstance {
            object_id: AudioObjectId(1),
            voice_ids: vec![1],
            route: AudioRouteId::default(),
            tags: Vec::new(),
            gain: 1.0,
            spatial: false,
            parameters: AudioParameterSet::default(),
            transport_start_sample: 480,
            transport_dispatch_sample: 720,
            render_armed: false,
        },
    );
    let state = runtime.snapshot_state();
    let timing = state
        .transport_instances
        .get(&AudioInstanceId(9))
        .expect("timing");
    assert_eq!(timing.start_sample, 480);
    assert_eq!(timing.dispatch_sample, 720);
    assert_eq!(timing.dispatch_lateness_samples, 240);
    assert_eq!(
        timing.logical_sample,
        runtime.transport.sample().saturating_sub(480)
    );
}

#[test]
fn sample_domain_scalar_transition_uses_existing_rtpc_and_exact_samples() {
    let handle = AudioOrchestrationHandle::default();
    let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
    runtime
        .global_parameters
        .set_scalar("project.transport.rtpc", 0.0)
        .unwrap();
    runtime.transition_scalar_samples(
        AudioParameterTarget::Global,
        "project.transport.rtpc".to_owned(),
        1.0,
        100,
        200,
    );
    runtime.advance_scalar_transitions_to_sample(100);
    assert_eq!(
        runtime.global_parameters.scalars["project.transport.rtpc"],
        0.0
    );
    runtime.advance_scalar_transitions_to_sample(200);
    assert!((runtime.global_parameters.scalars["project.transport.rtpc"] - 0.5).abs() < 1.0e-6);
    runtime.advance_scalar_transitions_to_sample(300);
    assert!((runtime.global_parameters.scalars["project.transport.rtpc"] - 1.0).abs() < 1.0e-6);
    assert!(runtime.scalar_transitions.is_empty());
}

#[test]
fn sample_scalar_transition_does_not_invent_missing_parameter_default() {
    let handle = AudioOrchestrationHandle::default();
    let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
    runtime.transition_scalar_samples(
        AudioParameterTarget::Global,
        "project.missing".to_owned(),
        1.0,
        0,
        100,
    );
    assert!(runtime.scalar_transitions.is_empty());
    assert!(!runtime
        .global_parameters
        .scalars
        .contains_key("project.missing"));
}

#[test]
fn schedule_stream_allocates_transport_and_instance_identity_without_stream_slots() {
    let handle = AudioOrchestrationHandle::default();
    let mut request = AudioPlayStreamInstanceRequest::new("shared/audio/music/stem.ogg");
    request.route = AudioRouteId::new("project.music.stems");
    request.tags = vec!["project.stem.rhythm".to_owned()];
    request.stream.voice_budget = "project.music".to_owned();
    let (instance_id, action_id) = handle
        .schedule_stream(
            AudioObjectId(7),
            request,
            AudioTransportSchedulePoint::NextBar,
        )
        .expect("schedule stream");
    assert_eq!(instance_id.0, 1);
    assert_eq!(action_id.0, 1);
    let commands = handle.queue.lock().drain();
    assert_eq!(commands.len(), 1);
    match &commands[0] {
        AudioOrchestrationCommand::ScheduleTransport {
            action_id: queued_id,
            when: AudioTransportSchedulePoint::NextBar,
            action:
                AudioTransportAction::PlayStream {
                    instance_id: queued_instance,
                    object_id,
                    request,
                },
        } => {
            assert_eq!(*queued_id, action_id);
            assert_eq!(*queued_instance, instance_id);
            assert_eq!(*object_id, AudioObjectId(7));
            assert_eq!(request.route.0, "project.music.stems");
            assert_eq!(request.stream.voice_budget, "project.music");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

fn test_music_graph() -> InteractiveMusicGraph {
    let mut base = AudioPlayStreamInstanceRequest::new("shared/audio/music/base.ogg");
    base.route = AudioRouteId::new("project.music");
    base.stream.voice_budget = "project.music".to_owned();
    base.stream.concurrency_group = "project.music.stems".to_owned();
    base.stream.concurrency_limit = 8;
    base.stream.steal_rule = AudioVoiceStealRule::RejectNew;
    let mut high = base.clone();
    high.stream.clip.uri = "shared/audio/music/high.ogg".to_owned();
    InteractiveMusicGraph {
        id: "project.score".to_owned(),
        initial_state: "calm".to_owned(),
        stems: vec![
            AudioMusicStemSpec {
                id: "base".to_owned(),
                request: base,
            },
            AudioMusicStemSpec {
                id: "high".to_owned(),
                request: high,
            },
        ],
        states: vec![
            AudioMusicStateSpec {
                id: "calm".to_owned(),
                layers: vec![AudioMusicLayerSpec {
                    stem: "base".to_owned(),
                    gain: 1.0,
                }],
            },
            AudioMusicStateSpec {
                id: "intense".to_owned(),
                layers: vec![
                    AudioMusicLayerSpec {
                        stem: "base".to_owned(),
                        gain: 0.6,
                    },
                    AudioMusicLayerSpec {
                        stem: "high".to_owned(),
                        gain: 1.0,
                    },
                ],
            },
        ],
        transitions: vec![AudioMusicTransitionSpec {
            from: "calm".to_owned(),
            to: "intense".to_owned(),
            quantization: AudioTransportSchedulePoint::NextBar,
            crossfade_samples: 100,
        }],
        selectors: vec![AudioMusicSelectorSpec {
            condition: AudioMusicSelectorCondition::ScalarRange {
                name: "project.score.intensity".to_owned(),
                min: 0.5,
                max: 1.0,
            },
            target_state: "intense".to_owned(),
        }],
        ..Default::default()
    }
}

fn music_runtime() -> AudioOrchestrationRuntimeModule {
    let handle = AudioOrchestrationHandle::default();
    let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
    runtime.objects.insert(
        AudioObjectId(7),
        RuntimeObject {
            state: AudioObjectState::default(),
        },
    );
    runtime.install_music_graph(test_music_graph());
    runtime
}

#[test]
fn interactive_music_initial_state_uses_transport_stream_and_preserves_voice_policy() {
    let mut runtime = music_runtime();
    runtime.create_music_session(
        AudioMusicSessionId(1),
        "project.score".to_owned(),
        AudioObjectId(7),
    );
    let (_, due) = runtime.transport.advance_samples(1);
    assert_eq!(due.len(), 1);
    match &due[0].action {
        AudioTransportAction::PlayStream {
            request, object_id, ..
        } => {
            assert_eq!(*object_id, AudioObjectId(7));
            assert_eq!(request.stream.clip.uri, "shared/audio/music/base.ogg");
            assert_eq!(request.stream.voice_budget, "project.music");
            assert_eq!(request.stream.concurrency_group, "project.music.stems");
            assert_eq!(request.stream.concurrency_limit, 8);
        }
        other => panic!("unexpected initial music action: {other:?}"),
    }
    runtime.finalize_music_transitions();
    let session = runtime.music_sessions.get(&AudioMusicSessionId(1)).unwrap();
    assert_eq!(session.active_state, "calm");
    assert!(session.pending.is_none());
}

#[test]
fn next_bar_music_transition_reuses_common_stem_and_crossfades_new_stem() {
    let mut runtime = music_runtime();
    runtime.create_music_session(
        AudioMusicSessionId(1),
        "project.score".to_owned(),
        AudioObjectId(7),
    );
    let (_, initial_due) = runtime.transport.advance_samples(1);
    let base_instance = match initial_due[0].action {
        AudioTransportAction::PlayStream { instance_id, .. } => instance_id,
        _ => panic!("initial stem must be stream"),
    };
    runtime.finalize_music_transitions();
    runtime.instances.insert(
        base_instance,
        RuntimeInstance {
            object_id: AudioObjectId(7),
            voice_ids: vec![101],
            route: AudioRouteId::new("project.music"),
            tags: Vec::new(),
            gain: 1.0,
            spatial: false,
            parameters: AudioParameterSet::default(),
            transport_start_sample: 0,
            transport_dispatch_sample: 0,
            render_armed: false,
        },
    );
    runtime.request_music_state(AudioMusicSessionId(1), "intense".to_owned());
    let pending = runtime
        .music_sessions
        .get(&AudioMusicSessionId(1))
        .and_then(|session| session.pending.as_ref())
        .expect("pending transition");
    assert_eq!(pending.start_sample, 96_000);
    assert_eq!(pending.complete_sample, 96_100);
    assert_eq!(
        pending.target_stems["base"], base_instance,
        "common stem identity must survive state change"
    );
    let high_instance = pending.target_stems["high"];
    assert_ne!(high_instance, base_instance);

    let (_, due) = runtime.transport.advance_samples(95_999);
    assert_eq!(runtime.transport.sample(), 96_000);
    assert_eq!(due.len(), 3);
    assert!(due.iter().any(|item| matches!(
            item.action,
            AudioTransportAction::TransitionInstanceGain { instance_id, target_gain, duration_samples: 100 }
                if instance_id == base_instance && (target_gain - 0.6).abs() < 1.0e-6
        )));
    assert!(due.iter().any(|item| matches!(
            &item.action,
            AudioTransportAction::PlayStream { instance_id, request, .. }
                if *instance_id == high_instance && request.gain == 0.0 && request.stream.voice_budget == "project.music"
        )));
    assert!(due.iter().any(|item| matches!(
            item.action,
            AudioTransportAction::TransitionInstanceGain { instance_id, target_gain, duration_samples: 100 }
                if instance_id == high_instance && (target_gain - 1.0).abs() < 1.0e-6
        )));
}

#[test]
fn project_scalar_selector_requests_authored_music_state_without_engine_known_semantics() {
    let mut runtime = music_runtime();
    runtime.create_music_session(
        AudioMusicSessionId(1),
        "project.score".to_owned(),
        AudioObjectId(7),
    );
    let _ = runtime.transport.advance_samples(1);
    runtime.finalize_music_transitions();
    runtime.set_music_scalar(
        AudioMusicSessionId(1),
        "project.score.intensity".to_owned(),
        0.75,
    );
    let session = runtime.music_sessions.get(&AudioMusicSessionId(1)).unwrap();
    assert_eq!(session.parameters.scalars["project.score.intensity"], 0.75);
    assert_eq!(session.pending.as_ref().unwrap().target_state, "intense");
    assert_eq!(
        runtime.objects[&AudioObjectId(7)].state.parameters.scalars["project.score.intensity"],
        0.75
    );
}

#[test]
fn instance_gain_transition_is_sample_domain_exact() {
    let handle = AudioOrchestrationHandle::default();
    let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
    runtime.instances.insert(
        AudioInstanceId(3),
        RuntimeInstance {
            object_id: AudioObjectId(1),
            voice_ids: vec![1],
            route: AudioRouteId::default(),
            tags: Vec::new(),
            gain: 0.0,
            spatial: false,
            parameters: AudioParameterSet::default(),
            transport_start_sample: 0,
            transport_dispatch_sample: 0,
            render_armed: false,
        },
    );
    runtime.transition_instance_gain_samples(AudioInstanceId(3), 1.0, 100, 200);
    runtime.advance_instance_gain_transitions_to_sample(100);
    assert!((runtime.instances[&AudioInstanceId(3)].gain - 0.0).abs() < 1.0e-6);
    runtime.advance_instance_gain_transitions_to_sample(200);
    assert!((runtime.instances[&AudioInstanceId(3)].gain - 0.5).abs() < 1.0e-6);
    runtime.advance_instance_gain_transitions_to_sample(300);
    assert!((runtime.instances[&AudioInstanceId(3)].gain - 1.0).abs() < 1.0e-6);
    assert!(runtime.instance_gain_transitions.is_empty());
}
