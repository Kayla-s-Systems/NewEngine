use super::*;

/// Material profiles are interpreted as nominal transmission through roughly one
/// interior-wall thickness. Bidirectional center probes scale that response from
/// actual scene geometry instead of treating a thin panel and a massive wall alike.
pub(super) const ACOUSTIC_REFERENCE_THICKNESS_M: f32 = 0.18;
const MAX_RESOLVED_OCCLUDER_THICKNESS_M: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProbeDirection {
    ListenerToEmitter,
    EmitterToListenerCenter,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PendingProbeRay {
    pub(super) emitter_key: u64,
    pub(super) listener_key: Option<u64>,
    pub(super) sample_index: u8,
    pub(super) max_t: f32,
    pub(super) direction: ProbeDirection,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct AcousticCandidate {
    pub(super) stable_key: u64,
    pub(super) position: Vec3,
    pub(super) distance: f32,
    pub(super) settings: AudioOcclusionSettings,
}

#[derive(Clone, Debug)]
pub(super) struct ProbeBlocker {
    pub(super) entity_key: u64,
    pub(super) distance: f32,
    pub(super) max_t: f32,
    pub(super) material_id: String,
    pub(super) material: AcousticMaterialProfile,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ProbeAggregate {
    pub(super) sample_count: u8,
    pub(super) blocked: u8,
    pub(super) center_blocked: bool,
    pub(super) transmission_sum: f32,
    pub(super) reflection_sum: f32,
    pub(super) absorption_sum: f32,
    pub(super) low_pass_sum: f32,
    pub(super) materials: BTreeMap<String, u8>,
    pub(super) center_forward: Option<ProbeBlocker>,
    pub(super) center_reverse: Option<ProbeBlocker>,
}

pub(crate) fn resolve_acoustic_surface_for_entity(
    world: &World,
    blocker_entity: Option<EntityId>,
) -> AcousticSurface {
    if let Some(authored) = blocker_entity
        .and_then(|entity| world.get::<AcousticSurface>(entity))
        .cloned()
        .map(AcousticSurface::sanitized)
    {
        return authored;
    }
    let surface_id = blocker_entity
        .and_then(|entity| world.get::<PhysicsSurface>(entity))
        .map(|surface| surface.id.as_str())
        .unwrap_or("surface.default");
    world
        .resource::<AcousticMaterialLibrary>()
        .and_then(|library| library.resolve(surface_id))
        .unwrap_or_else(|| {
            // Geometry remains authoritative, but an unmapped physics surface must never inherit
            // an invented concrete/material response from engine code.
            AcousticSurface::new(surface_id, AcousticMaterialProfile::transparent())
        })
}

pub(super) fn resolve_probe_blocker(
    world: &World,
    key_to_entity: &BTreeMap<u64, EntityId>,
    hit: &PhysicsQueryHitDto,
    max_t: f32,
) -> ProbeBlocker {
    let surface =
        resolve_acoustic_surface_for_entity(world, key_to_entity.get(&hit.entity).copied());
    let material_id = surface.material_id;
    let material = surface.profile;
    ProbeBlocker {
        entity_key: hit.entity,
        distance: hit.distance.max(0.0),
        max_t,
        material_id,
        material,
    }
}

/// Derives center-path complexity from the two nearest boundary hits. Matching entities
/// describe opposite faces of one closed occluder and therefore yield an actual thickness.
/// Different entities prove at least two blocker layers even though the physics API remains
/// intentionally nearest-hit-only.
pub(super) fn center_path_geometry(aggregate: &ProbeAggregate) -> (f32, u8) {
    let Some(forward) = aggregate.center_forward.as_ref() else {
        return (0.0, 0);
    };
    let Some(reverse) = aggregate.center_reverse.as_ref() else {
        return (0.0, 1);
    };
    if forward.entity_key != reverse.entity_key {
        return (0.0, 2);
    }
    let path_length = forward.max_t.min(reverse.max_t).max(0.0);
    let thickness = (path_length - forward.distance - reverse.distance)
        .clamp(0.0, MAX_RESOLVED_OCCLUDER_THICKNESS_M);
    (thickness, 1)
}

/// Scales an authored material response by measured geometric thickness. The material's
/// existing profile remains the authoring authority; geometry only changes how much of that
/// response accumulates along the direct path.
pub(super) fn material_response_for_thickness(
    material: AcousticMaterialProfile,
    thickness_m: f32,
) -> AcousticMaterialProfile {
    let material = material.sanitized();
    let exponent = (thickness_m / ACOUSTIC_REFERENCE_THICKNESS_M).clamp(0.20, 6.0);
    let transmission_gain = material.transmission_gain.powf(exponent).clamp(0.0, 1.0);
    let high_frequency_gain = material
        .high_frequency_gain()
        .powf(exponent)
        .clamp(0.0, 1.0);
    let low_pass_hz = if exponent <= 1.0 {
        20_000.0 + (material.low_pass_hz - 20_000.0) * exponent
    } else {
        material.low_pass_hz * (-(exponent - 1.0) * 0.45).exp()
    };
    AcousticMaterialProfile {
        transmission_gain,
        // Thickness affects through-material propagation, not the authored boundary reflection.
        reflection_gain: material.reflection_gain,
        high_frequency_absorption: 1.0 - high_frequency_gain,
        low_pass_hz,
    }
    .sanitized()
}

pub(super) fn combine_material_layers(
    a: AcousticMaterialProfile,
    b: AcousticMaterialProfile,
) -> AcousticMaterialProfile {
    let a = a.sanitized();
    let b = b.sanitized();
    AcousticMaterialProfile {
        transmission_gain: a.transmission_gain * b.transmission_gain,
        // A layered direct path keeps the first encountered boundary as its reflection authority.
        reflection_gain: a.reflection_gain,
        high_frequency_absorption: 1.0
            - (1.0 - a.high_frequency_absorption) * (1.0 - b.high_frequency_absorption),
        low_pass_hz: a.low_pass_hz.min(b.low_pass_hz),
    }
    .sanitized()
}

#[inline]
pub(super) fn occlusion_from_probe_coverage(obstruction: f32, center_blocked: bool) -> f32 {
    let coverage = obstruction.clamp(0.0, 1.0);
    if coverage >= 0.999 {
        return 1.0;
    }
    // The direct center path carries more perceptual weight than peripheral aperture rays.
    // Peripheral blockage still contributes a small diffuse occlusion term instead of an
    // artificial binary zero, producing smooth transitions around door frames and wall edges.
    if center_blocked {
        (0.30 + coverage * 0.70).clamp(0.0, 1.0)
    } else {
        (coverage * coverage * 0.32).clamp(0.0, 1.0)
    }
}
