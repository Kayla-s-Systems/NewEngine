#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialization_suppresses_near_zero_decoder_seek() {
        assert!(!should_seek_materialized_voice(Duration::ZERO));
        assert!(!should_seek_materialized_voice(Duration::from_millis(1)));
        assert!(!should_seek_materialized_voice(Duration::from_millis(49)));
        assert!(should_seek_materialized_voice(Duration::from_millis(50)));
        assert!(should_seek_materialized_voice(Duration::from_secs(1)));
    }

    #[test]
    fn feedback_tones_are_bounded() {
        for event in [
            "ui.open",
            "ui.close",
            "ui.navigate",
            "ui.confirm",
            "ui.back",
            "ui.rebind",
            "ui.error",
        ] {
            let (hz, ms) = feedback_tone(event);
            assert!((80.0..=4_000.0).contains(&hz));
            assert!((10..=500).contains(&ms));
        }
    }

    #[test]
    fn audio_paths_are_vfs_only() {
        assert_eq!(
            normalize_vfs_path("shared/audio/test.wav").unwrap(),
            "shared/audio/test.wav"
        );
        assert!(normalize_vfs_path("C:/audio/test.wav").is_err());
        assert!(normalize_vfs_path("../audio/test.wav").is_err());
        assert!(normalize_vfs_path("shared/audio/clip.wav@entry").is_err());
        assert!(normalize_vfs_path("shared/audio/weapon/rifle/rifle.yscd@fire").is_err());
    }

    #[test]
    fn yscd_metadata_maps_to_audio_runtime_semantics() {
        assert_eq!(audio_bus_from_yscd("sfx").unwrap(), AudioBus::Sfx);
        assert_eq!(
            sound_cue_spatial_policy_from_yscd("spatial").unwrap(),
            SoundCueSpatialPolicy::Spatial
        );
        let attenuation =
            audio_attenuation_from_yscd(&newengine_asset_format_nef8::YscdAttenuation {
                min_distance: 2.0,
                max_distance: 140.0,
                curve: "inverse".to_owned(),
                rolloff: 0.75,
                curve_points: Vec::new(),
            })
            .unwrap();
        assert_eq!(attenuation.min_distance, 2.0);
        assert_eq!(attenuation.max_distance, 140.0);
        assert_eq!(
            attenuation.curve,
            newengine_audio_api::AudioAttenuationCurve::Inverse
        );
        assert_eq!(attenuation.rolloff, 0.75);
    }

    #[test]
    fn yscd_embedded_clip_keys_are_stable_and_codec_suffixed() {
        let a = embedded_yscd_clip_key("shared/audio/rifle.yscd@fire", 0, "wav");
        let b = embedded_yscd_clip_key("shared/audio/rifle.yscd@fire", 0, "wav");
        assert_eq!(a, b);
        assert!(a.starts_with("__yscd/"));
        assert!(a.ends_with(".wav"));
    }

    #[test]
    fn weighted_selection_is_deterministic() {
        let cue = SoundCue {
            clips: vec![
                newengine_audio_api::SoundCueClip {
                    clip: newengine_audio_api::AudioClipRef::new("a.wav"),
                    weight: 1.0,
                    gain: 1.0,
                    pitch: 1.0,
                },
                newengine_audio_api::SoundCueClip {
                    clip: newengine_audio_api::AudioClipRef::new("b.wav"),
                    weight: 3.0,
                    gain: 1.0,
                    pitch: 1.0,
                },
            ],
            ..SoundCue::default()
        }
        .sanitized()
        .unwrap();
        assert_eq!(select_weighted_clip(&cue, 0.0).unwrap().clip.uri, "a.wav");
        assert_eq!(select_weighted_clip(&cue, 0.9).unwrap().clip.uri, "b.wav");
    }

    #[test]
    fn repeat_avoidance_excludes_recent_clip_without_destroying_weighted_fallback() {
        let clips = vec![
            newengine_audio_api::SoundCueClip {
                clip: newengine_audio_api::AudioClipRef::new("a.wav"),
                weight: 1.0,
                ..Default::default()
            },
            newengine_audio_api::SoundCueClip {
                clip: newengine_audio_api::AudioClipRef::new("b.wav"),
                weight: 1.0,
                ..Default::default()
            },
            newengine_audio_api::SoundCueClip {
                clip: newengine_audio_api::AudioClipRef::new("c.wav"),
                weight: 1.0,
                ..Default::default()
            },
        ];
        let recent = VecDeque::from(["a.wav".to_owned()]);
        assert_ne!(
            select_weighted_clips_avoiding(&clips, 0.0, &recent)
                .expect("eligible clip")
                .clip
                .uri,
            "a.wav"
        );
        let all_recent =
            VecDeque::from(["a.wav".to_owned(), "b.wav".to_owned(), "c.wav".to_owned()]);
        assert!(select_weighted_clips_avoiding(&clips, 0.0, &all_recent).is_some());
    }

    #[test]
    fn voice_budget_rank_prefers_priority_then_audibility_then_distance() {
        let mut ranks = vec![
            VoiceRank {
                voice_id: 1,
                priority: 10,
                audibility: 0.9,
                distance: 1.0,
                already_physical: false,
            },
            VoiceRank {
                voice_id: 2,
                priority: 20,
                audibility: 0.1,
                distance: 100.0,
                already_physical: false,
            },
            VoiceRank {
                voice_id: 3,
                priority: 10,
                audibility: 0.95,
                distance: 50.0,
                already_physical: false,
            },
        ];
        sort_voice_ranks(&mut ranks);
        assert_eq!(
            ranks.iter().map(|rank| rank.voice_id).collect::<Vec<_>>(),
            vec![2, 3, 1]
        );
    }

    #[test]
    fn voice_budget_selection_never_exceeds_hard_cap() {
        let ranks = (0..100_u64)
            .map(|voice_id| VoiceRank {
                voice_id,
                priority: (voice_id % 5) as i32,
                audibility: 1.0,
                distance: voice_id as f32,
                already_physical: false,
            })
            .collect::<Vec<_>>();
        let selected = select_physical_voice_ids(ranks, 16);
        assert_eq!(selected.len(), 16);
    }

    #[test]
    fn dynamic_spectral_filter_updates_in_place_and_attenuates_high_frequency_energy() {
        let samples = (0..512)
            .map(|index| if index % 2 == 0 { 1.0 } else { -1.0 })
            .collect::<Vec<_>>();
        let buffer = rodio::buffer::SamplesBuffer::new(
            ChannelCount::new(1).expect("mono"),
            SampleRate::new(48_000).expect("sample rate"),
            samples,
        );
        let control = SpectralFilterControl::new(AudioAcousticState::clear());
        let mut source = DynamicSpectralSource::new(buffer, control.clone());
        let clear = source.by_ref().take(128).collect::<Vec<_>>();
        let clear_rms =
            (clear.iter().map(|sample| sample * sample).sum::<f32>() / clear.len() as f32).sqrt();

        let concrete = AudioAcousticState {
            obstruction: 1.0,
            occlusion: 1.0,
            transmission_gain: 0.1,
            high_frequency_gain: 0.08,
            low_pass_hz: 1_100.0,
        };
        control.set_acoustic(concrete);
        assert!((control.low_pass_hz() - 1_100.0).abs() < 1.0e-3);
        assert!((control.high_frequency_gain() - 0.08).abs() < 1.0e-6);

        source.by_ref().take(64).for_each(drop);
        let filtered = source.by_ref().take(128).collect::<Vec<_>>();
        let filtered_rms = (filtered.iter().map(|sample| sample * sample).sum::<f32>()
            / filtered.len() as f32)
            .sqrt();
        assert!(clear_rms > 0.99);
        assert!(filtered_rms < clear_rms * 0.35);
    }

    #[test]
    fn dynamic_environment_reverb_updates_in_place_and_produces_room_tail() {
        let mut samples = vec![0.0_f32; 16_000];
        samples[0] = 1.0;
        samples[7_000] = 1.0;
        let buffer = rodio::buffer::SamplesBuffer::new(
            ChannelCount::new(1).expect("mono"),
            SampleRate::new(48_000).expect("sample rate"),
            samples,
        );
        let control = EnvironmentFilterControl::new(AudioEnvironmentState::clear());
        let mut source = DynamicEnvironmentSource::new(buffer, control.clone());

        let clear = source.by_ref().take(6_000).collect::<Vec<_>>();
        let clear_tail_energy = clear.iter().skip(1).map(|sample| sample.abs()).sum::<f32>();
        assert!(clear_tail_energy < 1.0e-6);

        control.set_environment(AudioEnvironmentState {
            source_send: AudioReverbSend::default(),
            listener_send: AudioReverbSend {
                room_bus_id: 0,
                gain: 0.7,
                preset: newengine_audio_api::AudioReverbPreset::room(),
                early_reflections: AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            },
            direct_path: newengine_audio_api::AudioDirectPathResponse::clear(),
            portal_gain: 1.0,
        });
        let wet = source.by_ref().take(8_000).collect::<Vec<_>>();
        assert!(wet.iter().copied().all(f32::is_finite));
        let wet_tail_energy = wet
            .iter()
            .skip(1_550)
            .map(|sample| sample.abs())
            .sum::<f32>();
        assert!(wet_tail_energy > 0.01);
        assert!(
            wet.iter()
                .map(|sample| sample.abs())
                .fold(0.0_f32, f32::max)
                < 4.0
        );
    }

    fn fdn_test_params(decay_seconds: f32) -> ReverbSendSnapshot {
        ReverbSendSnapshot {
            gain: 1.0,
            early_reflections: AudioEarlyReflectionField::empty(),
            early_reflections_gain: 0.0,
            early_reflections_high_frequency_gain: 1.0,
            early_reflection_direction: [0.0; 3],
            pre_delay_ms: 0.0,
            early_reflections_spread_ms: 0.0,
            decay_seconds,
            damping: 0.35,
            diffusion: 0.85,
        }
    }

    #[test]
    fn fdn_reverb_decay_controls_late_tail_energy() {
        let sample_rate = SampleRate::new(48_000).expect("sample rate");
        let channels = ChannelCount::new(1).expect("mono");
        let mut short = ReverbTank::new(sample_rate, channels);
        let mut long = ReverbTank::new(sample_rate, channels);
        let short_params = fdn_test_params(0.25);
        let long_params = fdn_test_params(2.2);
        let mut short_late = 0.0_f32;
        let mut long_late = 0.0_f32;
        for frame in 0..48_000 {
            let input = if frame == 0 { 1.0 } else { 0.0 };
            let short_sample = short.process(input, short_params);
            let long_sample = long.process(input, long_params);
            if frame > 12_000 {
                short_late += short_sample.abs();
                long_late += long_sample.abs();
            }
        }
        assert!(long_late > short_late * 2.0);
        assert!(long_late.is_finite());
    }

    #[test]
    fn fdn_reverb_decorrelates_identical_stereo_tail() {
        let sample_rate = SampleRate::new(48_000).expect("sample rate");
        let channels = ChannelCount::new(2).expect("stereo");
        let mut tank = ReverbTank::new(sample_rate, channels);
        let params = fdn_test_params(1.5);
        let mut difference_energy = 0.0_f32;
        let mut peak = 0.0_f32;
        for frame in 0..24_000 {
            let input = if frame == 0 { 1.0 } else { 0.0 };
            let left = tank.process(input, params);
            let right = tank.process(input, params);
            if frame > 2_000 {
                difference_energy += (left - right).abs();
            }
            peak = peak.max(left.abs()).max(right.abs());
        }
        assert!(difference_energy > 0.05);
        assert!(peak < 4.0);
    }

    #[test]
    fn acoustic_transmission_participates_in_voice_budget_audibility() {
        let clear = 0.8_f32 * AudioAcousticState::clear().transmission_gain;
        let occluded = 0.8_f32
            * AudioAcousticState {
                obstruction: 1.0,
                occlusion: 1.0,
                transmission_gain: 0.2,
                high_frequency_gain: 0.2,
                low_pass_hz: 1_200.0,
            }
            .sanitized()
            .transmission_gain;
        assert!(occluded < clear);
        let mut ranks = vec![
            VoiceRank {
                voice_id: 1,
                priority: 10,
                audibility: occluded,
                distance: 2.0,
                already_physical: true,
            },
            VoiceRank {
                voice_id: 2,
                priority: 10,
                audibility: clear,
                distance: 8.0,
                already_physical: false,
            },
        ];
        sort_voice_ranks(&mut ranks);
        assert_eq!(ranks[0].voice_id, 2);
    }

    #[test]
    fn virtual_timeline_advances_in_source_time_and_wraps_loops() {
        let now = Instant::now();
        let voice = VoiceEntry {
            control: None,
            source: VoiceSource::Clip {
                uri: "shared/audio/test.wav".to_owned(),
                source_duration: Some(Duration::from_secs(2)),
            },
            bus: AudioBus::Sfx,
            gain: 1.0,
            speed: 2.0,
            looping: true,
            spatial: None,
            attenuation: None,
            acoustic: AudioAcousticState::clear(),
            propagation: AudioPropagationState::default(),
            emitter_velocity: [0.0; 3],
            last_spatial_update: None,
            environment: AudioEnvironmentState::clear(),
            stream_stats: None,
            concurrency_group: String::new(),
            priority: 0,
            paused: false,
            virtual_source_position: Duration::from_millis(250),
            virtual_since: Some(now - Duration::from_millis(500)),
        };
        // 250ms source base + 500ms wall time * 2x speed = 1250ms source time.
        let position = voice.current_source_position(now);
        assert!((position.as_secs_f32() - 1.25).abs() < 0.02);

        let wrapped = VoiceEntry {
            virtual_source_position: Duration::from_millis(1750),
            virtual_since: Some(now - Duration::from_millis(500)),
            ..voice
        };
        // 1750ms + 1000ms source advance wraps over a 2s loop to ~750ms.
        let position = wrapped.current_source_position(now);
        assert!((position.as_secs_f32() - 0.75).abs() < 0.02);
    }

    #[test]
    fn attenuation_distance_reduces_physical_audibility() {
        let attenuation = AudioAttenuationSettings {
            min_distance: 0.0,
            max_distance: 100.0,
            curve: newengine_audio_api::AudioAttenuationCurve::Linear,
            ..Default::default()
        };
        assert!(attenuation.gain_at_distance(10.0) > attenuation.gain_at_distance(90.0));
        assert_eq!(attenuation.gain_at_distance(100.0), 0.0);
    }

    #[test]
    fn doppler_ratio_tracks_approach_and_recession() {
        let listener = [0.0, 0.0, 0.0];
        let source = [0.0, 0.0, 20.0];
        let approaching = doppler_ratio(listener, [0.0; 3], source, [0.0, 0.0, -30.0]);
        let receding = doppler_ratio(listener, [0.0; 3], source, [0.0, 0.0, 30.0]);
        assert!(approaching > 1.0);
        assert!(receding < 1.0);
        assert!(approaching > receding);
    }

    #[test]
    fn listener_motion_contributes_to_doppler() {
        let source = [0.0, 0.0, 20.0];
        let toward = doppler_ratio([0.0; 3], [0.0, 0.0, 20.0], source, [0.0; 3]);
        let away = doppler_ratio([0.0; 3], [0.0, 0.0, -20.0], source, [0.0; 3]);
        assert!(toward > 1.0);
        assert!(away < 1.0);
    }

    #[test]
    fn air_absorption_reduces_high_frequency_energy_with_distance() {
        let listener = AudioListenerState::default();
        let near = propagation_state(
            listener,
            [0.0; 3],
            Some(AudioSpatialParams {
                position: [0.0, 0.0, 5.0],
            }),
            [0.0; 3],
        );
        let far = propagation_state(
            listener,
            [0.0; 3],
            Some(AudioSpatialParams {
                position: [0.0, 0.0, 120.0],
            }),
            [0.0; 3],
        );
        assert!(far.air_gain < near.air_gain);
        assert!(far.air_high_frequency_gain < near.air_high_frequency_gain);
        assert!(far.air_low_pass_hz < near.air_low_pass_hz);
    }

    #[test]
    fn teleport_is_not_interpreted_as_acoustic_velocity() {
        let velocity = estimate_velocity([0.0; 3], [1000.0, 0.0, 0.0], 1.0 / 60.0);
        assert_eq!(velocity, [0.0; 3]);
    }

    #[test]
    fn direct_path_processor_applies_authored_detour_delay() {
        let sample_rate = SampleRate::new(48_000).expect("sample rate");
        let channels = ChannelCount::new(1).expect("mono");
        let mut processor = DirectPathProcessor::new(sample_rate, channels);
        let params = DirectPathSnapshot {
            gain: 0.5,
            high_frequency_gain: 1.0,
            low_pass_hz: 20_000.0,
            extra_delay_ms: 10.0,
        };
        let mut rendered = Vec::with_capacity(700);
        for index in 0..700 {
            rendered.push(processor.process(if index == 0 { 1.0 } else { 0.0 }, params));
        }
        assert!(rendered[..480].iter().all(|sample| sample.abs() < 1.0e-6));
        assert!((rendered[480] - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn direct_path_processor_filters_high_frequency_energy() {
        let sample_rate = SampleRate::new(48_000).expect("sample rate");
        let channels = ChannelCount::new(1).expect("mono");
        let mut clear = DirectPathProcessor::new(sample_rate, channels);
        let mut diffracted = DirectPathProcessor::new(sample_rate, channels);
        let clear_params = DirectPathSnapshot::from_response(AudioDirectPathResponse::clear());
        let diffracted_params = DirectPathSnapshot {
            gain: 1.0,
            high_frequency_gain: 0.08,
            low_pass_hz: 1_200.0,
            extra_delay_ms: 0.0,
        };
        let mut clear_energy = 0.0_f32;
        let mut diffracted_energy = 0.0_f32;
        for index in 0..2_048 {
            let input = if index & 1 == 0 { 1.0 } else { -1.0 };
            let a = clear.process(input, clear_params);
            let b = diffracted.process(input, diffracted_params);
            if index > 128 {
                clear_energy += a * a;
                diffracted_energy += b * b;
            }
        }
        assert!(diffracted_energy < clear_energy * 0.10);
    }

    #[test]
    fn early_reflection_spread_produces_multiple_first_order_arrivals() {
        fn early_impulse_count(spread_ms: f32) -> usize {
            let sample_rate = SampleRate::new(48_000).expect("sample rate");
            let channels = ChannelCount::new(1).expect("mono");
            let mut tank = ReverbTank::new(sample_rate, channels);
            let params = ReverbSendSnapshot::from_send(AudioReverbSend {
                room_bus_id: 0,
                gain: 1.0,
                preset: newengine_audio_api::AudioReverbPreset {
                    early_reflections_gain: 1.0,
                    early_reflections_high_frequency_gain: 1.0,
                    pre_delay_ms: 5.0,
                    early_reflections_spread_ms: spread_ms,
                    decay_seconds: 0.10,
                    damping: 1.0,
                    diffusion: 0.0,
                },
                early_reflections: AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            });
            (0..1_350)
                .map(|index| tank.process(if index == 0 { 1.0 } else { 0.0 }, params))
                .filter(|sample| sample.abs() > 1.0e-4)
                .count()
        }
        let compact = early_impulse_count(0.0);
        let spread = early_impulse_count(20.0);
        assert_eq!(compact, 1);
        assert!(spread >= 4);
    }

    #[test]
    fn closed_portal_removes_direct_dry_path_without_inventing_reverb() {
        let buffer = rodio::buffer::SamplesBuffer::new(
            ChannelCount::new(1).expect("mono"),
            SampleRate::new(48_000).expect("sample rate"),
            vec![1.0_f32, 0.5, -0.25, 0.125],
        );
        let environment = AudioEnvironmentState {
            direct_path: newengine_audio_api::AudioDirectPathResponse {
                gain: 0.0,
                ..newengine_audio_api::AudioDirectPathResponse::clear()
            },
            portal_gain: 0.0,
            ..AudioEnvironmentState::clear()
        };
        let control = EnvironmentFilterControl::new(environment);
        let rendered = DynamicEnvironmentSource::new(buffer, control).collect::<Vec<_>>();
        assert!(rendered.iter().all(|sample| sample.abs() < 1.0e-6));
    }

    #[test]
    fn direct_path_gain_attenuates_only_the_direct_path() {
        let input = rodio::buffer::SamplesBuffer::new(
            ChannelCount::new(1).expect("mono"),
            SampleRate::new(48_000).expect("sample rate"),
            vec![1.0_f32, 0.0, 0.0, 0.0],
        );
        let environment = AudioEnvironmentState {
            source_send: AudioReverbSend::default(),
            listener_send: AudioReverbSend::default(),
            direct_path: newengine_audio_api::AudioDirectPathResponse {
                gain: 0.25,
                ..newengine_audio_api::AudioDirectPathResponse::clear()
            },
            portal_gain: 0.25,
        };
        let control = EnvironmentFilterControl::new(environment);
        let rendered = DynamicEnvironmentSource::new(input, control)
            .take(1)
            .collect::<Vec<_>>();
        assert_eq!(rendered.len(), 1);
        assert!((rendered[0] - 0.25).abs() < 1.0e-6);
    }

    #[test]
    fn early_reflection_material_hf_retention_filters_only_first_order_field() {
        fn early_energy(high_frequency_gain: f32) -> f32 {
            let sample_rate = SampleRate::new(48_000).expect("sample rate");
            let channels = ChannelCount::new(1).expect("mono");
            let mut tank = ReverbTank::new(sample_rate, channels);
            let params = ReverbSendSnapshot::from_send(AudioReverbSend {
                room_bus_id: 0,
                gain: 1.0,
                preset: newengine_audio_api::AudioReverbPreset {
                    early_reflections_gain: 1.0,
                    early_reflections_high_frequency_gain: high_frequency_gain,
                    pre_delay_ms: 5.0,
                    early_reflections_spread_ms: 0.0,
                    decay_seconds: 0.10,
                    damping: 1.0,
                    diffusion: 0.0,
                },
                early_reflections: AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            });
            // Nyquist-adjacent alternating excitation makes the material HF shelf observable.
            // The measurement window ends before the shortest FDN line returns, so this assertion
            // cannot accidentally pass because late-reverb damping changed.
            let mut energy = 0.0_f32;
            for frame in 0..900usize {
                let input = if frame < 128 {
                    if frame & 1 == 0 {
                        1.0
                    } else {
                        -1.0
                    }
                } else {
                    0.0
                };
                let output = tank.process(input, params);
                if (235..430).contains(&frame) {
                    energy += output * output;
                }
            }
            energy
        }

        let reflective = early_energy(1.0);
        let absorptive = early_energy(0.05);
        assert!(reflective > 1.0e-4);
        assert!(absorptive < reflective * 0.20);
    }

    #[test]
    fn spatial_environment_pans_early_reflections_by_resolved_arrival_direction() {
        fn render(direction: [f32; 3]) -> (f32, f32) {
            let mut input = vec![0.0_f32; 1_200];
            input[0] = 1.0;
            let buffer = rodio::buffer::SamplesBuffer::new(
                ChannelCount::new(1).expect("mono"),
                SampleRate::new(48_000).expect("sample rate"),
                input,
            );
            let environment = AudioEnvironmentState {
                source_send: AudioReverbSend::default(),
                listener_send: AudioReverbSend {
                    room_bus_id: 0,
                    gain: 1.0,
                    preset: newengine_audio_api::AudioReverbPreset {
                        early_reflections_gain: 1.0,
                        early_reflections_high_frequency_gain: 1.0,
                        pre_delay_ms: 5.0,
                        early_reflections_spread_ms: 0.0,
                        decay_seconds: 0.10,
                        damping: 1.0,
                        diffusion: 0.0,
                    },
                    early_reflections: AudioEarlyReflectionField::empty(),
                    early_reflection_direction: direction,
                },
                direct_path: newengine_audio_api::AudioDirectPathResponse {
                    gain: 0.0,
                    ..newengine_audio_api::AudioDirectPathResponse::clear()
                },
                portal_gain: 1.0,
            };
            let env_control = EnvironmentFilterControl::new(environment);
            let spatial_control =
                SpatialMixControl::new([0.0, 0.0, 2.0], [-0.09, 0.0, 0.0], [0.09, 0.0, 0.0]);
            let rendered =
                DynamicSpatialEnvironmentSource::new(buffer, env_control, spatial_control)
                    .take(2_200)
                    .collect::<Vec<_>>();
            let mut left = 0.0_f32;
            let mut right = 0.0_f32;
            for frame in rendered.as_chunks::<2>().0 {
                left += frame[0].abs();
                right += frame[1].abs();
            }
            (left, right)
        }

        let from_right = render([1.0, 0.0, 0.0]);
        let from_left = render([-1.0, 0.0, 0.0]);
        assert!(from_right.1 > from_right.0 * 3.0);
        assert!(from_left.0 > from_left.1 * 3.0);
    }

    #[test]
    fn explicit_early_reflection_field_renders_authored_discrete_arrival_delays() {
        let sample_rate = SampleRate::new(48_000).expect("sample rate");
        let channels = ChannelCount::new(1).expect("mono");
        let mut tank = ReverbTank::new(sample_rate, channels);
        let mut field = AudioEarlyReflectionField::empty();
        field.count = 2;
        field.taps[0] = AudioEarlyReflectionTap {
            delay_ms: 5.0,
            gain: 1.0,
            high_frequency_gain: 1.0,
            direction: [1.0, 0.0, 0.0],
            order: 1,
        };
        field.taps[1] = AudioEarlyReflectionTap {
            delay_ms: 10.0,
            gain: 0.8,
            high_frequency_gain: 1.0,
            direction: [-1.0, 0.0, 0.0],
            order: 2,
        };
        let params = ReverbSendSnapshot::from_send(AudioReverbSend {
            room_bus_id: 0,
            gain: 1.0,
            preset: newengine_audio_api::AudioReverbPreset::dry(),
            early_reflections: field,
            early_reflection_direction: [0.0; 3],
        });
        let rendered = (0..700usize)
            .map(|frame| tank.process(if frame == 0 { 1.0 } else { 0.0 }, params))
            .collect::<Vec<_>>();
        let first = rendered
            .iter()
            .enumerate()
            .find(|(_, sample)| sample.abs() > 0.9)
            .map(|(index, _)| index)
            .expect("first explicit arrival");
        let second = rendered
            .iter()
            .enumerate()
            .skip(first + 2)
            .find(|(_, sample)| sample.abs() > 0.7)
            .map(|(index, _)| index)
            .expect("second explicit arrival");
        assert!((first as isize - 240).abs() <= 1, "first={first}");
        assert!((second as isize - 480).abs() <= 1, "second={second}");
    }

    #[test]
    fn explicit_early_reflection_taps_pan_each_arrival_independently() {
        let mut input = vec![0.0_f32; 700];
        input[0] = 1.0;
        let buffer = rodio::buffer::SamplesBuffer::new(
            ChannelCount::new(1).expect("mono"),
            SampleRate::new(48_000).expect("sample rate"),
            input,
        );
        let mut field = AudioEarlyReflectionField::empty();
        field.count = 2;
        field.taps[0] = AudioEarlyReflectionTap {
            delay_ms: 5.0,
            gain: 1.0,
            high_frequency_gain: 1.0,
            direction: [1.0, 0.0, 0.0],
            order: 1,
        };
        field.taps[1] = AudioEarlyReflectionTap {
            delay_ms: 10.0,
            gain: 1.0,
            high_frequency_gain: 1.0,
            direction: [-1.0, 0.0, 0.0],
            order: 2,
        };
        let environment = AudioEnvironmentState {
            source_send: AudioReverbSend::default(),
            listener_send: AudioReverbSend {
                room_bus_id: 0,
                gain: 1.0,
                preset: newengine_audio_api::AudioReverbPreset::dry(),
                early_reflections: field,
                early_reflection_direction: [0.0; 3],
            },
            direct_path: newengine_audio_api::AudioDirectPathResponse {
                gain: 0.0,
                ..newengine_audio_api::AudioDirectPathResponse::clear()
            },
            portal_gain: 1.0,
        };
        let rendered = DynamicSpatialEnvironmentSource::new(
            buffer,
            EnvironmentFilterControl::new(environment),
            SpatialMixControl::new([0.0, 0.0, 2.0], [-0.09, 0.0, 0.0], [0.09, 0.0, 0.0]),
        )
        .take(1_200)
        .collect::<Vec<_>>();
        let frames = rendered.as_chunks::<2>().0;
        let first = frames[240];
        let second = frames[480];
        assert!(first[1].abs() > first[0].abs() * 10.0, "first={first:?}");
        assert!(
            second[0].abs() > second[1].abs() * 10.0,
            "second={second:?}"
        );
    }

    #[test]
    fn diffuse_late_tail_is_independent_of_early_reflection_direction() {
        fn render(direction: [f32; 3]) -> Vec<f32> {
            let mut input = vec![0.0_f32; 3_000];
            input[0] = 1.0;
            let buffer = rodio::buffer::SamplesBuffer::new(
                ChannelCount::new(1).expect("mono"),
                SampleRate::new(48_000).expect("sample rate"),
                input,
            );
            let mut preset = newengine_audio_api::AudioReverbPreset::room();
            preset.early_reflections_gain = 0.0;
            let environment = AudioEnvironmentState {
                source_send: AudioReverbSend::default(),
                listener_send: AudioReverbSend {
                    room_bus_id: 0,
                    gain: 1.0,
                    preset,
                    early_reflections: AudioEarlyReflectionField::empty(),
                    early_reflection_direction: direction,
                },
                direct_path: newengine_audio_api::AudioDirectPathResponse {
                    gain: 0.0,
                    ..newengine_audio_api::AudioDirectPathResponse::clear()
                },
                portal_gain: 1.0,
            };
            DynamicSpatialEnvironmentSource::new(
                buffer,
                EnvironmentFilterControl::new(environment),
                SpatialMixControl::new([0.0, 0.0, 2.0], [-0.09, 0.0, 0.0], [0.09, 0.0, 0.0]),
            )
            .take(5_600)
            .collect()
        }

        let right = render([1.0, 0.0, 0.0]);
        let left = render([-1.0, 0.0, 0.0]);
        assert_eq!(right.len(), left.len());
        assert!(right.iter().zip(&left).all(|(a, b)| (a - b).abs() < 1.0e-7));
        let stereo_difference = right
            .as_chunks::<2>()
            .0
            .iter()
            .skip(1_500)
            .map(|frame| (frame[0] - frame[1]).abs())
            .sum::<f32>();
        assert!(stereo_difference > 1.0e-4);
    }
}
