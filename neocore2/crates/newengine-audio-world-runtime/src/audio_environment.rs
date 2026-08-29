#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};

use newengine_audio_api::{
    AudioDirectPathResponse, AudioEarlyReflectionField, AudioEarlyReflectionTap,
    AudioEnvironmentKind, AudioEnvironmentState, AudioEnvironmentZone, AudioPortal,
    AudioReverbPreset, AudioReverbSend, AUDIO_MAX_EARLY_REFLECTION_TAPS,
};
use newengine_ecs::{EntityId, World};
use newengine_math::{Quat, Vec3};
use newengine_transform::{read_entity_world_pose_local_chain, GlobalTransform, Transform};

use crate::{AudioEarlyReflectionObservation, AudioListenerRuntimeState};

#[derive(Clone, Debug)]
struct ResolvedEnvironmentZone {
    stable_key: u64,
    zone: AudioEnvironmentZone,
    center: Vec3,
    rotation: Quat,
    half_extents: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioEnvironmentResolution {
    pub emitter_zone: String,
    pub listener_zone: String,
    pub portal_gain: f32,
    pub transition_seconds: f32,
    pub state: AudioEnvironmentState,
}

impl Default for AudioEnvironmentResolution {
    fn default() -> Self {
        Self {
            emitter_zone: String::new(),
            listener_zone: String::new(),
            portal_gain: 1.0,
            transition_seconds: 0.18,
            state: AudioEnvironmentState::clear(),
        }
    }
}

#[derive(Clone, Debug)]
struct ZoneMembership {
    zone_index: usize,
    influence: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct PortalRoute {
    gain: f32,
    /// Ordered from listener zone outward toward the destination zone.
    portal_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioEnvironmentRuntimeState {
    pub listener_zone: String,
    pub listener_ready: bool,
    pub listener_outdoor: bool,
    pub zone_count: usize,
    pub portal_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct AudioEnvironmentFrame {
    zones: Vec<ResolvedEnvironmentZone>,
    portals: Vec<AudioPortal>,
    listener_membership: Option<ZoneMembership>,
    route_gains: BTreeMap<String, f32>,
    indirect_routes: BTreeMap<String, PortalRoute>,
    direct_routes: BTreeMap<String, PortalRoute>,
    portal_centers: BTreeMap<String, Vec3>,
    reflection_observations: BTreeMap<u64, AudioEarlyReflectionObservation>,
    listener_position: Vec3,
    listener_ready: bool,
}

impl AudioEnvironmentFrame {
    pub fn snapshot(world: &World) -> Self {
        let listener_position = world.resource::<AudioListenerRuntimeState>().map(|state| {
            Vec3::new(
                state.listener.position[0],
                state.listener.position[1],
                state.listener.position[2],
            )
        });
        let mut frame = Self::snapshot_at(world, listener_position.unwrap_or(Vec3::ZERO));
        if listener_position.is_none() {
            frame.listener_membership = None;
            frame.route_gains.clear();
            frame.indirect_routes.clear();
            frame.direct_routes.clear();
            frame.portal_centers.clear();
            frame.listener_ready = false;
        }
        frame
    }

    pub fn snapshot_at(world: &World, listener_position: Vec3) -> Self {
        let mut zones = Vec::new();
        let mut portals = Vec::new();
        let mut portal_centers = BTreeMap::new();
        let mut reflection_observations = BTreeMap::new();

        for entity in world.iter_entities() {
            if let Some(observation) = world
                .get::<AudioEarlyReflectionObservation>(entity)
                .cloned()
            {
                reflection_observations.insert(entity.stable_u64(), observation);
            }
            if let Some(zone) = world.get::<AudioEnvironmentZone>(entity).cloned() {
                let zone = zone.sanitized();
                if zone.enabled {
                    if let Some((center, rotation)) =
                        read_entity_world_pose_local_chain(world, entity).or_else(|| {
                            world.get::<Transform>(entity).map(|transform| {
                                (
                                    transform.position,
                                    transform.rotation.normalize_or_identity(),
                                )
                            })
                        })
                    {
                        let scale = world_scale(world, entity);
                        let half_extents = Vec3::new(
                            zone.half_extents[0] * scale.x.abs().max(1.0e-4),
                            zone.half_extents[1] * scale.y.abs().max(1.0e-4),
                            zone.half_extents[2] * scale.z.abs().max(1.0e-4),
                        );
                        zones.push(ResolvedEnvironmentZone {
                            stable_key: entity.stable_u64(),
                            zone,
                            center,
                            rotation: rotation.normalize_or_identity(),
                            half_extents,
                        });
                    }
                }
            }
            if let Some(portal) = world.get::<AudioPortal>(entity).cloned() {
                let mut portal = portal.sanitized();
                if portal.enabled && (portal.route_gain() > 0.0 || portal.direct_route_gain() > 0.0)
                {
                    if world.get::<GlobalTransform>(entity).is_some()
                        || world.get::<Transform>(entity).is_some()
                    {
                        if let Some((center, _)) = read_entity_world_pose_local_chain(world, entity)
                            .or_else(|| {
                                world.get::<Transform>(entity).map(|transform| {
                                    (
                                        transform.position,
                                        transform.rotation.normalize_or_identity(),
                                    )
                                })
                            })
                        {
                            portal_centers.insert(portal.portal_id.clone(), center);
                            let scale = world_scale(world, entity);
                            portal.half_extents[0] *= scale.x.abs().max(1.0e-4);
                            portal.half_extents[1] *= scale.y.abs().max(1.0e-4);
                        }
                    }
                    portals.push(portal);
                }
            }
        }

        zones.sort_by(|a, b| {
            a.zone
                .zone_id
                .cmp(&b.zone.zone_id)
                .then_with(|| a.stable_key.cmp(&b.stable_key))
        });
        portals.sort_by(|a, b| a.portal_id.cmp(&b.portal_id));

        let listener_membership = select_membership(&zones, listener_position);
        let indirect_routes = listener_membership
            .as_ref()
            .map(|membership| {
                strongest_portal_route_map(
                    &zones,
                    &portals,
                    membership.zone_index,
                    PortalRouteMetric::Indirect,
                )
            })
            .unwrap_or_default();
        let route_gains = zones
            .iter()
            .map(|zone| {
                let gain = indirect_routes
                    .get(&zone.zone.zone_id)
                    .map(|route| route.gain)
                    .unwrap_or(0.0);
                (zone.zone.zone_id.clone(), gain)
            })
            .collect::<BTreeMap<_, _>>();
        let direct_routes = listener_membership
            .as_ref()
            .map(|membership| {
                strongest_direct_portal_routes(&zones, &portals, membership.zone_index)
            })
            .unwrap_or_default();

        Self {
            zones,
            portals,
            listener_membership,
            route_gains,
            indirect_routes,
            direct_routes,
            portal_centers,
            reflection_observations,
            listener_position,
            listener_ready: true,
        }
    }

    pub fn resolve(&self, emitter_position: [f32; 3]) -> AudioEnvironmentResolution {
        self.resolve_at(Vec3::new(
            emitter_position[0],
            emitter_position[1],
            emitter_position[2],
        ))
    }

    pub fn resolve_for_emitter(
        &self,
        emitter_key: u64,
        emitter_position: [f32; 3],
    ) -> AudioEnvironmentResolution {
        self.resolve_at_internal(
            Some(emitter_key),
            Vec3::new(
                emitter_position[0],
                emitter_position[1],
                emitter_position[2],
            ),
        )
    }

    pub fn resolve_at(&self, emitter_position: Vec3) -> AudioEnvironmentResolution {
        self.resolve_at_internal(None, emitter_position)
    }

    fn resolve_at_internal(
        &self,
        emitter_key: Option<u64>,
        emitter_position: Vec3,
    ) -> AudioEnvironmentResolution {
        let emitter_membership = select_membership(&self.zones, emitter_position);
        let listener_membership = self.listener_membership.as_ref();

        let emitter_zone = emitter_membership
            .as_ref()
            .map(|membership| &self.zones[membership.zone_index]);
        let listener_zone =
            listener_membership.map(|membership| &self.zones[membership.zone_index]);

        let mut resolution = AudioEnvironmentResolution {
            emitter_zone: emitter_zone
                .map(|zone| zone.zone.zone_id.clone())
                .unwrap_or_default(),
            listener_zone: listener_zone
                .map(|zone| zone.zone.zone_id.clone())
                .unwrap_or_default(),
            ..AudioEnvironmentResolution::default()
        };

        match (
            emitter_membership.as_ref(),
            listener_membership,
            emitter_zone,
            listener_zone,
        ) {
            (
                Some(emitter_membership),
                Some(listener_membership),
                Some(emitter),
                Some(listener),
            ) if emitter.zone.zone_id == listener.zone.zone_id => {
                resolution.portal_gain = 1.0;
                resolution.transition_seconds = emitter
                    .zone
                    .transition_seconds
                    .max(listener.zone.transition_seconds);
                let reflection_observation =
                    emitter_key.and_then(|key| self.reflection_observations.get(&key));
                let listener_preset = geometry_adjusted_reverb(
                    listener,
                    emitter_position,
                    self.listener_position,
                    listener.zone.reverb,
                    reflection_observation,
                );
                let early_reflections = explicit_early_reflection_field(
                    listener.zone.reverb,
                    emitter_position,
                    self.listener_position,
                    reflection_observation,
                );
                let early_reflection_direction = early_reflections
                    .active()
                    .first()
                    .map(|tap| tap.direction)
                    .or_else(|| {
                        fresh_reflection_observation(
                            reflection_observation,
                            emitter_position,
                            self.listener_position,
                        )
                        .and_then(|observation| observation.paths.iter().find(|path| path.visible))
                        .map(|path| path.arrival_direction)
                    })
                    .unwrap_or([0.0; 3]);
                resolution.state = AudioEnvironmentState {
                    source_send: AudioReverbSend::default(),
                    listener_send: AudioReverbSend {
                        room_bus_id: listener.stable_key,
                        gain: listener.zone.send_gain
                            * emitter_membership.influence
                            * listener_membership.influence,
                        preset: listener_preset,
                        early_reflections,
                        early_reflection_direction,
                    },
                    direct_path: AudioDirectPathResponse::clear(),
                    portal_gain: 1.0,
                }
                .sanitized();
            }
            (
                Some(emitter_membership),
                Some(listener_membership),
                Some(emitter),
                Some(listener),
            ) => {
                let route_gain = self
                    .route_gains
                    .get(&emitter.zone.zone_id)
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(0.0, 1.0);
                resolution.portal_gain = route_gain;
                resolution.transition_seconds = emitter
                    .zone
                    .transition_seconds
                    .max(listener.zone.transition_seconds);
                let direct_route = self.direct_routes.get(&emitter.zone.zone_id);
                let direct_route_gain = direct_route.map(|route| route.gain).unwrap_or(0.0);
                let geometric_direct = direct_route.and_then(|route| {
                    direct_portal_route_response(
                        route,
                        &self.portals,
                        &self.portal_centers,
                        emitter_position,
                        self.listener_position,
                    )
                });
                let direct_path = geometric_direct.map(|(response, _, _)| response).unwrap_or(
                    AudioDirectPathResponse {
                        gain: direct_route_gain,
                        ..AudioDirectPathResponse::clear()
                    },
                );
                let indirect_boundaries = self
                    .indirect_routes
                    .get(&emitter.zone.zone_id)
                    .and_then(|route| portal_route_boundary_centers(route, &self.portal_centers));
                let (source_reverb_boundary, listener_reverb_boundary) =
                    indirect_boundaries.unwrap_or((None, None));
                let source_preset = source_reverb_boundary
                    .map(|center| {
                        geometry_adjusted_reverb(
                            emitter,
                            emitter_position,
                            center,
                            emitter.zone.reverb,
                            None,
                        )
                    })
                    .unwrap_or(emitter.zone.reverb);
                let listener_preset = listener_reverb_boundary
                    .map(|center| {
                        geometry_adjusted_reverb(
                            listener,
                            center,
                            self.listener_position,
                            listener.zone.reverb,
                            None,
                        )
                    })
                    .unwrap_or(listener.zone.reverb);
                let indirect_arrival_direction = listener_reverb_boundary
                    .map(|center| direction_array(self.listener_position, center))
                    .unwrap_or([0.0; 3]);
                resolution.state = AudioEnvironmentState {
                    source_send: AudioReverbSend {
                        room_bus_id: emitter.stable_key,
                        gain: emitter.zone.send_gain * emitter_membership.influence * route_gain,
                        preset: source_preset,
                        early_reflections: AudioEarlyReflectionField::empty(),
                        early_reflection_direction: indirect_arrival_direction,
                    },
                    listener_send: AudioReverbSend {
                        room_bus_id: listener.stable_key,
                        gain: listener.zone.send_gain * listener_membership.influence * route_gain,
                        preset: listener_preset,
                        early_reflections: AudioEarlyReflectionField::empty(),
                        early_reflection_direction: indirect_arrival_direction,
                    },
                    direct_path,
                    portal_gain: route_gain,
                }
                .sanitized();
            }
            _ => {
                resolution.state = AudioEnvironmentState::clear();
                resolution.portal_gain = 0.0;
                resolution.state.direct_path.gain = 0.0;
                resolution.state.portal_gain = 0.0;
            }
        }

        resolution
    }

    #[inline]
    pub fn zone_count(&self) -> usize {
        self.zones.len()
    }

    #[inline]
    pub fn portal_count(&self) -> usize {
        self.portals.len()
    }

    pub fn listener_zone_id(&self) -> Option<&str> {
        self.listener_membership
            .as_ref()
            .map(|membership| self.zones[membership.zone_index].zone.zone_id.as_str())
    }

    #[inline]
    pub fn listener_ready(&self) -> bool {
        self.listener_ready
    }

    pub fn listener_is_outdoor(&self) -> bool {
        self.listener_membership
            .as_ref()
            .map(|membership| {
                self.zones[membership.zone_index].zone.kind == AudioEnvironmentKind::Outdoor
            })
            .unwrap_or(true)
    }

    pub fn route_gain_to_zone(&self, zone_id: &str) -> f32 {
        let zone_id = zone_id.trim();
        if zone_id.is_empty() {
            return 0.0;
        }
        if self
            .listener_zone_id()
            .is_some_and(|listener| listener == zone_id)
        {
            return 1.0;
        }
        self.route_gains
            .get(zone_id)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
    }

    pub fn listener_environment_state(&self) -> AudioEnvironmentState {
        let Some(membership) = self.listener_membership.as_ref() else {
            return AudioEnvironmentState::clear();
        };
        let resolved_zone = &self.zones[membership.zone_index];
        let zone = &resolved_zone.zone;
        AudioEnvironmentState {
            source_send: AudioReverbSend::default(),
            listener_send: AudioReverbSend {
                room_bus_id: resolved_zone.stable_key,
                gain: zone.send_gain * membership.influence,
                preset: zone.reverb,
                early_reflections: AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            },
            direct_path: AudioDirectPathResponse::clear(),
            portal_gain: 1.0,
        }
        .sanitized()
    }

    pub fn runtime_state(&self) -> AudioEnvironmentRuntimeState {
        AudioEnvironmentRuntimeState {
            listener_zone: self.listener_zone_id().unwrap_or_default().to_owned(),
            listener_ready: self.listener_ready(),
            listener_outdoor: self.listener_is_outdoor(),
            zone_count: self.zone_count(),
            portal_count: self.portal_count(),
        }
    }
}

fn world_scale(world: &World, entity: EntityId) -> Vec3 {
    if let Some(global) = world.get::<GlobalTransform>(entity) {
        let (scale, _, _) = global.0.to_scale_rotation_translation();
        if scale.x.is_finite() && scale.y.is_finite() && scale.z.is_finite() {
            return scale;
        }
    }
    world
        .get::<Transform>(entity)
        .map(|transform| transform.scale)
        .unwrap_or(Vec3::ONE)
}

fn zone_metrics(zone: &ResolvedEnvironmentZone, point: Vec3) -> Option<(f32, f32)> {
    let local = zone.rotation.inverse() * (point - zone.center);
    let ax = local.x.abs();
    let ay = local.y.abs();
    let az = local.z.abs();
    if ax > zone.half_extents.x || ay > zone.half_extents.y || az > zone.half_extents.z {
        return None;
    }

    let dx = zone.half_extents.x - ax;
    let dy = zone.half_extents.y - ay;
    let dz = zone.half_extents.z - az;
    let edge_distance = dx.min(dy).min(dz).max(0.0);
    let blend = zone.zone.blend_distance;
    let influence = if blend <= 1.0e-5 {
        1.0
    } else {
        (edge_distance / blend).clamp(0.0, 1.0)
    };
    let normalized_center_distance = (ax / zone.half_extents.x.max(1.0e-5))
        .max(ay / zone.half_extents.y.max(1.0e-5))
        .max(az / zone.half_extents.z.max(1.0e-5));
    Some((influence, normalized_center_distance))
}

fn select_membership(zones: &[ResolvedEnvironmentZone], point: Vec3) -> Option<ZoneMembership> {
    let mut candidates = zones
        .iter()
        .enumerate()
        .filter_map(|(zone_index, zone)| {
            zone_metrics(zone, point).map(|(influence, normalized_center_distance)| {
                (
                    zone_index,
                    zone.zone.priority,
                    normalized_center_distance,
                    influence,
                    zone.stable_key,
                )
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.2.total_cmp(&b.2))
            .then_with(|| a.4.cmp(&b.4))
    });
    candidates
        .first()
        .map(|(zone_index, _, _, influence, _)| ZoneMembership {
            zone_index: *zone_index,
            influence: *influence,
        })
}

const SPEED_OF_SOUND_MPS: f32 = 343.0;

fn fresh_reflection_observation(
    observation: Option<&AudioEarlyReflectionObservation>,
    source_world: Vec3,
    receiver_world: Vec3,
) -> Option<&AudioEarlyReflectionObservation> {
    observation.filter(|observation| {
        vec3_array_distance(observation.source_position, source_world) <= 0.75
            && vec3_array_distance(observation.listener_position, receiver_world) <= 0.75
            && !observation.paths.is_empty()
    })
}

#[inline]
fn direction_array(origin: Vec3, target: Vec3) -> [f32; 3] {
    let delta = target - origin;
    let length = delta.length();
    if !length.is_finite() || length <= 1.0e-5 {
        [0.0; 3]
    } else {
        [delta.x / length, delta.y / length, delta.z / length]
    }
}

fn explicit_early_reflection_field(
    preset: AudioReverbPreset,
    source_world: Vec3,
    receiver_world: Vec3,
    observation: Option<&AudioEarlyReflectionObservation>,
) -> AudioEarlyReflectionField {
    let Some(observation) = fresh_reflection_observation(observation, source_world, receiver_world)
    else {
        return AudioEarlyReflectionField::empty();
    };
    let preset = preset.sanitized();
    if preset.early_reflections_gain <= 1.0e-5 {
        return AudioEarlyReflectionField::empty();
    }
    let direct = source_world.distance(receiver_world).max(1.0e-4);
    let mut candidates =
        Vec::with_capacity(observation.paths.len() + observation.second_order_paths.len());
    for path in observation
        .paths
        .iter()
        .filter(|path| path.visible && path.path_length_m.is_finite())
    {
        let ratio = (direct / path.path_length_m.max(direct))
            .clamp(0.2, 1.0)
            .sqrt();
        let material = path.material.sanitized();
        let material_gain = if path.material_known {
            material.reflection_gain
        } else {
            1.0
        };
        let high_frequency_gain = if path.material_known {
            material.high_frequency_gain()
        } else {
            1.0
        };
        candidates.push(AudioEarlyReflectionTap {
            delay_ms: (path.excess_length_m.max(0.0) / SPEED_OF_SOUND_MPS * 1_000.0)
                .clamp(0.0, 500.0),
            gain: (preset.early_reflections_gain * ratio * material_gain).clamp(0.0, 2.0),
            high_frequency_gain,
            direction: path.arrival_direction,
            order: 1,
        });
    }
    for path in observation
        .second_order_paths
        .iter()
        .filter(|path| path.visible && path.path_length_m.is_finite())
    {
        let ratio = (direct / path.path_length_m.max(direct))
            .clamp(0.2, 1.0)
            .sqrt();
        let mut material_gain = 1.0_f32;
        let mut high_frequency_gain = 1.0_f32;
        for bounce in 0..2 {
            if path.material_known[bounce] {
                let material = path.materials[bounce].sanitized();
                material_gain *= material.reflection_gain;
                high_frequency_gain *= material.high_frequency_gain();
            }
        }
        candidates.push(AudioEarlyReflectionTap {
            delay_ms: (path.excess_length_m.max(0.0) / SPEED_OF_SOUND_MPS * 1_000.0)
                .clamp(0.0, 500.0),
            gain: (preset.early_reflections_gain * ratio * material_gain).clamp(0.0, 2.0),
            high_frequency_gain: high_frequency_gain.clamp(0.0, 1.0),
            direction: path.arrival_direction,
            order: 2,
        });
    }
    candidates.retain(|tap| tap.gain > 1.0e-5);
    candidates.sort_by(|a, b| {
        b.gain
            .total_cmp(&a.gain)
            .then_with(|| a.delay_ms.total_cmp(&b.delay_ms))
            .then_with(|| a.order.cmp(&b.order))
    });
    candidates.truncate(AUDIO_MAX_EARLY_REFLECTION_TAPS);
    let mut field = AudioEarlyReflectionField::empty();
    field.count = candidates.len() as u8;
    for (slot, tap) in field.taps.iter_mut().zip(candidates) {
        *slot = tap;
    }
    field.sanitized()
}

fn geometry_adjusted_reverb(
    zone: &ResolvedEnvironmentZone,
    source_world: Vec3,
    receiver_world: Vec3,
    preset: AudioReverbPreset,
    observation: Option<&AudioEarlyReflectionObservation>,
) -> AudioReverbPreset {
    let preset = preset.sanitized();
    if zone.zone.kind == AudioEnvironmentKind::Outdoor {
        return preset;
    }

    if let Some(observation) =
        fresh_reflection_observation(observation, source_world, receiver_world)
    {
        let mut visible = observation
            .paths
            .iter()
            .filter(|path| path.visible && path.path_length_m.is_finite())
            .collect::<Vec<_>>();
        visible.sort_by(|a, b| {
            a.path_length_m
                .total_cmp(&b.path_length_m)
                .then_with(|| a.face_index.cmp(&b.face_index))
        });
        if visible.is_empty() {
            return AudioReverbPreset {
                early_reflections_gain: 0.0,
                early_reflections_high_frequency_gain: 0.0,
                ..preset
            }
            .sanitized();
        }

        let direct = source_world.distance(receiver_world).max(1.0e-4);
        let first_excess = visible[0].excess_length_m.max(0.0);
        let fourth_index = visible.len().min(4).saturating_sub(1);
        let fourth_excess = visible[fourth_index].excess_length_m.max(first_excess);
        let visibility =
            (visible.len() as f32 / observation.paths.len().max(1) as f32).clamp(0.0, 1.0);
        let mut broadband_sum = 0.0_f32;
        let mut hf_sum = 0.0_f32;
        let mut weight_sum = 0.0_f32;
        for path in visible.iter().take(4) {
            let ratio = (direct / path.path_length_m.max(direct))
                .clamp(0.2, 1.0)
                .sqrt();
            let material_gain = if path.material_known {
                path.material.sanitized().reflection_gain
            } else {
                1.0
            };
            let hf_gain = if path.material_known {
                path.material.sanitized().high_frequency_gain()
            } else {
                1.0
            };
            broadband_sum += ratio * material_gain;
            hf_sum += ratio * material_gain * hf_gain;
            weight_sum += ratio * material_gain;
        }
        let broadband = if visible.is_empty() {
            0.0
        } else {
            broadband_sum / visible.len().min(4) as f32
        };
        let hf_retention = if weight_sum > 1.0e-5 {
            (hf_sum / weight_sum).clamp(0.0, 1.0)
        } else {
            0.0
        };
        return AudioReverbPreset {
            early_reflections_gain: (preset.early_reflections_gain * broadband * visibility.sqrt())
                .clamp(0.0, 2.0),
            early_reflections_high_frequency_gain: hf_retention,
            pre_delay_ms: (first_excess / SPEED_OF_SOUND_MPS * 1_000.0).clamp(0.0, 250.0),
            early_reflections_spread_ms: ((fourth_excess - first_excess) / SPEED_OF_SOUND_MPS
                * 1_000.0)
                .clamp(0.0, 250.0),
            ..preset
        }
        .sanitized();
    }

    // No fresh visibility/material observation yet: retain the geometric first-order baseline.
    let inverse = zone.rotation.inverse();
    let source = inverse * (source_world - zone.center);
    let receiver = inverse * (receiver_world - zone.center);
    let direct = source.distance(receiver).max(1.0e-4);
    let extents = zone.half_extents;
    let mut excess = [0.0_f32; 6];
    let faces = [
        (0usize, extents.x),
        (0usize, -extents.x),
        (1usize, extents.y),
        (1usize, -extents.y),
        (2usize, extents.z),
        (2usize, -extents.z),
    ];
    for (index, (axis, plane)) in faces.into_iter().enumerate() {
        let mut mirrored = source;
        match axis {
            0 => mirrored.x = 2.0 * plane - source.x,
            1 => mirrored.y = 2.0 * plane - source.y,
            _ => mirrored.z = 2.0 * plane - source.z,
        }
        excess[index] = (mirrored.distance(receiver) - direct).max(0.0);
    }
    excess.sort_by(f32::total_cmp);
    let first = excess[0];
    let fourth = excess[3];
    let first_path = direct + first;
    let path_ratio = (direct / first_path.max(direct)).clamp(0.2, 1.0);
    AudioReverbPreset {
        early_reflections_gain: (preset.early_reflections_gain * path_ratio.sqrt()).clamp(0.0, 2.0),
        pre_delay_ms: (first / SPEED_OF_SOUND_MPS * 1_000.0).clamp(0.0, 250.0),
        early_reflections_spread_ms: ((fourth - first) / SPEED_OF_SOUND_MPS * 1_000.0)
            .clamp(0.0, 250.0),
        ..preset
    }
    .sanitized()
}

#[inline]
fn vec3_array_distance(value: [f32; 3], point: Vec3) -> f32 {
    Vec3::new(value[0], value[1], value[2]).distance(point)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PortalRouteMetric {
    Direct,
    Indirect,
}

fn portal_edge_gain(portal: &AudioPortal, metric: PortalRouteMetric) -> f32 {
    match metric {
        PortalRouteMetric::Direct => portal.direct_route_gain(),
        PortalRouteMetric::Indirect => portal.route_gain(),
    }
}

/// Max-product routing with deterministic path reconstruction. Because every portal edge is in
/// `[0,1]`, selecting the currently strongest unvisited route is the multiplicative equivalent of
/// Dijkstra and cannot be improved later by a cycle.
fn strongest_portal_route_map(
    zones: &[ResolvedEnvironmentZone],
    portals: &[AudioPortal],
    listener_zone_index: usize,
    metric: PortalRouteMetric,
) -> BTreeMap<String, PortalRoute> {
    let known_zone_ids = zones
        .iter()
        .map(|zone| zone.zone.zone_id.clone())
        .collect::<BTreeSet<_>>();
    let listener_id = zones
        .get(listener_zone_index)
        .map(|zone| zone.zone.zone_id.clone())
        .unwrap_or_default();
    if listener_id.is_empty() {
        return BTreeMap::new();
    }

    let mut routes = BTreeMap::<String, PortalRoute>::new();
    routes.insert(
        listener_id,
        PortalRoute {
            gain: 1.0,
            portal_ids: Vec::new(),
        },
    );
    let mut visited = BTreeSet::<String>::new();

    loop {
        let current_zone = routes
            .iter()
            .filter(|(zone_id, route)| !visited.contains(*zone_id) && route.gain > 0.0)
            .max_by(|(zone_a, route_a), (zone_b, route_b)| {
                route_a
                    .gain
                    .total_cmp(&route_b.gain)
                    // For equal gain, lexically smaller zone id wins deterministically.
                    .then_with(|| zone_b.cmp(zone_a))
            })
            .map(|(zone_id, _)| zone_id.clone());
        let Some(current_zone) = current_zone else {
            break;
        };
        let current = routes
            .get(&current_zone)
            .cloned()
            .expect("selected route exists");
        visited.insert(current_zone.clone());

        for portal in portals {
            let next_zone = if portal.zone_a == current_zone {
                &portal.zone_b
            } else if portal.zone_b == current_zone {
                &portal.zone_a
            } else {
                continue;
            };
            if !known_zone_ids.contains(next_zone) || visited.contains(next_zone) {
                continue;
            }
            let edge = portal_edge_gain(portal, metric);
            if edge <= 0.0 {
                continue;
            }
            let mut candidate = current.clone();
            candidate.gain = (candidate.gain * edge).clamp(0.0, 1.0);
            candidate.portal_ids.push(portal.portal_id.clone());
            let replace = routes.get(next_zone).is_none_or(|existing| {
                candidate.gain > existing.gain + 1.0e-7
                    || ((candidate.gain - existing.gain).abs() <= 1.0e-7
                        && candidate.portal_ids < existing.portal_ids)
            });
            if replace {
                routes.insert(next_zone.clone(), candidate);
            }
        }
    }
    routes
}

fn strongest_direct_portal_routes(
    zones: &[ResolvedEnvironmentZone],
    portals: &[AudioPortal],
    listener_zone_index: usize,
) -> BTreeMap<String, PortalRoute> {
    strongest_portal_route_map(
        zones,
        portals,
        listener_zone_index,
        PortalRouteMetric::Direct,
    )
}

#[cfg(test)]
fn strongest_portal_routes(
    zones: &[ResolvedEnvironmentZone],
    portals: &[AudioPortal],
    listener_zone_index: usize,
) -> BTreeMap<String, f32> {
    let routes = strongest_portal_route_map(
        zones,
        portals,
        listener_zone_index,
        PortalRouteMetric::Indirect,
    );
    zones
        .iter()
        .map(|zone| {
            let gain = routes
                .get(&zone.zone.zone_id)
                .map(|route| route.gain)
                .unwrap_or(0.0);
            (zone.zone.zone_id.clone(), gain)
        })
        .collect()
}

#[cfg(test)]
fn direct_portal_response(
    portal: &AudioPortal,
    center: Vec3,
    emitter_position: Vec3,
    listener_position: Vec3,
) -> AudioDirectPathResponse {
    let route = PortalRoute {
        gain: portal.direct_route_gain(),
        portal_ids: vec![portal.portal_id.clone()],
    };
    let mut portals = vec![portal.clone()];
    portals.sort_by(|a, b| a.portal_id.cmp(&b.portal_id));
    let centers = BTreeMap::from([(portal.portal_id.clone(), center)]);
    direct_portal_route_response(
        &route,
        &portals,
        &centers,
        emitter_position,
        listener_position,
    )
    .map(|(response, _, _)| response)
    .unwrap_or(AudioDirectPathResponse {
        gain: route.gain,
        ..AudioDirectPathResponse::clear()
    })
}

fn portal_route_boundary_centers(
    route: &PortalRoute,
    portal_centers: &BTreeMap<String, Vec3>,
) -> Option<(Option<Vec3>, Option<Vec3>)> {
    if route.portal_ids.is_empty() {
        return None;
    }
    // Stored listener->destination. Emitter-side boundary is therefore the final id; the
    // listener-side boundary is the first id.
    let source = portal_centers.get(route.portal_ids.last()?).copied()?;
    let listener = portal_centers.get(route.portal_ids.first()?).copied()?;
    Some((Some(source), Some(listener)))
}

/// Resolves the strongest topological direct route into actual portal waypoints. Route portal ids
/// are stored listener->destination, so they are reversed here to walk emitter->listener.
/// Every aperture contributes its own diffraction loss while extra delay is derived once from the
/// complete polyline length.
fn direct_portal_route_response(
    route: &PortalRoute,
    portals: &[AudioPortal],
    portal_centers: &BTreeMap<String, Vec3>,
    emitter_position: Vec3,
    listener_position: Vec3,
) -> Option<(AudioDirectPathResponse, Option<Vec3>, Option<Vec3>)> {
    if route.portal_ids.is_empty() || route.gain <= 0.0 {
        return None;
    }
    let mut route_portals = Vec::<AudioPortal>::with_capacity(route.portal_ids.len());
    let mut centers = Vec::<Vec3>::with_capacity(route.portal_ids.len());
    for portal_id in route.portal_ids.iter().rev() {
        let portal = portals
            .iter()
            .find(|portal| portal.portal_id == *portal_id)?;
        let center = portal_centers.get(portal_id).copied()?;
        route_portals.push(portal.clone().sanitized());
        centers.push(center);
    }

    let direct_length = emitter_position.distance(listener_position).max(1.0e-4);
    let mut routed_length = 0.0_f32;
    let mut previous = emitter_position;
    for center in &centers {
        routed_length += previous.distance(*center);
        previous = *center;
    }
    routed_length += previous.distance(listener_position);
    let total_excess = (routed_length - direct_length).max(0.0);

    let mut gain = 1.0_f32;
    let mut high_frequency_gain = 1.0_f32;
    for index in 0..route_portals.len() {
        let portal = &route_portals[index];
        let center = centers[index];
        let previous = if index == 0 {
            emitter_position
        } else {
            centers[index - 1]
        };
        let next = if index + 1 == centers.len() {
            listener_position
        } else {
            centers[index + 1]
        };
        let local_direct = previous.distance(next).max(1.0e-4);
        let local_routed = previous.distance(center) + center.distance(next);
        let local_excess = (local_routed - local_direct).max(0.0);
        let aperture_radius =
            (2.0 * portal.half_extents[0].min(portal.half_extents[1])) * portal.openness.sqrt();
        let aperture_factor = (aperture_radius / (aperture_radius + 0.20)).clamp(0.0, 1.0);
        let bend = local_excess / (aperture_radius + 0.10);
        let geometric_gain = (0.75 + 0.25 * aperture_factor) / (1.0 + 0.55 * bend);
        let edge_hf = (aperture_factor / (1.0 + 0.90 * bend)).clamp(0.02, 1.0);
        gain *= portal.direct_route_gain() * geometric_gain;
        high_frequency_gain *= edge_hf;
    }

    let source_boundary = centers.first().copied();
    let listener_boundary = centers.last().copied();
    let response = AudioDirectPathResponse {
        gain: gain.clamp(0.0, 1.0),
        high_frequency_gain: high_frequency_gain.clamp(0.001, 1.0),
        low_pass_hz: (900.0 + 19_100.0 * high_frequency_gain.sqrt()).clamp(900.0, 20_000.0),
        extra_delay_ms: (total_excess / SPEED_OF_SOUND_MPS * 1_000.0).clamp(0.0, 500.0),
    }
    .sanitized();
    Some((response, source_boundary, listener_boundary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_audio_api::AudioReverbPreset;

    fn zone(
        key: u64,
        id: &str,
        center: Vec3,
        half_extents: Vec3,
        priority: i32,
    ) -> ResolvedEnvironmentZone {
        ResolvedEnvironmentZone {
            stable_key: key,
            zone: AudioEnvironmentZone {
                zone_id: id.to_owned(),
                priority,
                half_extents: [half_extents.x, half_extents.y, half_extents.z],
                blend_distance: 0.0,
                send_gain: 0.5,
                reverb: AudioReverbPreset::room(),
                ..AudioEnvironmentZone::default()
            },
            center,
            rotation: Quat::IDENTITY,
            half_extents,
        }
    }

    #[test]
    fn overlapping_zone_selection_prefers_priority_then_center_distance() {
        let zones = vec![
            zone(1, "room.low", Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0), 0),
            zone(
                2,
                "room.high",
                Vec3::new(5.0, 0.0, 0.0),
                Vec3::new(10.0, 10.0, 10.0),
                5,
            ),
        ];
        let selected = select_membership(&zones, Vec3::ZERO).expect("membership");
        assert_eq!(zones[selected.zone_index].zone.zone_id, "room.high");

        let equal_priority = vec![
            zone(
                3,
                "room.far",
                Vec3::new(4.0, 0.0, 0.0),
                Vec3::new(10.0, 10.0, 10.0),
                5,
            ),
            zone(4, "room.near", Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0), 5),
        ];
        let selected = select_membership(&equal_priority, Vec3::ZERO).expect("membership");
        assert_eq!(
            equal_priority[selected.zone_index].zone.zone_id,
            "room.near"
        );
    }

    #[test]
    fn room_geometry_moves_first_order_early_reflections() {
        let room = zone(10, "room.geometry", Vec3::ZERO, Vec3::new(5.0, 3.0, 7.0), 0);
        let centered = geometry_adjusted_reverb(
            &room,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            AudioReverbPreset::room(),
            None,
        );
        let near_wall = geometry_adjusted_reverb(
            &room,
            Vec3::new(4.5, 0.0, 0.0),
            Vec3::new(3.5, 0.0, 0.0),
            AudioReverbPreset::room(),
            None,
        );
        assert!(near_wall.pre_delay_ms < centered.pre_delay_ms);
        assert!(centered.early_reflections_spread_ms > 0.0);
        assert!(near_wall.early_reflections_spread_ms > 0.0);
    }

    #[test]
    fn portal_detour_adds_delay_and_high_frequency_loss() {
        let portal = AudioPortal::new("door", "a", "b");
        let straight = direct_portal_response(
            &portal,
            Vec3::ZERO,
            Vec3::new(-5.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 0.0),
        );
        let detour = direct_portal_response(
            &portal,
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(-5.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 0.0),
        );
        assert!(straight.extra_delay_ms < 1.0e-4);
        assert!(detour.extra_delay_ms > 10.0);
        assert!(detour.gain < straight.gain);
        assert!(detour.high_frequency_gain < straight.high_frequency_gain);
        assert!(detour.low_pass_hz < straight.low_pass_hz);
    }

    #[test]
    fn smaller_portal_aperture_diffracts_high_frequencies_more() {
        let mut wide = AudioPortal::new("wide", "a", "b");
        wide.half_extents = [1.5, 1.5];
        let mut narrow = AudioPortal::new("narrow", "a", "b");
        narrow.half_extents = [0.12, 0.9];
        let emitter = Vec3::new(-5.0, 0.0, 0.0);
        let listener = Vec3::new(5.0, 0.0, 0.0);
        let center = Vec3::new(0.0, 0.0, 2.5);
        let wide_response = direct_portal_response(&wide, center, emitter, listener);
        let narrow_response = direct_portal_response(&narrow, center, emitter, listener);
        assert!(narrow_response.high_frequency_gain < wide_response.high_frequency_gain);
        assert!(narrow_response.low_pass_hz < wide_response.low_pass_hz);
        assert!(narrow_response.gain < wide_response.gain);
    }

    #[test]
    fn strongest_portal_path_multiplies_edges_and_prefers_better_route() {
        let zones = vec![
            zone(1, "a", Vec3::ZERO, Vec3::ONE, 0),
            zone(2, "b", Vec3::ZERO, Vec3::ONE, 0),
            zone(3, "c", Vec3::ZERO, Vec3::ONE, 0),
        ];
        let mut ab = AudioPortal::new("ab", "a", "b");
        ab.openness = 0.8;
        let mut bc = AudioPortal::new("bc", "b", "c");
        bc.openness = 0.5;
        let mut ac = AudioPortal::new("ac", "a", "c");
        ac.openness = 0.3;
        let gains = strongest_portal_routes(&zones, &[ab, bc, ac], 0);
        assert!((gains["c"] - 0.4).abs() < 1.0e-6);
    }

    #[test]
    fn multi_hop_geometric_route_accumulates_waypoint_delay_and_multi_edge_diffraction() {
        let zones = vec![
            zone(1, "a", Vec3::ZERO, Vec3::ONE, 0),
            zone(2, "b", Vec3::ZERO, Vec3::ONE, 0),
            zone(3, "c", Vec3::ZERO, Vec3::ONE, 0),
        ];
        let mut ab = AudioPortal::new("ab", "a", "b");
        ab.openness = 0.9;
        ab.half_extents = [0.15, 0.9];
        let mut bc = AudioPortal::new("bc", "b", "c");
        bc.openness = 0.9;
        bc.half_extents = [0.15, 0.9];
        let portals = vec![ab.clone(), bc.clone()];
        let routes = strongest_direct_portal_routes(&zones, &portals, 0);
        let route = routes.get("c").expect("multi-hop direct route");
        assert_eq!(route.portal_ids, vec!["ab".to_owned(), "bc".to_owned()]);
        assert!((route.gain - 0.81).abs() < 1.0e-6);

        let centers = BTreeMap::from([
            ("ab".to_owned(), Vec3::new(-2.0, 0.0, 3.0)),
            ("bc".to_owned(), Vec3::new(2.0, 0.0, -3.0)),
        ]);
        let emitter = Vec3::new(5.0, 0.0, 0.0);
        let listener = Vec3::new(-5.0, 0.0, 0.0);
        let (narrow, source_boundary, listener_boundary) =
            direct_portal_route_response(route, &portals, &centers, emitter, listener)
                .expect("geometric route");
        assert_eq!(source_boundary, Some(centers["bc"]));
        assert_eq!(listener_boundary, Some(centers["ab"]));
        assert!(narrow.extra_delay_ms > 10.0);
        assert!(narrow.gain < route.gain);
        assert!(narrow.high_frequency_gain < 1.0);

        // Widening only one of two apertures must improve the result, proving that each edge
        // contributes independently instead of collapsing the chain into one scalar portal gain.
        let mut widened_ab = ab;
        widened_ab.half_extents = [1.5, 1.5];
        let partly_wide_portals = vec![widened_ab, bc];
        let (partly_wide, _, _) =
            direct_portal_route_response(route, &partly_wide_portals, &centers, emitter, listener)
                .expect("partly wide geometric route");
        assert!((partly_wide.extra_delay_ms - narrow.extra_delay_ms).abs() < 1.0e-4);
        assert!(partly_wide.high_frequency_gain > narrow.high_frequency_gain);
        assert!(partly_wide.gain > narrow.gain);
    }

    #[test]
    fn direct_and_indirect_graphs_can_choose_different_portal_topology() {
        let zones = vec![
            zone(1, "a", Vec3::ZERO, Vec3::ONE, 0),
            zone(2, "b", Vec3::ZERO, Vec3::ONE, 0),
            zone(3, "c", Vec3::ZERO, Vec3::ONE, 0),
        ];
        let mut ab = AudioPortal::new("ab", "a", "b");
        ab.openness = 0.95;
        ab.send_gain = 0.10;
        let mut bc = AudioPortal::new("bc", "b", "c");
        bc.openness = 0.95;
        bc.send_gain = 0.10;
        let mut ac = AudioPortal::new("ac", "a", "c");
        ac.openness = 0.70;
        ac.send_gain = 1.0;
        let portals = vec![ab, bc, ac];

        let direct = strongest_portal_route_map(&zones, &portals, 0, PortalRouteMetric::Direct);
        let indirect = strongest_portal_route_map(&zones, &portals, 0, PortalRouteMetric::Indirect);
        assert_eq!(
            direct["c"].portal_ids,
            vec!["ab".to_owned(), "bc".to_owned()]
        );
        assert_eq!(indirect["c"].portal_ids, vec!["ac".to_owned()]);
        assert!(direct["c"].gain > 0.90);
        assert!((indirect["c"].gain - 0.70).abs() < 1.0e-6);
    }

    #[test]
    fn reflection_visibility_and_material_absorption_shape_early_field() {
        let room = ResolvedEnvironmentZone {
            stable_key: 1,
            zone: AudioEnvironmentZone {
                zone_id: "room.test".to_owned(),
                kind: AudioEnvironmentKind::Indoor,
                half_extents: [5.0, 5.0, 5.0],
                blend_distance: 0.0,
                ..AudioEnvironmentZone::default()
            },
            center: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            half_extents: Vec3::splat(5.0),
        };
        let source = Vec3::ZERO;
        let listener = Vec3::new(1.0, 0.0, 0.0);
        let geometry = newengine_audio_world_api::first_order_reflection_geometry(
            newengine_audio_world_api::AudioRoomObbGeometry {
                center: [0.0; 3],
                rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
                half_extents: [5.0; 3],
            },
            [0.0; 3],
            [1.0, 0.0, 0.0],
        );
        let reflective = newengine_audio_api::AcousticMaterialProfile {
            transmission_gain: 0.2,
            reflection_gain: 0.8,
            high_frequency_absorption: 0.15,
            low_pass_hz: 5_000.0,
        };
        let absorptive = newengine_audio_api::AcousticMaterialProfile {
            transmission_gain: 0.2,
            reflection_gain: 0.35,
            high_frequency_absorption: 0.90,
            low_pass_hz: 2_000.0,
        };
        let make_observation =
            |blocked_face: Option<u8>, material| AudioEarlyReflectionObservation {
                fixed_tick: 10,
                source_position: [0.0; 3],
                listener_position: [1.0, 0.0, 0.0],
                paths: geometry
                    .iter()
                    .map(
                        |path| newengine_audio_world_api::AudioEarlyReflectionPathObservation {
                            face_index: path.face_index,
                            visible: blocked_face != Some(path.face_index),
                            boundary_entity: Some(42),
                            reflection_point: path.reflection_point,
                            arrival_direction: path.arrival_direction,
                            path_length_m: path.path_length_m,
                            excess_length_m: path.excess_length_m,
                            material_known: true,
                            material,
                        },
                    )
                    .collect(),
                second_order_paths: Vec::new(),
            };
        let open = make_observation(None, reflective);
        let blocked = make_observation(Some(geometry[0].face_index), reflective);
        let soft = make_observation(None, absorptive);
        let open_preset = geometry_adjusted_reverb(
            &room,
            source,
            listener,
            AudioReverbPreset::room(),
            Some(&open),
        );
        let blocked_preset = geometry_adjusted_reverb(
            &room,
            source,
            listener,
            AudioReverbPreset::room(),
            Some(&blocked),
        );
        let soft_preset = geometry_adjusted_reverb(
            &room,
            source,
            listener,
            AudioReverbPreset::room(),
            Some(&soft),
        );
        assert!(blocked_preset.early_reflections_gain < open_preset.early_reflections_gain);
        assert!(soft_preset.early_reflections_gain < open_preset.early_reflections_gain);
        assert!(
            soft_preset.early_reflections_high_frequency_gain
                < open_preset.early_reflections_high_frequency_gain
        );
    }

    #[test]
    fn second_order_reflection_becomes_discrete_later_tap_with_multiplicative_material_loss() {
        let first_material = newengine_audio_api::AcousticMaterialProfile {
            transmission_gain: 1.0,
            reflection_gain: 0.8,
            high_frequency_absorption: 0.10,
            low_pass_hz: 12_000.0,
        };
        let second_a = newengine_audio_api::AcousticMaterialProfile {
            transmission_gain: 1.0,
            reflection_gain: 0.55,
            high_frequency_absorption: 0.35,
            low_pass_hz: 8_000.0,
        };
        let second_b = newengine_audio_api::AcousticMaterialProfile {
            transmission_gain: 1.0,
            reflection_gain: 0.45,
            high_frequency_absorption: 0.50,
            low_pass_hz: 6_000.0,
        };
        let observation = AudioEarlyReflectionObservation {
            fixed_tick: 12,
            source_position: [0.0, 0.0, 0.0],
            listener_position: [1.0, 0.0, 0.0],
            paths: vec![
                newengine_audio_world_api::AudioEarlyReflectionPathObservation {
                    face_index: 1,
                    visible: true,
                    boundary_entity: Some(10),
                    reflection_point: [2.0, 0.0, 0.0],
                    arrival_direction: [1.0, 0.0, 0.0],
                    path_length_m: 2.0,
                    excess_length_m: 1.0,
                    material_known: true,
                    material: first_material,
                },
            ],
            second_order_paths: vec![
                newengine_audio_world_api::AudioSecondOrderReflectionPathObservation {
                    face_indices: [1, 3],
                    visible: true,
                    boundary_entities: [Some(11), Some(12)],
                    reflection_points: [[2.0, 0.0, 0.0], [2.0, 2.0, 0.0]],
                    arrival_direction: [0.0, 1.0, 0.0],
                    path_length_m: 3.0,
                    excess_length_m: 2.0,
                    material_known: [true, true],
                    materials: [second_a, second_b],
                },
            ],
        };
        let field = explicit_early_reflection_field(
            AudioReverbPreset::room(),
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Some(&observation),
        );
        assert_eq!(field.count, 2);
        let first = field.active().iter().find(|tap| tap.order == 1).unwrap();
        let second = field.active().iter().find(|tap| tap.order == 2).unwrap();
        assert!(second.delay_ms > first.delay_ms);
        assert!(second.gain < first.gain);
        let expected_hf = second_a.high_frequency_gain() * second_b.high_frequency_gain();
        assert!((second.high_frequency_gain - expected_hf).abs() < 1.0e-6);
        assert_eq!(second.direction, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn stale_reflection_observation_falls_back_to_current_room_geometry() {
        let room = ResolvedEnvironmentZone {
            stable_key: 1,
            zone: AudioEnvironmentZone::new("room.test", [5.0, 5.0, 5.0]),
            center: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            half_extents: Vec3::splat(5.0),
        };
        let stale = AudioEarlyReflectionObservation {
            fixed_tick: 1,
            source_position: [100.0, 0.0, 0.0],
            listener_position: [101.0, 0.0, 0.0],
            paths: vec![
                newengine_audio_world_api::AudioEarlyReflectionPathObservation {
                    visible: false,
                    ..Default::default()
                },
            ],
            second_order_paths: Vec::new(),
        };
        let current = geometry_adjusted_reverb(
            &room,
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            AudioReverbPreset::room(),
            Some(&stale),
        );
        assert!(current.early_reflections_gain > 0.0);
    }
}
