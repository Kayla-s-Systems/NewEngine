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
                gain: 0.7,
                preset: newengine_audio_api::AudioReverbPreset::room(),
            },
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
}
