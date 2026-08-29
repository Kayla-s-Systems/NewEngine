use super::*;

impl GameReadyValidationModule {
    fn write_snapshot_atomic(path: &Path, payload: &str) -> Result<(), String> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!("create save directory '{}': {error}", parent.display())
            })?;
        }
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, payload)
            .map_err(|error| format!("write temporary save '{}': {error}", temporary.display()))?;
        if path.exists() {
            fs::remove_file(path)
                .map_err(|error| format!("remove previous save '{}': {error}", path.display()))?;
        }
        fs::rename(&temporary, path).map_err(|error| {
            format!(
                "commit save '{}' -> '{}': {error}",
                temporary.display(),
                path.display()
            )
        })
    }

    pub(super) fn normalized_snapshot(
        mut snapshot: WorldSnapshotResponse,
    ) -> WorldSnapshotResponse {
        snapshot.state.notes.clear();
        snapshot.state.tick = 0;
        snapshot
    }

    fn save(&self, client: &newengine_world_api::WorldClient) -> Result<(), String> {
        let response = client.save_snapshot_json_v1(&WorldSaveSnapshotRequest {
            include_scene_payload: true,
            include_cells: true,
            target_ref: Some(self.save_path.to_string_lossy().into_owned()),
        })?;
        if !response.ok {
            return Err("engine.world returned ok=false while saving".to_owned());
        }
        if response.storage != "caller-owned" {
            return Err(format!(
                "unexpected snapshot storage policy '{}'",
                response.storage
            ));
        }

        let reparsed = serde_json::from_str::<WorldSnapshotResponse>(&response.payload_text)
            .map_err(|error| format!("saved payload is not a WorldSnapshotResponse: {error}"))?;
        if reparsed != response.snapshot {
            return Err("payload_text does not match typed snapshot".to_owned());
        }
        Self::write_snapshot_atomic(&self.save_path, &response.payload_text)?;

        let persisted = fs::read_to_string(&self.save_path).map_err(|error| {
            format!(
                "read committed save '{}': {error}",
                self.save_path.display()
            )
        })?;
        let persisted_snapshot = serde_json::from_str::<WorldSnapshotResponse>(&persisted)
            .map_err(|error| format!("committed save parse failed: {error}"))?;
        if persisted_snapshot != response.snapshot {
            return Err("committed save differs from engine.world snapshot".to_owned());
        }

        newengine_ulog_api::ulog::info!(
            "game-ready validation: save snapshot complete path='{}' schema='{}' world_instance_id='{}' active_cells={} entities={} scene_payload={}",
            self.save_path.display(),
            response.snapshot.schema,
            response.snapshot.state.world_instance_id,
            response.snapshot.state.active_cells.len(),
            response.snapshot.state.entity_count,
            response.snapshot.scene_payload.is_some(),
        );
        Ok(())
    }

    fn load(&self, client: &newengine_world_api::WorldClient) -> Result<(), String> {
        let payload_text = fs::read_to_string(&self.save_path)
            .map_err(|error| format!("read save '{}': {error}", self.save_path.display()))?;
        let expected = serde_json::from_str::<WorldSnapshotResponse>(&payload_text)
            .map_err(|error| format!("save parse failed: {error}"))?;

        let response = client.load_snapshot_json_v1(&WorldLoadSnapshotRequest {
            snapshot: Some(expected.clone()),
            payload: None,
            replace_scene: true,
        })?;
        if !response.ok {
            return Err("engine.world returned ok=false while loading".to_owned());
        }

        let actual = client.snapshot_response_json_v1()?;
        if Self::normalized_snapshot(actual.clone()) != Self::normalized_snapshot(expected.clone())
        {
            return Err(format!(
                "restored snapshot mismatch expected_world='{}' actual_world='{}' expected_cells={} actual_cells={} expected_entities={} actual_entities={}",
                expected.state.world_instance_id,
                actual.state.world_instance_id,
                expected.state.active_cells.len(),
                actual.state.active_cells.len(),
                expected.state.entity_count,
                actual.state.entity_count,
            ));
        }

        newengine_ulog_api::ulog::info!(
            "game-ready validation: load snapshot complete path='{}' schema='{}' world_instance_id='{}' active_cells={} entities={} scene_payload={}",
            self.save_path.display(),
            actual.schema,
            actual.state.world_instance_id,
            actual.state.active_cells.len(),
            actual.state.entity_count,
            actual.scene_payload.is_some(),
        );
        Ok(())
    }

    pub(super) fn run_snapshot_validation(&self) -> Result<(), String> {
        if !newengine_plugin_host::has_service(ENGINE_WORLD_SERVICE_ID) {
            return Err(format!(
                "required service '{ENGINE_WORLD_SERVICE_ID}' is unavailable"
            ));
        }
        let client =
            newengine_world_api::WorldClient::new(newengine_plugin_host::default_host_api());
        match self.scenario {
            ValidationScenario::Save => self.save(&client),
            ValidationScenario::Load => self.load(&client),
            _ => Err("invalid snapshot validation scenario".to_owned()),
        }
    }
}
