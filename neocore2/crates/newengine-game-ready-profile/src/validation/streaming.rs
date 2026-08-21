use super::*;

impl GameReadyValidationModule {
    pub(super) fn streaming_update<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
    ) -> Result<bool, String> {
        if !Self::gameplay_active(ctx) {
            return Ok(false);
        }
        if self.delay_frames > 0 {
            self.delay_frames -= 1;
            return Ok(false);
        }
        if !newengine_plugin_host::has_service(ENGINE_WORLD_SERVICE_ID) {
            return Err(format!(
                "required service '{ENGINE_WORLD_SERVICE_ID}' is unavailable"
            ));
        }
        const ROUTE: [WorldCellCoord; 9] = [
            WorldCellCoord { x: 0, z: 0 },
            WorldCellCoord { x: 8, z: 0 },
            WorldCellCoord { x: 8, z: 8 },
            WorldCellCoord { x: 0, z: 8 },
            WorldCellCoord { x: -8, z: 8 },
            WorldCellCoord { x: -8, z: 0 },
            WorldCellCoord { x: -8, z: -8 },
            WorldCellCoord { x: 0, z: -8 },
            WorldCellCoord { x: 8, z: -8 },
        ];
        let center = ROUTE[self.step as usize % ROUTE.len()];
        let partition = WorldPartitionState {
            enabled: true,
            cell_size_x: 64,
            cell_size_z: 64,
            center,
            render_radius: 2,
            simulation_radius: 1,
        };
        let client =
            newengine_world_api::WorldClient::new(newengine_plugin_host::default_host_api());
        let boot = client.boot_json_v1(&WorldBootRequest {
            deterministic: true,
            seed: 0x4e_53_54_52,
            scene_ref: Some("maps/forest_road_operation.ymap".to_owned()),
            partition: partition.clone(),
        })?;
        if !boot.ok || boot.state.active_cells.len() != 25 {
            return Err(format!(
                "streaming boot mismatch step={} ok={} active_cells={}",
                self.step,
                boot.ok,
                boot.state.active_cells.len()
            ));
        }
        let streaming = client.streaming_cells_response_json_v1(&WorldStreamingCellsRequest {
            include_unloaded: true,
            include_reasons: true,
        })?;
        if streaming.plan.center != center
            || streaming.plan.desired_cells.len() != 25
            || streaming.cells.len() != 25
        {
            return Err(format!(
                "streaming plan mismatch step={} center={:?} actual_center={:?} desired={} cells={}",
                self.step,
                center,
                streaming.plan.center,
                streaming.plan.desired_cells.len(),
                streaming.cells.len()
            ));
        }
        let unique = streaming
            .plan
            .desired_cells
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if unique.len() != 25 {
            return Err(format!(
                "streaming plan contains duplicate cells step={}",
                self.step
            ));
        }
        let center_cell = streaming
            .cells
            .iter()
            .find(|cell| cell.coord == center)
            .ok_or_else(|| format!("streaming center cell missing step={}", self.step))?;
        if center_cell.residency != WorldCellResidency::RenderAndSimulation {
            return Err(format!(
                "streaming center residency mismatch step={} actual={:?}",
                self.step, center_cell.residency
            ));
        }

        self.step += 1;
        self.wait(2);
        if self.step.is_multiple_of(16) || self.step == self.streaming_steps {
            newengine_ulog_api::ulog::info!(
                "game-ready validation: world streaming progress step={}/{} center=({}, {}) desired_cells=25",
                self.step,
                self.streaming_steps,
                center.x,
                center.z,
            );
        }
        if self.step >= self.streaming_steps {
            newengine_ulog_api::ulog::info!(
                "game-ready validation: world streaming stress complete steps={} route_cells={} desired_per_step=25",
                self.streaming_steps,
                ROUTE.len(),
            );
            return Ok(true);
        }
        Ok(false)
    }

}
