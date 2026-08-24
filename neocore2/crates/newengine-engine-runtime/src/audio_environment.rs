#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};

use newengine_audio_api::{
    AudioEnvironmentKind, AudioEnvironmentState, AudioEnvironmentZone, AudioPortal, AudioReverbSend,
};
use newengine_ecs::{EntityId, World};
use newengine_math::{Quat, Vec3};
use newengine_transform::{read_entity_world_pose_local_chain, GlobalTransform, Transform};

use crate::audio_occlusion::AudioListenerRuntimeState;

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
            frame.listener_ready = false;
        }
        frame
    }

    pub fn snapshot_at(world: &World, listener_position: Vec3) -> Self {
        let mut zones = Vec::new();
        let mut portals = Vec::new();

        for entity in world.iter_entities() {
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
                let portal = portal.sanitized();
                if portal.enabled && portal.route_gain() > 0.0 {
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
        let route_gains = listener_membership
            .as_ref()
            .map(|membership| strongest_portal_routes(&zones, &portals, membership.zone_index))
            .unwrap_or_default();

        Self {
            zones,
            portals,
            listener_membership,
            route_gains,
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

    pub fn resolve_at(&self, emitter_position: Vec3) -> AudioEnvironmentResolution {
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
                resolution.state = AudioEnvironmentState {
                    source_send: AudioReverbSend::default(),
                    listener_send: AudioReverbSend {
                        gain: listener.zone.send_gain
                            * emitter_membership.influence
                            * listener_membership.influence,
                        preset: listener.zone.reverb,
                    },
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
                resolution.state = AudioEnvironmentState {
                    source_send: AudioReverbSend {
                        gain: emitter.zone.send_gain * emitter_membership.influence * route_gain,
                        preset: emitter.zone.reverb,
                    },
                    listener_send: AudioReverbSend {
                        gain: listener.zone.send_gain * listener_membership.influence * route_gain,
                        preset: listener.zone.reverb,
                    },
                    portal_gain: route_gain,
                }
                .sanitized();
            }
            _ => {
                resolution.state = AudioEnvironmentState::clear();
                resolution.portal_gain = 0.0;
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
        let zone = &self.zones[membership.zone_index].zone;
        AudioEnvironmentState {
            source_send: AudioReverbSend::default(),
            listener_send: AudioReverbSend {
                gain: zone.send_gain * membership.influence,
                preset: zone.reverb,
            },
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

fn strongest_portal_routes(
    zones: &[ResolvedEnvironmentZone],
    portals: &[AudioPortal],
    listener_zone_index: usize,
) -> BTreeMap<String, f32> {
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

    let mut gains = known_zone_ids
        .iter()
        .map(|zone_id| (zone_id.clone(), 0.0_f32))
        .collect::<BTreeMap<_, _>>();
    gains.insert(listener_id, 1.0);

    let iterations = known_zone_ids.len().max(1);
    for _ in 0..iterations {
        let mut changed = false;
        for portal in portals {
            if !known_zone_ids.contains(&portal.zone_a) || !known_zone_ids.contains(&portal.zone_b)
            {
                continue;
            }
            let edge = portal.route_gain();
            if edge <= 0.0 {
                continue;
            }
            let a = gains.get(&portal.zone_a).copied().unwrap_or(0.0);
            let b = gains.get(&portal.zone_b).copied().unwrap_or(0.0);
            let next_b = (a * edge).max(b);
            let next_a = (b * edge).max(a);
            if next_b > b + 1.0e-7 {
                gains.insert(portal.zone_b.clone(), next_b);
                changed = true;
            }
            if next_a > a + 1.0e-7 {
                gains.insert(portal.zone_a.clone(), next_a);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    gains
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
}
