#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime world authority bridge.
//!
//! This module is the host-side guardrail for the plugin replaceability model:
//! systems may still use the in-process `newengine_ecs::World` for typed hot-path
//! component storage, but the authoritative service identity is discovered from
//! `engine.ecs` and `engine.entity` gateway routes. The bridge publishes that
//! route decision into the world every frame so scene, physics sync, render
//! extraction and gameplay can see whether they are running against an engine
//! owned baseline or a plugin-owned ECS/entity authority.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::RwLock;

use newengine_ecs::{EntityId, World};
use newengine_ecs_api::{EcsCommand, EcsCommandRequest};
use newengine_entity_api::{EntityHandle, EntitySpawnRequest};
use newengine_runtime_host::world_authority::{WorldAuthorityClient, WorldAuthoritySnapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeWorldAuthorityMode {
    /// The selected ECS/entity gateways are both engine-runtime or missing; the
    /// in-process world is the baseline authority.
    EngineRuntimeWorld,
    /// `engine.ecs` and `engine.entity` are both plugin-owned by the same
    /// provider. The in-process world is a typed cache/hot-path staging surface.
    PluginEcsEntityAuthority,
    /// ECS/entity or scene/ecs owners disagree. This is allowed during a staged
    /// migration, but must be visible in diagnostics.
    SplitAuthority,
}

impl RuntimeWorldAuthorityMode {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EngineRuntimeWorld => "engine-runtime-world",
            Self::PluginEcsEntityAuthority => "plugin-ecs-entity-authority",
            Self::SplitAuthority => "split-authority",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeWorldAuthorityFrame {
    pub frame_index: u64,
    pub phase: &'static str,
    pub mode: RuntimeWorldAuthorityMode,
    pub route_snapshot: WorldAuthoritySnapshot,
    pub native_world_tick: u64,
    pub native_entity_count: u64,
    pub native_storage_count: u64,
    pub native_resource_count: u64,
}

impl RuntimeWorldAuthorityFrame {
    #[inline]
    pub fn native_world_is_component_cache(&self) -> bool {
        matches!(
            self.mode,
            RuntimeWorldAuthorityMode::PluginEcsEntityAuthority
                | RuntimeWorldAuthorityMode::SplitAuthority
        )
    }
}

/// Frame-local ECS resource. Systems can read this instead of re-querying the
/// plugin host or assuming the native `World` is the source of truth.
#[derive(Clone, Debug)]
pub struct RuntimeWorldAuthorityResource {
    pub frame: RuntimeWorldAuthorityFrame,
}

/// Native-world-to-provider handle projection for staged hot-path caches.
///
/// When a plugin owns `engine.ecs`/`engine.entity`, native `EntityId` values are
/// not source-of-truth ids. This resource records the explicit projection created
/// by scene bootstrap so render/physics/gameplay can name the provider handle
/// without treating the native key as authoritative.
#[derive(Clone, Debug, Default)]
pub struct RuntimeEntityAuthorityMap {
    pub phase: &'static str,
    pub authority: String,
    pub native_to_provider: BTreeMap<u64, EntityHandle>,
    pub selected_provider: Option<EntityHandle>,
}

impl RuntimeEntityAuthorityMap {
    #[inline]
    pub fn provider_for_native(&self, native: EntityId) -> Option<EntityHandle> {
        self.native_to_provider.get(&native.stable_u64()).copied()
    }
}

pub struct RuntimeWorldAuthorityBridge {
    client: WorldAuthorityClient,
    last_snapshot: RwLock<WorldAuthoritySnapshot>,
    logged_bootstrap: AtomicBool,
    logged_split: AtomicBool,
    logged_plugin_authority: AtomicBool,
}

impl Default for RuntimeWorldAuthorityBridge {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeWorldAuthorityBridge {
    #[inline]
    pub fn new() -> Self {
        Self {
            client: WorldAuthorityClient::default(),
            last_snapshot: RwLock::new(WorldAuthoritySnapshot::default()),
            logged_bootstrap: AtomicBool::new(false),
            logged_split: AtomicBool::new(false),
            logged_plugin_authority: AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn client(&self) -> &WorldAuthorityClient {
        &self.client
    }

    pub fn detect(&self) -> WorldAuthoritySnapshot {
        let snapshot = self.client.snapshot_routes();
        *self.last_snapshot.write() = snapshot.clone();
        snapshot
    }

    #[inline]
    pub fn last_snapshot(&self) -> WorldAuthoritySnapshot {
        self.last_snapshot.read().clone()
    }

    pub fn classify(snapshot: &WorldAuthoritySnapshot) -> RuntimeWorldAuthorityMode {
        if snapshot.has_split_world_authority() {
            RuntimeWorldAuthorityMode::SplitAuthority
        } else if snapshot.ecs_entity_are_plugin_authority() {
            RuntimeWorldAuthorityMode::PluginEcsEntityAuthority
        } else {
            RuntimeWorldAuthorityMode::EngineRuntimeWorld
        }
    }

    pub fn publish_frame(
        &self,
        world: &mut World,
        frame_index: u64,
        phase: &'static str,
    ) -> RuntimeWorldAuthorityFrame {
        let snapshot = self.detect();
        let mode = Self::classify(&snapshot);
        let frame = RuntimeWorldAuthorityFrame {
            frame_index,
            phase,
            mode,
            route_snapshot: snapshot.clone(),
            native_world_tick: world.tick(),
            native_entity_count: world.entity_count() as u64,
            native_storage_count: world.storage_count() as u64,
            native_resource_count: world.resource_count() as u64,
        };
        world.insert_resource(RuntimeWorldAuthorityResource {
            frame: frame.clone(),
        });
        self.log_frame_boundary(&frame);
        frame
    }

    pub fn log_bootstrap_boundary(&self, phase: &'static str) {
        if !self.logged_bootstrap.swap(true, Ordering::Relaxed) {
            let snapshot = self.detect();
            let mode = Self::classify(&snapshot);
            newengine_ulog_api::ulog::info!(
                "world authority: bootstrap phase='{}' mode='{}' authority='{}' notes='{}'",
                phase,
                mode.as_str(),
                snapshot.authority_label(),
                snapshot.notes.join(";")
            );
        }
    }

    /// Declare the freshly assembled native scene cache to the selected provider
    /// authority and publish an explicit native->provider handle map.
    ///
    /// This is a visible authority boundary
    /// declaration. Scene/bootstrap code may keep native typed storages for hot
    /// paths, but identity and semantic component lifecycle are pushed through
    /// `engine.entity` / `engine.ecs` provider packets.
    pub fn declare_native_scene_cache(
        &self,
        world: &mut World,
        phase: &'static str,
        selected_native: Option<EntityId>,
    ) -> Option<EntityHandle> {
        let snapshot = self.detect();
        if !snapshot.ecs_entity_are_plugin_authority() {
            return None;
        }

        let mut native_ids: Vec<EntityId> = world.iter_entities().collect();
        native_ids.sort_unstable_by_key(|id| id.stable_u64());
        let count = native_ids.len();
        if count == 0 {
            world.insert_resource(RuntimeEntityAuthorityMap {
                phase,
                authority: snapshot.authority_label(),
                native_to_provider: BTreeMap::new(),
                selected_provider: None,
            });
            return None;
        }

        let spawn = match self.client.spawn_entities(EntitySpawnRequest {
            count,
            ..EntitySpawnRequest::default()
        }) {
            Ok(v) => v,
            Err(e) => {
                newengine_ulog_api::ulog::warn!(
                    "world authority: provider scene declaration failed phase='{}' authority='{}' native_entities={} err='{}'",
                    phase,
                    snapshot.authority_label(),
                    count,
                    e
                );
                return None;
            }
        };

        let mut native_to_provider = BTreeMap::new();
        let mut selected_provider = None;
        let mut commands = Vec::with_capacity(spawn.entities.len());
        for (native, record) in native_ids.into_iter().zip(spawn.entities.iter()) {
            let native_stable = native.stable_u64();
            let handle = record.handle;
            let selected = selected_native.map(|it| it == native).unwrap_or(false);
            if selected {
                selected_provider = Some(handle);
            }
            native_to_provider.insert(native_stable, handle);
            commands.push(EcsCommand::SetComponentJson {
                entity_id: handle.stable_id,
                component_type: "newengine.native_entity_ref".to_owned(),
                payload: serde_json::json!({
                    "native_entity_id": native_stable,
                    "phase": phase,
                    "selected": selected,
                    "role": if selected { "selected-player" } else { "scene-cache-entity" },
                    "staging": "native-world-cache"
                }),
            });
        }

        if !commands.is_empty() {
            if let Err(e) = self.client.command(EcsCommandRequest { commands }) {
                newengine_ulog_api::ulog::warn!(
                    "world authority: provider semantic component declaration failed phase='{}' authority='{}' err='{}'",
                    phase,
                    snapshot.authority_label(),
                    e
                );
            }
        }

        world.insert_resource(RuntimeEntityAuthorityMap {
            phase,
            authority: snapshot.authority_label(),
            native_to_provider,
            selected_provider,
        });

        newengine_ulog_api::ulog::info!(
            "world authority: native scene cache declared phase='{}' authority='{}' native_entities={} provider_entities={} selected_provider={:?}",
            phase,
            snapshot.authority_label(),
            count,
            spawn.entities.len(),
            selected_provider
        );

        selected_provider
    }

    fn log_frame_boundary(&self, frame: &RuntimeWorldAuthorityFrame) {
        match frame.mode {
            RuntimeWorldAuthorityMode::EngineRuntimeWorld => {}
            RuntimeWorldAuthorityMode::PluginEcsEntityAuthority => {
                if !self.logged_plugin_authority.swap(true, Ordering::Relaxed) {
                    newengine_ulog_api::ulog::info!(
                        "world authority: plugin ECS/entity authority active owner='{}' phase='{}' native_world='component-cache' native_entities={} native_storages={}",
                        frame.route_snapshot.authority_label(),
                        frame.phase,
                        frame.native_entity_count,
                        frame.native_storage_count
                    );
                }
            }
            RuntimeWorldAuthorityMode::SplitAuthority => {
                if !self.logged_split.swap(true, Ordering::Relaxed) {
                    newengine_ulog_api::ulog::warn!(
                        "world authority: split authority detected phase='{}' authority='{}' native_world_entities={} notes='{}'",
                        frame.phase,
                        frame.route_snapshot.authority_label(),
                        frame.native_entity_count,
                        frame.route_snapshot.notes.join(";")
                    );
                }
            }
        }
    }
}

#[inline]
pub fn current_world_authority_frame(world: &World) -> Option<&RuntimeWorldAuthorityFrame> {
    world
        .resource::<RuntimeWorldAuthorityResource>()
        .map(|resource| &resource.frame)
}

#[inline]
pub fn current_entity_authority_map(world: &World) -> Option<&RuntimeEntityAuthorityMap> {
    world.resource::<RuntimeEntityAuthorityMap>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_runtime_host::world_authority::WorldAuthorityGatewayRoute;

    fn route(gateway: &str, owner: &str, origin: &str) -> WorldAuthorityGatewayRoute {
        WorldAuthorityGatewayRoute {
            gateway_id: gateway.to_owned(),
            service_kind: gateway.trim_start_matches("engine.").to_owned(),
            provider_service_id: format!("{}.api", gateway.trim_start_matches("engine.")),
            provider_owner_id: owner.to_owned(),
            backend_capability_id: format!("{}.backend", gateway.trim_start_matches("engine.")),
            backend_priority: 100,
            origin: origin.to_owned(),
            override_mode: "open".to_owned(),
            active_score: 20_100,
        }
    }

    #[test]
    fn plugin_ecs_and_entity_same_owner_is_plugin_mode() {
        let snapshot = WorldAuthoritySnapshot {
            ecs: Some(route(
                "engine.ecs",
                "newengine.ecs.flecs",
                "first-party-plugin",
            )),
            entity: Some(route(
                "engine.entity",
                "newengine.ecs.flecs",
                "first-party-plugin",
            )),
            scene: Some(route(
                "engine.scene",
                "newengine.ecs.flecs",
                "first-party-plugin",
            )),
            ..Default::default()
        };
        assert_eq!(
            RuntimeWorldAuthorityBridge::classify(&snapshot),
            RuntimeWorldAuthorityMode::PluginEcsEntityAuthority
        );
    }

    #[test]
    fn plugin_ecs_with_engine_scene_is_split_authority() {
        let snapshot = WorldAuthoritySnapshot {
            ecs: Some(route(
                "engine.ecs",
                "newengine.ecs.flecs",
                "first-party-plugin",
            )),
            entity: Some(route(
                "engine.entity",
                "newengine.ecs.flecs",
                "first-party-plugin",
            )),
            scene: Some(route(
                "engine.scene",
                "newengine-scene-runtime.scene-gateway",
                "engine-runtime",
            )),
            ..Default::default()
        };
        assert_eq!(
            RuntimeWorldAuthorityBridge::classify(&snapshot),
            RuntimeWorldAuthorityMode::SplitAuthority
        );
    }
}
