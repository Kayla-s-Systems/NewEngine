use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::ok_json;
use newengine_world_api::{
    WorldActiveCellsRequest, WorldActiveCellsResponse, WorldBootPhase, WorldBootRequest,
    WorldBootResponse, WorldCellResidency, WorldPartitionResponse, WorldRuntimeState,
    WorldServiceInfo, WorldStateRequest, WorldStateResponse,
};

use crate::{payload::decode_blob, service::EngineWorldGatewayService};

impl EngineWorldGatewayService {
    pub(crate) fn authority_json(&self) -> serde_json::Value {
        let snapshot = self.scene.authority_snapshot();
        let route_json = |route: &newengine_world_authority_runtime::WorldAuthorityGatewayRoute| {
            serde_json::json!({
                "gateway": route.gateway_id,
                "kind": route.service_kind,
                "provider_service": route.provider_service_id,
                "provider_owner": route.provider_owner_id,
                "capability": route.backend_capability_id,
                "origin": route.origin,
                "priority": route.backend_priority,
                "score": route.active_score,
            })
        };
        serde_json::json!({
            "authority": snapshot.authority_label(),
            "split": snapshot.has_split_world_authority(),
            "ecs": snapshot.ecs.as_ref().map(route_json),
            "entity": snapshot.entity.as_ref().map(route_json),
            "scene": snapshot.scene.as_ref().map(route_json),
            "world": snapshot.world.as_ref().map(route_json),
            "physics": snapshot.physics.as_ref().map(route_json),
            "render": snapshot.render.as_ref().map(route_json),
            "notes": snapshot.notes,
        })
    }

    pub(crate) fn runtime_state(&self, include_cells: bool) -> WorldRuntimeState {
        let (
            world_instance_id,
            phase,
            deterministic,
            boot_sequence,
            partition,
            active_cells,
            notes,
        ) = {
            let state = self.state.lock();
            (
                state.world_instance_id.clone(),
                state.phase,
                state.deterministic,
                state.boot_sequence,
                state.partition.clone(),
                if include_cells {
                    state.active_cells.clone()
                } else {
                    Vec::new()
                },
                state.notes.clone(),
            )
        };

        let (tick, entity_count) = {
            let scene_lock = self.scene.scene();
            let scene = scene_lock.read();
            (scene.world().tick(), scene.world().entity_count() as u64)
        };

        WorldRuntimeState {
            world_instance_id,
            phase,
            deterministic,
            boot_sequence,
            tick,
            entity_count,
            selected_entity: self.scene.selection_authority_handle(),
            partition,
            active_cells,
            authority: self.authority_json(),
            notes,
        }
    }

    #[inline]
    pub(crate) fn info_json(&self) -> WorldServiceInfo {
        WorldServiceInfo::default()
    }

    pub(crate) fn boot(&self, request: WorldBootRequest) -> WorldBootResponse {
        let headless = newengine_plugin_host::active_engine_gateway_route("engine.platform")
            .map(|route| route.provider_route_id.as_deref() == Some("engine.platform.headless"))
            .unwrap_or(false);
        let scene_declared = request
            .scene_ref
            .as_deref()
            .map(str::trim)
            .is_some_and(|scene_ref| !scene_ref.is_empty());
        let phase = if headless {
            WorldBootPhase::Headless
        } else if scene_declared {
            WorldBootPhase::SceneDeclared
        } else {
            WorldBootPhase::RuntimeBootstrapped
        };
        let note = format!(
            "boot: deterministic={} seed={} scene_ref={}",
            request.deterministic,
            request.seed,
            request.scene_ref.as_deref().unwrap_or("<none>")
        );
        let (active_cells, desired_cells) = Self::build_partition_cache(&request.partition);

        {
            let mut state = self.state.lock();
            state.boot_sequence = state.boot_sequence.saturating_add(1);
            state.deterministic = request.deterministic;
            state.phase = phase;
            state.partition = request.partition;
            state.active_cells = active_cells;
            state.desired_cells = desired_cells;
            state
                .notes
                .retain(|existing| !existing.starts_with("boot:"));
            state.notes.push(note);
        }

        WorldBootResponse {
            ok: true,
            state: self.runtime_state(true),
        }
    }

    pub(crate) fn boot_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        match decode_blob::<WorldBootRequest>(&payload) {
            Ok(request) => ok_json(self.boot(request)),
            Err(error) => RResult::RErr(error),
        }
    }

    pub(crate) fn state_response(&self, request: WorldStateRequest) -> WorldStateResponse {
        WorldStateResponse {
            state: self.runtime_state(request.include_cells),
        }
    }

    pub(crate) fn state_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        match decode_blob::<WorldStateRequest>(&payload) {
            Ok(request) => ok_json(self.state_response(request)),
            Err(error) => RResult::RErr(error),
        }
    }

    pub(crate) fn active_cells_response(
        &self,
        request: WorldActiveCellsRequest,
    ) -> WorldActiveCellsResponse {
        let state = self.state.lock();
        let cells = state
            .active_cells
            .iter()
            .filter(|cell| {
                request.include_unloaded || !matches!(cell.residency, WorldCellResidency::Unloaded)
            })
            .cloned()
            .collect();
        WorldActiveCellsResponse {
            partition: state.partition.clone(),
            cells,
        }
    }

    pub(crate) fn active_cells_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        match decode_blob::<WorldActiveCellsRequest>(&payload) {
            Ok(request) => ok_json(self.active_cells_response(request)),
            Err(error) => RResult::RErr(error),
        }
    }

    pub(crate) fn partition_response(&self) -> WorldPartitionResponse {
        WorldPartitionResponse {
            partition: self.state.lock().partition.clone(),
        }
    }

    pub(crate) fn partition_json_v1(&self) -> RResult<Blob, RString> {
        ok_json(self.partition_response())
    }
}
