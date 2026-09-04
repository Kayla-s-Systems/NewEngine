#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitter_defaults_to_enabled_spatial_autoplay() {
        let emitter = AudioEmitter::new("shared/audio/ambience/wind.xvag");
        assert!(emitter.enabled);
        assert!(emitter.autoplay);
        assert!(emitter.spatial);
        assert!(!emitter.looping);
        assert!(emitter.attenuation.is_none());
        assert_eq!(emitter.sanitized_gain(), 1.0);
    }

    #[test]
    fn emitter_gain_is_sanitized_before_crossing_provider_boundary() {
        let mut emitter = AudioEmitter {
            gain: f32::INFINITY,
            ..AudioEmitter::default()
        };
        assert_eq!(emitter.sanitized_gain(), 1.0);
        emitter.gain = 99.0;
        assert_eq!(emitter.sanitized_gain(), 4.0);
    }

    #[test]
    fn emitter_runtime_defaults_to_clear_acoustic_transmission() {
        let runtime = AudioEmitterRuntime::default();
        assert_eq!(runtime.obstruction, 0.0);
        assert_eq!(runtime.occlusion, 0.0);
        assert_eq!(runtime.transmission_gain, 1.0);
    }

    fn test_material(transmission_gain: f32) -> newengine_audio_api::AcousticMaterialProfile {
        newengine_audio_api::AcousticMaterialProfile {
            transmission_gain,
            reflection_gain: 0.82,
            high_frequency_absorption: 0.24,
            low_pass_hz: 8_500.0,
        }
    }

    fn blocked_observation(
        fixed_tick: u64,
        blocker: u64,
        material: newengine_audio_api::AcousticMaterialProfile,
    ) -> AudioOcclusionObservation {
        AudioOcclusionObservation {
            fixed_tick,
            samples: 3,
            blocked_samples: 3,
            obstruction: 1.0,
            occlusion: 1.0,
            estimated_thickness_m: 1.0,
            center_blocker_layers: 1,
            dominant_blocker_entity: Some(blocker),
            dominant_material: "surface.test".to_owned(),
            material,
        }
    }

    fn diffraction_path(
        visible: bool,
        excess_length_m: f32,
        bend_angle_radians: f32,
        material: newengine_audio_api::AcousticMaterialProfile,
    ) -> AudioEdgeDiffractionPathObservation {
        AudioEdgeDiffractionPathObservation {
            edge_vertex_indices: [2, 7],
            visible,
            diffraction_point: [2.0, 1.0, 0.0],
            arrival_direction: [0.0, 1.0, 0.0],
            path_length_m: 4.0 + excess_length_m,
            excess_length_m,
            bend_angle_radians,
            wedge_angle_radians: std::f32::consts::FRAC_PI_2,
            material_known: true,
            material,
        }
    }

    fn diffraction_observation(
        fixed_tick: u64,
        blocker: u64,
        path: AudioEdgeDiffractionPathObservation,
    ) -> AudioEdgeDiffractionObservation {
        AudioEdgeDiffractionObservation {
            fixed_tick,
            source_position: [4.0, 0.0, 0.0],
            listener_position: [0.0, 0.0, 0.0],
            blocker_entity: Some(blocker),
            paths: vec![path],
        }
    }

    #[test]
    fn stale_diffraction_falls_back_to_through_wall_route() {
        let settings = AudioOcclusionSettings::default();
        let wall = blocked_observation(20, 77, test_material(0.08));
        let edge = diffraction_observation(
            10,
            77,
            diffraction_path(true, 0.4, 0.5, test_material(0.95)),
        );
        let route = resolve_effective_acoustic_route(
            settings,
            Some(&wall),
            Some(&edge),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            20,
        );
        let expected = settings.acoustic_state_with_material(1.0, 1.0, wall.material);
        assert!(!route.used_diffraction);
        assert_eq!(route.detour_delay_ms, 0.0);
        assert_eq!(route.acoustic, expected);
    }

    #[test]
    fn blocker_mismatch_rejects_unrelated_scene_edge() {
        let settings = AudioOcclusionSettings::default();
        let wall = blocked_observation(30, 77, test_material(0.08));
        let edge = diffraction_observation(
            30,
            88,
            diffraction_path(true, 0.3, 0.4, test_material(0.95)),
        );
        let route = resolve_effective_acoustic_route(
            settings,
            Some(&wall),
            Some(&edge),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            30,
        );
        assert!(!route.used_diffraction);
        assert_eq!(route.detour_delay_ms, 0.0);
    }

    #[test]
    fn blocked_edge_visibility_cannot_bypass_wall() {
        let settings = AudioOcclusionSettings::default();
        let wall = blocked_observation(40, 77, test_material(0.05));
        let edge = diffraction_observation(
            40,
            77,
            diffraction_path(false, 0.2, 0.25, test_material(0.95)),
        );
        let route = resolve_effective_acoustic_route(
            settings,
            Some(&wall),
            Some(&edge),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            40,
        );
        assert!(!route.used_diffraction);
    }

    #[test]
    fn strongest_route_selects_diffraction_or_wall_by_broadband_energy() {
        let edge = diffraction_observation(
            50,
            77,
            diffraction_path(true, 0.3, 0.35, test_material(0.95)),
        );
        let heavy_wall = blocked_observation(50, 77, test_material(0.04));
        let edge_route = resolve_effective_acoustic_route(
            AudioOcclusionSettings::default(),
            Some(&heavy_wall),
            Some(&edge),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            50,
        );
        assert!(edge_route.used_diffraction);
        assert!(edge_route.detour_delay_ms > 0.0);

        let permissive_settings = AudioOcclusionSettings {
            obstruction_gain: 0.97,
            occlusion_gain: 0.97,
            ..AudioOcclusionSettings::default()
        };
        let light_wall = blocked_observation(
            50,
            77,
            newengine_audio_api::AcousticMaterialProfile::transparent(),
        );
        let wall_route = resolve_effective_acoustic_route(
            permissive_settings,
            Some(&light_wall),
            Some(&edge),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            50,
        );
        assert!(!wall_route.used_diffraction);
        assert_eq!(wall_route.detour_delay_ms, 0.0);
    }

    #[test]
    fn diffraction_hf_loss_increases_with_bend_and_excess_distance() {
        let material = test_material(0.5);
        let mild = diffraction_path_acoustic_state(
            &diffraction_path(true, 0.10, 0.20, material),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            1.0,
            1.0,
        )
        .expect("mild edge");
        let severe = diffraction_path_acoustic_state(
            &diffraction_path(true, 1.20, 1.40, material),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            1.0,
            1.0,
        )
        .expect("severe edge");
        assert!(severe.transmission_gain < mild.transmission_gain);
        assert!(severe.high_frequency_gain < mild.high_frequency_gain);
        assert!(severe.low_pass_hz < mild.low_pass_hz);
    }

    #[test]
    fn diffraction_route_never_reuses_wall_transmission_gain() {
        let opaque = diffraction_path_acoustic_state(
            &diffraction_path(true, 0.4, 0.6, test_material(0.05)),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            1.0,
            1.0,
        )
        .expect("opaque material edge");
        let transmissive = diffraction_path_acoustic_state(
            &diffraction_path(true, 0.4, 0.6, test_material(0.95)),
            [4.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            1.0,
            1.0,
        )
        .expect("transmissive material edge");
        assert_eq!(opaque.obstruction, transmissive.obstruction);
        assert_eq!(opaque.occlusion, transmissive.occlusion);
        assert!((opaque.transmission_gain - transmissive.transmission_gain).abs() < 1.0e-5);
        assert!((opaque.high_frequency_gain - transmissive.high_frequency_gain).abs() < 1.0e-5);
        assert!((opaque.low_pass_hz - transmissive.low_pass_hz).abs() < 0.01);
    }

    #[test]
    fn diffraction_detour_does_not_double_apply_same_room_attenuation() {
        let clear = AudioEnvironmentState::clear();
        let acoustic = AudioAcousticState {
            obstruction: 1.0,
            occlusion: 1.0,
            transmission_gain: 0.55,
            high_frequency_gain: 0.42,
            low_pass_hz: 5_500.0,
        };
        let routed = environment_with_effective_direct_route(clear, acoustic, 12.5);
        assert_eq!(routed.direct_path.gain, 1.0);
        assert_eq!(routed.direct_path.high_frequency_gain, 1.0);
        assert_eq!(routed.direct_path.low_pass_hz, 20_000.0);
        assert!((routed.direct_path.extra_delay_ms - 12.5).abs() < 1.0e-6);
    }

    #[test]
    fn occlusion_redirects_some_direct_energy_into_existing_room_tail() {
        let dry = AudioEnvironmentState::clear();
        let blocked = AudioAcousticState {
            obstruction: 1.0,
            occlusion: 1.0,
            transmission_gain: 0.2,
            high_frequency_gain: 0.3,
            low_pass_hz: 1_500.0,
        };
        assert_eq!(environment_with_indirect_occlusion(dry, blocked), dry);

        let room = AudioEnvironmentState {
            source_send: newengine_audio_api::AudioReverbSend {
                room_bus_id: 0,
                gain: 0.30,
                preset: newengine_audio_api::AudioReverbPreset::room(),
                early_reflections: newengine_audio_api::AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            },
            listener_send: newengine_audio_api::AudioReverbSend {
                room_bus_id: 0,
                gain: 0.20,
                preset: newengine_audio_api::AudioReverbPreset::room(),
                early_reflections: newengine_audio_api::AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            },
            direct_path: newengine_audio_api::AudioDirectPathResponse::clear(),
            portal_gain: 1.0,
        };
        let indirect = environment_with_indirect_occlusion(room, blocked);
        assert!(indirect.source_send.gain > room.source_send.gain);
        assert!(indirect.listener_send.gain > room.listener_send.gain);
        assert_eq!(indirect.portal_gain, room.portal_gain);
    }
}
