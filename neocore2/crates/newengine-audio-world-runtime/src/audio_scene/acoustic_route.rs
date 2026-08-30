#[derive(Clone, Copy, Debug, PartialEq)]
struct EffectiveAcousticRoute {
    acoustic: AudioAcousticState,
    detour_delay_ms: f32,
    used_diffraction: bool,
}

impl EffectiveAcousticRoute {
    #[inline]
    const fn clear() -> Self {
        Self {
            acoustic: AudioAcousticState::clear(),
            detour_delay_ms: 0.0,
            used_diffraction: false,
        }
    }
}

#[inline]
fn array_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).max(0.0).sqrt()
}

fn diffraction_path_acoustic_state(
    path: &AudioEdgeDiffractionPathObservation,
    source_position: [f32; 3],
    listener_position: [f32; 3],
    obstruction: f32,
    occlusion: f32,
) -> Option<AudioAcousticState> {
    if !path.visible
        || !path.path_length_m.is_finite()
        || !path.excess_length_m.is_finite()
        || !path.bend_angle_radians.is_finite()
        || !path.wedge_angle_radians.is_finite()
    {
        return None;
    }
    let direct = array_distance(source_position, listener_position);
    if !direct.is_finite() || direct <= 1.0e-4 || path.path_length_m + 1.0e-4 < direct {
        return None;
    }

    let excess = path.excess_length_m.max(0.0);
    let bend = path.bend_angle_radians.clamp(0.0, std::f32::consts::PI) / std::f32::consts::PI;
    let wedge_sharpness = (1.0
        - path.wedge_angle_radians.clamp(0.0, std::f32::consts::PI) / std::f32::consts::PI)
        .clamp(0.0, 1.0);
    let spreading = (direct / path.path_length_m.max(direct))
        .clamp(0.05, 1.0)
        .sqrt();
    let excess_gain = (-0.22 * excess).exp();
    let bend_gain = (-0.85 * bend * bend).exp();
    let wedge_gain = (1.0 - 0.12 * wedge_sharpness).clamp(0.70, 1.0);

    // Edge diffraction is an alternate route around the blocker. Deliberately use boundary
    // hardness/absorption here, never material.transmission_gain: that field describes energy
    // travelling through the blocker and would double-apply wall transmission to the bypass.
    let material = path.material.sanitized();
    let boundary_gain = if path.material_known {
        0.72 + 0.28 * material.reflection_gain
    } else {
        1.0
    };
    let broadband =
        (spreading * excess_gain * bend_gain * wedge_gain * boundary_gain).clamp(0.0, 1.0);

    let material_hf = if path.material_known {
        0.35 + 0.65 * material.high_frequency_gain()
    } else {
        1.0
    };
    let hf = ((-2.20 * bend - 0.28 * excess).exp()
        * (1.0 - 0.45 * wedge_sharpness).clamp(0.35, 1.0)
        * material_hf)
        .clamp(0.0, 1.0);
    let low_pass_hz = (700.0 + 19_300.0 * hf.sqrt()).clamp(80.0, 20_000.0);

    Some(
        AudioAcousticState {
            obstruction,
            occlusion,
            transmission_gain: broadband,
            high_frequency_gain: hf,
            low_pass_hz,
        }
        .sanitized(),
    )
}

fn resolve_effective_acoustic_route(
    settings: AudioOcclusionSettings,
    occlusion_observation: Option<&AudioOcclusionObservation>,
    diffraction_observation: Option<&AudioEdgeDiffractionObservation>,
    source_position: [f32; 3],
    listener_position: [f32; 3],
    current_fixed_tick: u64,
) -> EffectiveAcousticRoute {
    let settings = settings.sanitized();
    if !settings.enabled {
        return EffectiveAcousticRoute::clear();
    }
    let Some(occlusion) = occlusion_observation.filter(|observation| {
        current_fixed_tick.saturating_sub(observation.fixed_tick)
            <= AUDIO_OCCLUSION_STALE_FIXED_TICKS
    }) else {
        return EffectiveAcousticRoute::clear();
    };

    let wall = settings.acoustic_state_with_material(
        occlusion.obstruction,
        occlusion.occlusion,
        occlusion.material,
    );
    let wall_route = EffectiveAcousticRoute {
        acoustic: wall,
        detour_delay_ms: 0.0,
        used_diffraction: false,
    };
    let Some(blocker) = occlusion.dominant_blocker_entity else {
        return wall_route;
    };
    let Some(diffraction) = diffraction_observation.filter(|observation| {
        current_fixed_tick.saturating_sub(observation.fixed_tick)
            <= AUDIO_DIFFRACTION_STALE_FIXED_TICKS
            && observation.blocker_entity == Some(blocker)
            && array_distance(observation.source_position, source_position)
                <= AUDIO_DIFFRACTION_POSITION_EPSILON_M
            && array_distance(observation.listener_position, listener_position)
                <= AUDIO_DIFFRACTION_POSITION_EPSILON_M
    }) else {
        return wall_route;
    };

    let mut best: Option<(AudioAcousticState, f32, [u32; 2])> = None;
    for path in &diffraction.paths {
        let Some(acoustic) = diffraction_path_acoustic_state(
            path,
            source_position,
            listener_position,
            occlusion.obstruction,
            occlusion.occlusion,
        ) else {
            continue;
        };
        let delay_ms =
            (path.excess_length_m.max(0.0) / SPEED_OF_SOUND_MPS * 1_000.0).clamp(0.0, 500.0);
        let replace = best
            .as_ref()
            .is_none_or(|(current, current_delay, current_edge)| {
                acoustic
                    .transmission_gain
                    .total_cmp(&current.transmission_gain)
                    .then_with(|| {
                        acoustic
                            .high_frequency_gain
                            .total_cmp(&current.high_frequency_gain)
                    })
                    .then_with(|| current_delay.total_cmp(&delay_ms))
                    .then_with(|| current_edge.cmp(&path.edge_vertex_indices).reverse())
                    .is_gt()
            });
        if replace {
            best = Some((acoustic, delay_ms, path.edge_vertex_indices));
        }
    }
    let Some((edge, delay_ms, _)) = best else {
        return wall_route;
    };
    if edge.transmission_gain <= wall.transmission_gain + 1.0e-4 {
        return wall_route;
    }
    EffectiveAcousticRoute {
        acoustic: edge,
        detour_delay_ms: delay_ms,
        used_diffraction: true,
    }
}

#[inline]
fn environment_with_indirect_occlusion(
    environment: AudioEnvironmentState,
    acoustic: AudioAcousticState,
) -> AudioEnvironmentState {
    let mut environment = environment.sanitized();
    let acoustic = acoustic.sanitized();
    if !environment.is_wet() || (acoustic.obstruction <= 1.0e-4 && acoustic.occlusion <= 1.0e-4) {
        return environment;
    }
    // Energy removed from the direct path does not simply disappear indoors. A bounded part of
    // it reaches the listener as diffuse/early room energy. Keep the authored room preset and
    // change only send strength; geometry still controls the direct transmission separately.
    let blocked_energy = (1.0 - acoustic.transmission_gain).clamp(0.0, 1.0);
    let diffuse =
        blocked_energy * (acoustic.obstruction * 0.12 + acoustic.occlusion * 0.28).clamp(0.0, 0.40);
    environment.source_send.gain = (environment.source_send.gain + diffuse).clamp(0.0, 2.0);
    environment.listener_send.gain =
        (environment.listener_send.gain + diffuse * 0.65).clamp(0.0, 2.0);
    environment.sanitized()
}

fn environment_with_effective_direct_route(
    environment: AudioEnvironmentState,
    acoustic: AudioAcousticState,
    detour_delay_ms: f32,
) -> AudioEnvironmentState {
    let mut environment = environment_with_indirect_occlusion(environment, acoustic);
    let detour_delay_ms = if detour_delay_ms.is_finite() {
        detour_delay_ms.clamp(0.0, 500.0)
    } else {
        0.0
    };
    environment.direct_path.extra_delay_ms =
        (environment.direct_path.extra_delay_ms + detour_delay_ms).clamp(0.0, 500.0);
    environment.sanitized()
}
