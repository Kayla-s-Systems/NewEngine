use newengine_ecs_api::{EcsCommandRequest, EcsCommandResponse, EcsSnapshotRequest, EcsWorldSnapshot, EcsWorldSummary};
use newengine_entity_api::{EntityDespawnRequest, EntityDespawnResponse, EntityListRequest, EntityListResponse, EntitySpawnRequest, EntitySpawnResponse};
use newengine_plugin_host::EngineGatewayRouteSnapshot;

use crate::{ecs_runtime::EcsServiceClient, entity_runtime::EntityServiceClient};

pub const WORLD_AUTHORITY_GATEWAY_ECS: &str = newengine_ecs_api::ENGINE_ECS_SERVICE_ID;
pub const WORLD_AUTHORITY_GATEWAY_ENTITY: &str = newengine_entity_api::ENGINE_ENTITY_SERVICE_ID;
pub const WORLD_AUTHORITY_GATEWAY_SCENE: &str = newengine_scene_io::ENGINE_SCENE_SERVICE_ID;
pub const WORLD_AUTHORITY_GATEWAY_PHYSICS: &str = newengine_physics_api::ENGINE_PHYSICS_SERVICE_ID;
pub const WORLD_AUTHORITY_GATEWAY_RENDER: &str = newengine_render_api::ENGINE_RENDER_SERVICE_ID;

/// Active provider route for one world-authority-relevant gateway.
///
/// This is deliberately DTO-only. Runtime layers use it for diagnostics and
/// policy decisions without importing provider implementation crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldAuthorityGatewayRoute {
    pub gateway_id: String,
    pub service_kind: String,
    pub provider_service_id: String,
    pub provider_owner_id: String,
    pub backend_capability_id: String,
    pub backend_priority: i32,
    pub origin: String,
    pub override_mode: String,
    pub active_score: i64,
}

impl WorldAuthorityGatewayRoute {
    #[inline]
    pub fn from_snapshot(snapshot: EngineGatewayRouteSnapshot) -> Self {
        Self {
            gateway_id: snapshot.gateway_id,
            service_kind: snapshot.service_kind,
            provider_service_id: snapshot.provider_service_id,
            provider_owner_id: snapshot.provider_owner_id,
            backend_capability_id: snapshot.backend_capability_id,
            backend_priority: snapshot.backend_priority,
            origin: snapshot.origin,
            override_mode: snapshot.override_mode,
            active_score: snapshot.active_score,
        }
    }

    #[inline]
    pub fn is_engine_runtime(&self) -> bool {
        self.origin == "engine-runtime"
    }

    #[inline]
    pub fn is_external_provider(&self) -> bool {
        !self.is_engine_runtime()
    }
}

/// Snapshot of the world authority topology currently selected by the plugin
/// host. It is the single diagnostic object that scene/entity/physics/render
/// and gameplay can look at when deciding whether a concrete `World` is an
/// implementation detail or the active source of truth.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorldAuthoritySnapshot {
    pub ecs: Option<WorldAuthorityGatewayRoute>,
    pub entity: Option<WorldAuthorityGatewayRoute>,
    pub scene: Option<WorldAuthorityGatewayRoute>,
    pub physics: Option<WorldAuthorityGatewayRoute>,
    pub render: Option<WorldAuthorityGatewayRoute>,
    pub notes: Vec<String>,
}

impl WorldAuthoritySnapshot {
    #[inline]
    pub fn ecs_entity_share_owner(&self) -> bool {
        match (&self.ecs, &self.entity) {
            (Some(ecs), Some(entity)) => ecs.provider_owner_id == entity.provider_owner_id,
            _ => false,
        }
    }

    #[inline]
    pub fn ecs_entity_are_plugin_authority(&self) -> bool {
        match (&self.ecs, &self.entity) {
            (Some(ecs), Some(entity)) => {
                ecs.is_external_provider() && entity.is_external_provider() && ecs.provider_owner_id == entity.provider_owner_id
            }
            _ => false,
        }
    }

    #[inline]
    pub fn scene_is_same_authority_owner(&self) -> bool {
        match (&self.scene, &self.ecs) {
            (Some(scene), Some(ecs)) => scene.provider_owner_id == ecs.provider_owner_id,
            _ => false,
        }
    }

    #[inline]
    pub fn has_split_world_authority(&self) -> bool {
        self.ecs_entity_are_plugin_authority()
            && self.scene.as_ref().map(|route| route.provider_owner_id.as_str())
                != self.ecs.as_ref().map(|route| route.provider_owner_id.as_str())
    }

    #[inline]
    pub fn authority_label(&self) -> String {
        match (&self.ecs, &self.entity) {
            (Some(ecs), Some(entity)) if ecs.provider_owner_id == entity.provider_owner_id => {
                format!("{} via {}/{}", ecs.provider_owner_id, ecs.provider_service_id, entity.provider_service_id)
            }
            (Some(ecs), Some(entity)) => {
                format!("split ecs={} entity={}", ecs.provider_owner_id, entity.provider_owner_id)
            }
            (Some(ecs), None) => format!("ecs={} entity=<missing>", ecs.provider_owner_id),
            (None, Some(entity)) => format!("ecs=<missing> entity={}", entity.provider_owner_id),
            (None, None) => "<missing>".to_owned(),
        }
    }
}

/// Gateway-backed authority client for world/entity diagnostics and coarse
/// world commands. This is not an ECS implementation; it is a host-side adapter
/// over the selected `engine.ecs` + `engine.entity` providers.
#[derive(Clone)]
pub struct WorldAuthorityClient {
    ecs: EcsServiceClient,
    entity: EntityServiceClient,
}

impl Default for WorldAuthorityClient {
    #[inline]
    fn default() -> Self {
        Self::new(newengine_plugin_host::default_host_api())
    }
}

impl WorldAuthorityClient {
    #[inline]
    pub fn new(host: newengine_plugin_api::HostApiV1) -> Self {
        Self {
            ecs: EcsServiceClient::new(host.clone()),
            entity: EntityServiceClient::new(host),
        }
    }

    #[inline]
    pub fn ecs(&self) -> &EcsServiceClient {
        &self.ecs
    }

    #[inline]
    pub fn entity(&self) -> &EntityServiceClient {
        &self.entity
    }

    pub fn snapshot_routes(&self) -> WorldAuthoritySnapshot {
        let mut out = WorldAuthoritySnapshot {
            ecs: active_route(WORLD_AUTHORITY_GATEWAY_ECS),
            entity: active_route(WORLD_AUTHORITY_GATEWAY_ENTITY),
            scene: active_route(WORLD_AUTHORITY_GATEWAY_SCENE),
            physics: active_route(WORLD_AUTHORITY_GATEWAY_PHYSICS),
            render: active_route(WORLD_AUTHORITY_GATEWAY_RENDER),
            notes: Vec::new(),
        };

        if out.ecs.is_none() {
            out.notes.push("engine.ecs has no active route".to_owned());
        }
        if out.entity.is_none() {
            out.notes.push("engine.entity has no active route".to_owned());
        }
        if let (Some(ecs), Some(entity)) = (&out.ecs, &out.entity) {
            if ecs.provider_owner_id != entity.provider_owner_id {
                out.notes.push(format!(
                    "split ecs/entity authority: ecs_owner={} entity_owner={}",
                    ecs.provider_owner_id, entity.provider_owner_id
                ));
            }
        }
        if out.has_split_world_authority() {
            let scene_owner = out
                .scene
                .as_ref()
                .map(|route| route.provider_owner_id.as_str())
                .unwrap_or("<missing>");
            let ecs_owner = out
                .ecs
                .as_ref()
                .map(|route| route.provider_owner_id.as_str())
                .unwrap_or("<missing>");
            out.notes.push(format!(
                "scene gateway owner differs from ECS authority: scene_owner={} ecs_owner={}",
                scene_owner, ecs_owner
            ));
        }
        out
    }

    #[inline]
    pub fn summary(&self) -> Result<EcsWorldSummary, String> {
        self.ecs.summary()
    }

    #[inline]
    pub fn snapshot(&self, req: EcsSnapshotRequest) -> Result<EcsWorldSnapshot, String> {
        self.ecs.snapshot(req)
    }

    #[inline]
    pub fn command(&self, req: EcsCommandRequest) -> Result<EcsCommandResponse, String> {
        self.ecs.command(req)
    }

    #[inline]
    pub fn list_entities(&self, req: EntityListRequest) -> Result<EntityListResponse, String> {
        self.entity.list(req)
    }

    #[inline]
    pub fn spawn_entities(&self, req: EntitySpawnRequest) -> Result<EntitySpawnResponse, String> {
        self.entity.spawn(req)
    }

    #[inline]
    pub fn despawn_entities(&self, req: EntityDespawnRequest) -> Result<EntityDespawnResponse, String> {
        self.entity.despawn(req)
    }
}

fn active_route(gateway_id: &str) -> Option<WorldAuthorityGatewayRoute> {
    newengine_plugin_host::active_engine_gateway_route(gateway_id)
        .map(WorldAuthorityGatewayRoute::from_snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn ecs_entity_same_plugin_is_plugin_authority() {
        let snap = WorldAuthoritySnapshot {
            ecs: Some(route("engine.ecs", "newengine.ecs.flecs", "first-party-plugin")),
            entity: Some(route("engine.entity", "newengine.ecs.flecs", "first-party-plugin")),
            ..Default::default()
        };
        assert!(snap.ecs_entity_are_plugin_authority());
        assert!(snap.ecs_entity_share_owner());
    }

    #[test]
    fn scene_owned_by_engine_while_ecs_plugin_is_split_authority() {
        let snap = WorldAuthoritySnapshot {
            ecs: Some(route("engine.ecs", "newengine.ecs.flecs", "first-party-plugin")),
            entity: Some(route("engine.entity", "newengine.ecs.flecs", "first-party-plugin")),
            scene: Some(route("engine.scene", "newengine-scene-runtime.scene-gateway", "engine-runtime")),
            ..Default::default()
        };
        assert!(snap.has_split_world_authority());
        assert!(!snap.scene_is_same_authority_owner());
    }

    #[test]
    fn scene_owned_by_same_plugin_removes_split_authority() {
        let snap = WorldAuthoritySnapshot {
            ecs: Some(route("engine.ecs", "newengine.ecs.flecs", "first-party-plugin")),
            entity: Some(route("engine.entity", "newengine.ecs.flecs", "first-party-plugin")),
            scene: Some(route("engine.scene", "newengine.ecs.flecs", "first-party-plugin")),
            ..Default::default()
        };
        assert!(!snap.has_split_world_authority());
        assert!(snap.scene_is_same_authority_owner());
    }
}
