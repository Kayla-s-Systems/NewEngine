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
        self.indirect_routes
            .get(zone_id)
            .map(|route| route.gain)
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

include!("audio_environment/resolve.rs");
include!("audio_environment/acoustics.rs");
include!("audio_environment/portal_routes.rs");

#[cfg(test)]
#[path = "audio_environment/tests.rs"]
mod tests;
