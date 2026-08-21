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

}

#[path = "validation/snapshot.rs"]
mod snapshot;
#[path = "validation/input.rs"]
mod input;
#[path = "validation/streaming.rs"]
mod streaming;

impl GameReadyValidationModule {
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
