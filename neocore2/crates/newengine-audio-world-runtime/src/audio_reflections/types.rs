use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReflectionProbeLeg {
    Source,
    Listener,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PendingReflectionRay {
    pub(super) emitter_key: u64,
    pub(super) leg: ReflectionProbeLeg,
    pub(super) geometry: AudioFirstOrderReflectionGeometry,
    pub(super) max_t: f32,
    pub(super) source_position: [f32; 3],
    pub(super) listener_position: [f32; 3],
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct LegResolution {
    pub(super) blocked: bool,
    pub(super) endpoint_entity: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ReflectionAggregate {
    pub(super) geometry: AudioFirstOrderReflectionGeometry,
    pub(super) source_position: [f32; 3],
    pub(super) listener_position: [f32; 3],
    pub(super) source: LegResolution,
    pub(super) listener: LegResolution,
}

impl ReflectionAggregate {
    pub(super) fn new(ray: PendingReflectionRay) -> Self {
        Self {
            geometry: ray.geometry,
            source_position: ray.source_position,
            listener_position: ray.listener_position,
            source: LegResolution::default(),
            listener: LegResolution::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SecondOrderProbeLeg {
    Source,
    Middle,
    Listener,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PendingSecondOrderRay {
    pub(super) emitter_key: u64,
    pub(super) leg: SecondOrderProbeLeg,
    pub(super) geometry: AudioSecondOrderReflectionGeometry,
    pub(super) max_t: f32,
    pub(super) source_position: [f32; 3],
    pub(super) listener_position: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SecondOrderAggregate {
    pub(super) geometry: AudioSecondOrderReflectionGeometry,
    pub(super) source_position: [f32; 3],
    pub(super) listener_position: [f32; 3],
    pub(super) source: LegResolution,
    pub(super) middle_blocked: bool,
    pub(super) listener: LegResolution,
}

impl SecondOrderAggregate {
    pub(super) fn new(ray: PendingSecondOrderRay) -> Self {
        Self {
            geometry: ray.geometry,
            source_position: ray.source_position,
            listener_position: ray.listener_position,
            source: LegResolution::default(),
            middle_blocked: false,
            listener: LegResolution::default(),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ReflectionEmitterCandidate {
    pub(super) key: u64,
    pub(super) position: Vec3,
    pub(super) distance: f32,
}
