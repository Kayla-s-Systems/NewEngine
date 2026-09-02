use super::*;

#[test]
fn playback_provider_exposes_voice_and_spatial_methods() {
    let info = AudioServiceInfo::playback_provider("test.audio");
    assert!(info
        .methods
        .iter()
        .any(|method| method == AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1));
    assert!(info
        .methods
        .iter()
        .any(|method| method == AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1));
    assert!(info
        .features
        .iter()
        .any(|feature| feature == "spatial-audio"));
}

#[test]
fn listener_ear_positions_are_centered_and_finite() {
    let (left, right) = AudioListenerState::default().ear_positions();
    assert!(left.iter().copied().all(f32::is_finite));
    assert!(right.iter().copied().all(f32::is_finite));
    assert!((left[0] + right[0]).abs() < 1.0e-6);
    assert!(left[0] < right[0]);
}

#[test]
fn play_request_sanitizes_runtime_controls() {
    let mut request = AudioPlayRequest::new("test.wav");
    request.gain = f32::INFINITY;
    request.speed = -10.0;
    let request = request.sanitized();
    assert_eq!(request.gain, 1.0);
    assert_eq!(request.speed, 0.05);

    let mut cue = AudioCuePlayRequest::new("shared/audio/test.ysncd@test");
    cue.pitch = -10.0;
    cue.gain = f32::NAN;
    let cue = cue.sanitized();
    assert_eq!(cue.pitch, 0.05);
    assert_eq!(cue.gain, 1.0);
}
#[test]
fn sound_cue_sanitizes_ranges_and_rejects_empty_clip_sets() {
    assert!(SoundCue::default().sanitized().is_err());
    let cue = SoundCue {
        clips: vec![SoundCueClip {
            clip: AudioClipRef::new("shared/audio/test.ogg"),
            weight: 1.0,
            gain: 1.0,
            pitch: 1.0,
        }],
        gain_range: [1.25, 0.75],
        pitch_range: [1.1, 0.9],
        ..SoundCue::default()
    }
    .sanitized()
    .expect("valid cue");
    assert_eq!(cue.gain_range, [0.75, 1.25]);
    assert_eq!(cue.pitch_range, [0.9, 1.1]);
}

#[test]
fn attenuation_curves_are_bounded_and_custom_points_interpolate() {
    let linear = AudioAttenuationSettings {
        min_distance: 2.0,
        max_distance: 12.0,
        curve: AudioAttenuationCurve::Linear,
        ..Default::default()
    };
    assert_eq!(linear.gain_at_distance(2.0), 1.0);
    assert_eq!(linear.gain_at_distance(12.0), 0.0);
    assert!((linear.gain_at_distance(7.0) - 0.5).abs() < 1.0e-5);

    let custom = AudioAttenuationSettings {
        min_distance: 0.0,
        max_distance: 100.0,
        curve: AudioAttenuationCurve::Custom,
        curve_points: vec![[0.75, 0.1], [0.25, 0.8]],
        ..Default::default()
    }
    .sanitized();
    assert_eq!(custom.curve_points.first().copied(), Some([0.0, 1.0]));
    assert_eq!(custom.curve_points.last().copied(), Some([1.0, 0.0]));
    let gain = custom.gain_at_distance(50.0);
    assert!((0.1..0.8).contains(&gain));
}

#[test]
fn sound_cue_carries_authored_attenuation() {
    let cue = SoundCue {
        clips: vec![SoundCueClip {
            clip: AudioClipRef::new("shared/audio/test.wav"),
            ..SoundCueClip::default()
        }],
        spatial_policy: SoundCueSpatialPolicy::Spatial,
        attenuation: Some(AudioAttenuationSettings {
            min_distance: 3.0,
            max_distance: 60.0,
            curve: AudioAttenuationCurve::Smoothstep,
            ..Default::default()
        }),
        ..SoundCue::default()
    }
    .sanitized()
    .expect("valid attenuated cue");
    let attenuation = cue.attenuation.expect("attenuation retained");
    assert_eq!(attenuation.min_distance, 3.0);
    assert_eq!(attenuation.max_distance, 60.0);
}

#[test]
fn audio_emitter_is_a_stable_semantic_component_payload() {
    let emitter = AudioEmitter::new("shared/audio/test.ysncd@test");
    let json = serde_json::to_value(&emitter).expect("serialize emitter");
    let decoded: AudioEmitter = serde_json::from_value(json).expect("decode emitter");
    assert_eq!(decoded, emitter);
    assert_eq!(AUDIO_EMITTER_COMPONENT_TYPE, "audio.emitter");
}

#[test]
fn occlusion_policy_produces_distinct_obstruction_and_occlusion_gain() {
    let settings = AudioOcclusionSettings::default().sanitized();
    assert_eq!(settings.ray_count, 3);
    let clear = settings.acoustic_state(0.0, 0.0);
    let obstructed = settings.acoustic_state(0.5, 0.0);
    let occluded = settings.acoustic_state(1.0, 1.0);
    assert_eq!(clear.transmission_gain, 1.0);
    assert!(obstructed.transmission_gain < clear.transmission_gain);
    assert!(occluded.transmission_gain < obstructed.transmission_gain);
    assert!((occluded.transmission_gain - settings.occlusion_gain).abs() < 1.0e-6);
}

#[test]
fn acoustic_state_smoothing_uses_attack_and_release_time_constants() {
    let blocked = AudioAcousticState {
        obstruction: 1.0,
        occlusion: 1.0,
        transmission_gain: 0.2,
        high_frequency_gain: 0.15,
        low_pass_hz: 1_200.0,
    };
    let attacked = AudioAcousticState::clear().smoothed_towards(blocked, 0.016, 0.05, 0.4);
    assert!(attacked.transmission_gain < 1.0);
    let released = attacked.smoothed_towards(AudioAcousticState::clear(), 0.016, 0.05, 0.4);
    assert!(released.transmission_gain > attacked.transmission_gain);
    assert!(released.transmission_gain < 1.0);
}

#[test]
fn acoustic_surface_is_a_stable_semantic_component_payload() {
    let surface = AcousticSurface::new(
        "material.concrete.wall",
        AcousticMaterialProfile {
            transmission_gain: 0.2,
            reflection_gain: 0.78,
            high_frequency_absorption: 0.9,
            low_pass_hz: 1_200.0,
        },
    );
    let json = serde_json::to_value(&surface).expect("serialize acoustic surface");
    let decoded: AcousticSurface = serde_json::from_value(json).expect("decode acoustic surface");
    assert_eq!(decoded, surface);
    assert_eq!(
        AUDIO_ACOUSTIC_SURFACE_COMPONENT_TYPE,
        "audio.acoustic_surface"
    );
}

#[test]
fn acoustic_material_profiles_change_energy_and_spectrum() {
    let settings = AudioOcclusionSettings::default();
    let concrete = AcousticMaterialProfile {
        transmission_gain: 0.16,
        reflection_gain: 0.78,
        high_frequency_absorption: 0.92,
        low_pass_hz: 1_100.0,
    };
    let glass = AcousticMaterialProfile {
        transmission_gain: 0.58,
        reflection_gain: 0.74,
        high_frequency_absorption: 0.42,
        low_pass_hz: 6_500.0,
    };
    let concrete_state = settings.acoustic_state_with_material(1.0, 1.0, concrete);
    let glass_state = settings.acoustic_state_with_material(1.0, 1.0, glass);
    assert!(concrete_state.transmission_gain < glass_state.transmission_gain);
    assert!(concrete_state.high_frequency_gain < glass_state.high_frequency_gain);
    assert!(concrete_state.low_pass_hz < glass_state.low_pass_hz);
    assert_ne!(concrete.reflection_gain, 1.0 - concrete.transmission_gain);
    assert_eq!(AcousticMaterialProfile::transparent().reflection_gain, 0.0);
}

#[test]
fn acoustic_material_library_prefers_longest_authored_surface_match() {
    let library = AcousticMaterialLibrary::new(vec![
        AcousticMaterialRule {
            material_id: "material.generic".to_owned(),
            surface_matches: vec!["wall".to_owned()],
            profile: AcousticMaterialProfile {
                transmission_gain: 0.7,
                reflection_gain: 0.25,
                high_frequency_absorption: 0.2,
                low_pass_hz: 9_000.0,
            },
        },
        AcousticMaterialRule {
            material_id: "material.specific".to_owned(),
            surface_matches: vec!["wall.solid".to_owned()],
            profile: AcousticMaterialProfile {
                transmission_gain: 0.2,
                reflection_gain: 0.85,
                high_frequency_absorption: 0.8,
                low_pass_hz: 2_000.0,
            },
        },
    ]);
    let resolved = library
        .resolve("surface.wall.solid")
        .expect("authored acoustic material");
    assert_eq!(resolved.material_id, "material.specific");
    assert_eq!(resolved.profile.transmission_gain, 0.2);
    assert_eq!(resolved.profile.reflection_gain, 0.85);
    assert!(library.resolve("surface.unmapped").is_none());
}

#[test]
fn transparent_acoustic_material_does_not_invent_spectral_loss() {
    let profile = AcousticMaterialProfile::transparent();
    assert_eq!(profile.transmission_gain, 1.0);
    assert_eq!(profile.high_frequency_absorption, 0.0);
    assert_eq!(profile.low_pass_hz, 20_000.0);
}

#[test]
fn invalid_acoustic_state_fails_open_instead_of_muting_audio() {
    let state = AudioAcousticState {
        obstruction: f32::NAN,
        occlusion: f32::INFINITY,
        transmission_gain: f32::NAN,
        high_frequency_gain: f32::NAN,
        low_pass_hz: f32::NAN,
    }
    .sanitized();
    assert_eq!(state, AudioAcousticState::clear());
}
