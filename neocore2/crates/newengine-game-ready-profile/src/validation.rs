#![forbid(unsafe_op_in_unsafe_fn)]

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use abi_stable::std_types::RString;
use newengine_core::{EngineError, EngineResult, Module, ModuleCtx};
use newengine_plugin_api::{Blob, MethodName};
use newengine_ui_api::UiPresentationFlowState;
use newengine_world_api::{
    WorldBootRequest, WorldCellCoord, WorldCellResidency, WorldLoadSnapshotRequest,
    WorldPartitionState, WorldSaveSnapshotRequest, WorldSnapshotResponse,
    WorldStreamingCellsRequest, ENGINE_WORLD_SERVICE_ID,
};

pub(crate) const GAME_READY_VALIDATION_SCENARIO_ENV: &str =
    "NEWENGINE_GAME_READY_VALIDATION_SCENARIO";
pub(crate) const GAME_READY_VALIDATION_SAVE_PATH_ENV: &str =
    "NEWENGINE_GAME_READY_VALIDATION_SAVE_PATH";
const GAME_READY_STREAMING_STEPS_ENV: &str = "NEWENGINE_GAME_READY_STREAMING_STEPS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationScenario {
    Save,
    Load,
    Controller,
    Hotplug,
    Streaming,
}

impl ValidationScenario {
    fn from_env() -> Option<Self> {
        match crate::env_config::var(GAME_READY_VALIDATION_SCENARIO_ENV)?
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "save" => Some(Self::Save),
            "load" => Some(Self::Load),
            "controller" => Some(Self::Controller),
            "hotplug" => Some(Self::Hotplug),
            "streaming" => Some(Self::Streaming),
            _ => None,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Save => "save",
            Self::Load => "load",
            Self::Controller => "controller",
            Self::Hotplug => "hotplug",
            Self::Streaming => "streaming",
        }
    }
}

pub(crate) struct GameReadyValidationModule {
    scenario: ValidationScenario,
    save_path: PathBuf,
    gameplay_frames: u32,
    completed: bool,
    step: u32,
    delay_frames: u32,
    streaming_steps: u32,
}

impl GameReadyValidationModule {
    pub(crate) fn from_env() -> Option<Self> {
        let scenario = ValidationScenario::from_env()?;
        let save_path = crate::env_config::path(GAME_READY_VALIDATION_SAVE_PATH_ENV)
            .unwrap_or_else(|| PathBuf::from("target/smoke/game-ready-world-save.json"));
        let streaming_steps =
            crate::env_config::var_u32(GAME_READY_STREAMING_STEPS_ENV, 128, 8, 4096);
        Some(Self {
            scenario,
            save_path,
            gameplay_frames: 0,
            completed: false,
            step: 0,
            delay_frames: 0,
            streaming_steps,
        })
    }

    fn gameplay_active<E: Send + 'static>(ctx: &ModuleCtx<'_, E>) -> bool {
        ctx.resources()
            .get::<UiPresentationFlowState>()
            .is_some_and(|flow| flow.state_id == "gameplay" && flow.runtime_ready)
    }

    fn flow_state_id<E: Send + 'static>(ctx: &ModuleCtx<'_, E>) -> Option<String> {
        ctx.resources()
            .get::<UiPresentationFlowState>()
            .map(|flow| flow.state_id.clone())
    }

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

    fn normalized_snapshot(mut snapshot: WorldSnapshotResponse) -> WorldSnapshotResponse {
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

    fn input_call(method: &str, payload: &serde_json::Value) -> Result<serde_json::Value, String> {
        if !newengine_plugin_host::has_service(newengine_input_api::ENGINE_INPUT_SERVICE_ID) {
            return Err(format!(
                "required input gateway '{}' is unavailable",
                newengine_input_api::ENGINE_INPUT_SERVICE_ID
            ));
        }
        let bytes = serde_json::to_vec(payload).map_err(|error| error.to_string())?;
        let host = newengine_plugin_host::default_host_api();
        let response = (host.call_service_v1)(
            RString::from(newengine_input_api::ENGINE_INPUT_SERVICE_ID),
            MethodName::from(method),
            Blob::from(bytes),
        )
        .into_result()
        .map_err(|error| error.to_string())?
        .into_vec();
        if response.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_slice(&response).map_err(|error| error.to_string())
    }

    fn input_ingest(topic: &str, data: serde_json::Value) -> Result<(), String> {
        Self::input_call(
            newengine_input_api::INPUT_METHOD_INGEST_JSON,
            &serde_json::json!({ "topic": topic, "data": data }),
        )?;
        Ok(())
    }

    fn input_state() -> Result<serde_json::Value, String> {
        Self::input_call(
            newengine_input_api::INPUT_METHOD_STATE_JSON,
            &serde_json::Value::Null,
        )
    }

    fn gamepad_device(connected: bool) -> Result<(), String> {
        Self::input_ingest(
            "test.device",
            serde_json::json!({
                "id": "game-ready-controller",
                "kind": "gamepad",
                "connected": connected,
            }),
        )
    }

    fn gamepad_button(button: &str, pressed: bool) -> Result<(), String> {
        Self::input_ingest(
            "test.gamepad.button",
            serde_json::json!({
                "id": "game-ready-controller",
                "button": button,
                "state": if pressed { "pressed" } else { "released" },
            }),
        )
    }

    fn wait(&mut self, frames: u32) {
        self.delay_frames = frames;
    }

    fn controller_update<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
    ) -> Result<bool, String> {
        if self.delay_frames > 0 {
            self.delay_frames -= 1;
            return Ok(false);
        }
        let state_id = Self::flow_state_id(ctx);
        let state_id = state_id.as_deref();
        match self.step {
            0 if state_id == Some("main_menu") => {
                Self::gamepad_device(true)?;
                self.step = 1;
                self.wait(3);
            }
            1 if state_id == Some("main_menu") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::DPAD_DOWN, true)?;
                self.step = 2;
                self.wait(3);
            }
            2 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::DPAD_DOWN, false)?;
                self.step = 3;
                self.wait(3);
            }
            3 if state_id == Some("main_menu") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::SOUTH, true)?;
                self.step = 4;
                self.wait(3);
            }
            4 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::SOUTH, false)?;
                self.step = 5;
            }
            5 if state_id == Some("gameplay") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::START, true)?;
                self.step = 6;
                self.wait(3);
            }
            6 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::START, false)?;
                self.step = 7;
            }
            7 if state_id == Some("pause") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::DPAD_DOWN, true)?;
                self.step = 8;
                self.wait(3);
            }
            8 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::DPAD_DOWN, false)?;
                self.step = 9;
                self.wait(3);
            }
            9 if state_id == Some("pause") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::DPAD_DOWN, true)?;
                self.step = 10;
                self.wait(3);
            }
            10 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::DPAD_DOWN, false)?;
                self.step = 11;
                self.wait(3);
            }
            11 if state_id == Some("pause") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::SOUTH, true)?;
                self.step = 12;
                self.wait(3);
            }
            12 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::SOUTH, false)?;
                self.step = 13;
            }
            13 if state_id == Some("pause_settings") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::EAST, true)?;
                self.step = 14;
                self.wait(3);
            }
            14 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::EAST, false)?;
                self.step = 15;
            }
            15 if state_id == Some("pause") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::START, true)?;
                self.step = 16;
                self.wait(3);
            }
            16 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::START, false)?;
                self.step = 17;
            }
            17 if state_id == Some("gameplay") => {
                Self::gamepad_device(false)?;
                newengine_ulog_api::ulog::info!(
                    "game-ready validation: controller-only flow complete path='main_menu->gameplay->pause->pause_settings->pause->gameplay' input='virtual gamepad only'"
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }

    fn expect_device(
        snapshot: &serde_json::Value,
        id: &str,
        kind: &str,
        connected: bool,
    ) -> Result<(), String> {
        let device = &snapshot["devices"][id];
        if device["kind"].as_str() != Some(kind)
            || device["connected"].as_bool() != Some(connected)
            || device["virtual"].as_bool() != Some(true)
        {
            return Err(format!(
                "device snapshot mismatch id='{id}' expected kind='{kind}' connected={connected} actual={device}"
            ));
        }
        Ok(())
    }

    fn hotplug(&self) -> Result<(), String> {
        for (id, kind) in [
            ("keyboard0", "keyboard"),
            ("mouse0", "mouse"),
            ("game-ready-controller", "gamepad"),
        ] {
            Self::input_ingest(
                "test.device",
                serde_json::json!({ "id": id, "kind": kind, "connected": true }),
            )?;
        }
        let connected = Self::input_state()?;
        Self::expect_device(&connected, "virtual:keyboard0", "keyboard", true)?;
        Self::expect_device(&connected, "virtual:mouse0", "mouse", true)?;
        Self::expect_device(&connected, "virtual:game-ready-controller", "gamepad", true)?;

        Self::input_ingest(
            "test.gamepad.axis",
            serde_json::json!({
                "id": "game-ready-controller",
                "axis": newengine_input_api::gamepad_axis::LEFT_STICK_X,
                "value": 0.75,
            }),
        )?;
        Self::gamepad_button(newengine_input_api::gamepad_button::SOUTH, true)?;
        let pressed = Self::input_state()?;
        let pad = &pressed["gamepads"]["virtual:game-ready-controller"];
        if pad["connected"].as_bool() != Some(true)
            || pad["buttons"][newengine_input_api::gamepad_button::SOUTH].as_f64() != Some(1.0)
            || !pad["buttons_pressed"]
                .as_array()
                .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("South")))
            || pad["axes"][newengine_input_api::gamepad_axis::LEFT_STICK_X].as_f64() != Some(0.75)
        {
            return Err(format!(
                "virtual gamepad edge/axis snapshot mismatch: {pad}"
            ));
        }
        let consumed = Self::input_state()?;
        if !consumed["gamepads"]["virtual:game-ready-controller"]["buttons_pressed"]
            .as_array()
            .is_some_and(Vec::is_empty)
        {
            return Err("gamepad pressed edge was not one-shot".to_owned());
        }
        Self::gamepad_button(newengine_input_api::gamepad_button::SOUTH, false)?;
        let released = Self::input_state()?;
        if !released["gamepads"]["virtual:game-ready-controller"]["buttons_released"]
            .as_array()
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("South")))
        {
            return Err("gamepad release edge missing".to_owned());
        }

        for (id, kind) in [
            ("keyboard0", "keyboard"),
            ("mouse0", "mouse"),
            ("game-ready-controller", "gamepad"),
        ] {
            Self::input_ingest(
                "test.device",
                serde_json::json!({ "id": id, "kind": kind, "connected": false }),
            )?;
        }
        let disconnected = Self::input_state()?;
        Self::expect_device(&disconnected, "virtual:keyboard0", "keyboard", false)?;
        Self::expect_device(&disconnected, "virtual:mouse0", "mouse", false)?;
        Self::expect_device(
            &disconnected,
            "virtual:game-ready-controller",
            "gamepad",
            false,
        )?;
        newengine_ulog_api::ulog::info!(
            "game-ready validation: device hot-plug complete devices=3 lifecycle='connect->input-edge->disconnect'"
        );
        Ok(())
    }

    fn streaming_update<E: Send + 'static>(
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

    fn run_snapshot_validation(&self) -> Result<(), String> {
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

    fn fail(&self, error: String) -> EngineError {
        EngineError::Other(format!(
            "game-ready validation scenario='{}' failed: {error}",
            self.scenario.label()
        ))
    }
}

impl<E: Send + 'static> Module<E> for GameReadyValidationModule {
    fn id(&self) -> &'static str {
        "app.game_ready_validation"
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if self.completed {
            return Ok(());
        }

        let complete = match self.scenario {
            ValidationScenario::Save | ValidationScenario::Load => {
                if !Self::gameplay_active(ctx) {
                    return Ok(());
                }
                self.gameplay_frames = self.gameplay_frames.saturating_add(1);
                if self.gameplay_frames < 12 {
                    return Ok(());
                }
                self.run_snapshot_validation()
                    .map_err(|error| self.fail(error))?;
                true
            }
            ValidationScenario::Controller => self
                .controller_update(ctx)
                .map_err(|error| self.fail(error))?,
            ValidationScenario::Hotplug => {
                if !newengine_plugin_host::has_service(newengine_input_api::ENGINE_INPUT_SERVICE_ID)
                {
                    return Ok(());
                }
                self.hotplug().map_err(|error| self.fail(error))?;
                true
            }
            ValidationScenario::Streaming => self
                .streaming_update(ctx)
                .map_err(|error| self.fail(error))?,
        };
        if complete {
            self.completed = true;
            return Err(EngineError::ExitRequested);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_ignores_runtime_tick_and_restore_note() {
        let mut snapshot = WorldSnapshotResponse {
            schema: "newengine.world.snapshot.v1".to_owned(),
            state: newengine_world_api::WorldRuntimeState {
                world_instance_id: "world".to_owned(),
                phase: newengine_world_api::WorldBootPhase::Playable,
                deterministic: true,
                boot_sequence: 1,
                tick: 44,
                entity_count: 3,
                selected_entity: None,
                partition: WorldPartitionState::default(),
                active_cells: Vec::new(),
                authority: serde_json::Value::Null,
                notes: vec!["restore note".to_owned()],
            },
            scene_payload: None,
        };
        let normalized = GameReadyValidationModule::normalized_snapshot(snapshot.clone());
        snapshot.state.tick = 0;
        snapshot.state.notes.clear();
        assert_eq!(normalized, snapshot);
    }

    #[test]
    fn validation_scenarios_are_stable_labels() {
        assert_eq!(ValidationScenario::Controller.label(), "controller");
        assert_eq!(ValidationScenario::Hotplug.label(), "hotplug");
        assert_eq!(ValidationScenario::Streaming.label(), "streaming");
    }
}
