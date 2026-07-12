use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::ok_json;
use newengine_world_api::{
    WorldCellResidency, WorldStreamingCellDto, WorldStreamingCellsRequest,
    WorldStreamingCellsResponse, WorldStreamingPlanDto,
};

use crate::{partition::distance_key, payload::decode_blob, service::EngineWorldGatewayService};

impl EngineWorldGatewayService {
    pub(crate) fn streaming_response(
        &self,
        request: WorldStreamingCellsRequest,
    ) -> WorldStreamingCellsResponse {
        let state = self.state.lock();
        let partition = state.partition.clone();
        let desired_cells = state.desired_cells.clone();
        let mut cells = Vec::with_capacity(state.active_cells.len());

        for cell in &state.active_cells {
            if !request.include_unloaded && matches!(cell.residency, WorldCellResidency::Unloaded) {
                continue;
            }

            cells.push(WorldStreamingCellDto {
                coord: cell.coord,
                residency: cell.residency,
                dirty: cell.dirty,
                reason: if request.include_reasons {
                    cell.reason.clone()
                } else {
                    String::new()
                },
            });
        }
        drop(state);

        cells.sort_unstable_by_key(|cell| distance_key(cell.coord, partition.center));

        WorldStreamingCellsResponse {
            plan: WorldStreamingPlanDto {
                center: partition.center,
                render_radius: partition.render_radius,
                simulation_radius: partition.simulation_radius,
                desired_cells,
            },
            partition,
            cells,
        }
    }

    pub(crate) fn streaming_cells_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        match decode_blob::<WorldStreamingCellsRequest>(&payload) {
            Ok(request) => ok_json(self.streaming_response(request)),
            Err(error) => RResult::RErr(error),
        }
    }
}
