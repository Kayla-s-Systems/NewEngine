#![forbid(unsafe_op_in_unsafe_fn)]

use super::*;

impl EngineWorldGatewayService {
    pub(crate) fn streaming_response(
        &self,
        req: WorldStreamingCellsRequest,
    ) -> WorldStreamingCellsResponse {
        let state = self.state.lock().clone();
        let mut cells = state
            .active_cells
            .iter()
            .filter(|cell| {
                req.include_unloaded || !matches!(cell.residency, WorldCellResidency::Unloaded)
            })
            .map(|cell| WorldStreamingCellDto {
                coord: cell.coord,
                residency: cell.residency.clone(),
                dirty: cell.dirty,
                reason: if req.include_reasons {
                    cell.reason.clone()
                } else {
                    String::new()
                },
            })
            .collect::<Vec<_>>();
        cells.sort_by_key(|cell| {
            let dx = cell.coord.x - state.partition.center.x;
            let dz = cell.coord.z - state.partition.center.z;
            (dx * dx + dz * dz, cell.coord.x, cell.coord.z)
        });
        let desired_cells = Self::build_active_cells(state.partition.clone())
            .into_iter()
            .map(|cell| cell.coord)
            .collect::<Vec<_>>();
        WorldStreamingCellsResponse {
            partition: state.partition.clone(),
            plan: WorldStreamingPlanDto {
                center: state.partition.center,
                render_radius: state.partition.render_radius,
                simulation_radius: state.partition.simulation_radius,
                desired_cells,
            },
            cells,
        }
    }

    pub(crate) fn streaming_cells_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<WorldStreamingCellsRequest>(v).map_err(|e| e.to_string())
        }) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        ok_json(self.streaming_response(req))
    }
}
