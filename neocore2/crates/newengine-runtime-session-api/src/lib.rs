use serde::{Deserialize, Serialize};

pub const RUNTIME_SESSION_CONTRACT: &str = "newengine.runtime-session.v1";
pub const RUNTIME_SESSION_COMMAND_SOURCE_EDITOR: &str = "engine.editor";
pub const RUNTIME_SESSION_COMMAND_SOURCE_GAME: &str = "engine.game";
pub const RUNTIME_SESSION_COMMAND_SOURCE_CONSOLE: &str = "engine.command";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimeSessionId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSessionMode {
    Simulate,
    Play,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSessionControlMode {
    #[default]
    Possessed,
    Ejected,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSessionPhase {
    #[default]
    Idle,
    Starting,
    Running,
    Paused,
    Stopping,
    Restoring,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "command")]
pub enum RuntimeSessionCommand {
    Start { mode: RuntimeSessionMode },
    Pause,
    Resume,
    TogglePause,
    Stop,
    Restart,
    Eject,
    Possess,
    ApplyChangesAndStop,
    Step { frames: u32 },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeSessionCommandFrame {
    pub version: u32,
    pub frame_index: u64,
    pub source: String,
    pub commands: Vec<RuntimeSessionCommand>,
}

impl RuntimeSessionCommandFrame {
    pub fn single(
        frame_index: u64,
        source: impl Into<String>,
        command: RuntimeSessionCommand,
    ) -> Self {
        Self {
            version: 1,
            frame_index,
            source: source.into(),
            commands: vec![command],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeSessionState {
    pub version: u32,
    pub session_id: RuntimeSessionId,
    pub generation: u64,
    pub phase: RuntimeSessionPhase,
    pub mode: Option<RuntimeSessionMode>,
    pub paused: bool,
    pub control_mode: RuntimeSessionControlMode,
    pub apply_changes_requested: bool,
    pub frame_index: u64,
    pub phase_frame_index: u64,
    pub simulation_tick: u64,
    pub step_budget: u32,
    pub pending_start_mode: Option<RuntimeSessionMode>,
    pub world_snapshot_id: Option<String>,
    pub last_reason: String,
}

impl Default for RuntimeSessionState {
    fn default() -> Self {
        Self {
            version: 1,
            session_id: RuntimeSessionId(0),
            generation: 0,
            phase: RuntimeSessionPhase::Idle,
            mode: None,
            paused: false,
            control_mode: RuntimeSessionControlMode::Possessed,
            apply_changes_requested: false,
            frame_index: 0,
            phase_frame_index: 0,
            simulation_tick: 0,
            step_budget: 0,
            pending_start_mode: None,
            world_snapshot_id: None,
            last_reason: "runtime session idle".to_owned(),
        }
    }
}

impl RuntimeSessionState {
    #[inline]
    pub fn is_active(&self) -> bool {
        self.mode.is_some()
            && matches!(
                self.phase,
                RuntimeSessionPhase::Starting
                    | RuntimeSessionPhase::Running
                    | RuntimeSessionPhase::Paused
            )
    }

    #[inline]
    pub fn is_paused(&self) -> bool {
        self.is_active() && self.paused
    }

    #[inline]
    pub fn is_possessed(&self) -> bool {
        self.is_active()
            && self.mode == Some(RuntimeSessionMode::Play)
            && self.control_mode == RuntimeSessionControlMode::Possessed
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeSessionFrameDecision {
    pub active: bool,
    pub paused: bool,
    pub step_this_frame: bool,
    pub possessed: bool,
    pub mode: Option<RuntimeSessionMode>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeWorldChangeKind {
    Added,
    Removed,
    #[default]
    Modified,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeWorldChangeV1 {
    pub path: String,
    pub kind: RuntimeWorldChangeKind,
    pub before_json: Option<String>,
    pub after_json: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeWorldDiffV1 {
    pub version: u32,
    pub session_id: RuntimeSessionId,
    pub frame_index: u64,
    pub change_count: usize,
    pub changes: Vec<RuntimeWorldChangeV1>,
    pub truncated: bool,
    pub reason: String,
}

impl RuntimeWorldDiffV1 {
    pub fn empty(
        session_id: RuntimeSessionId,
        frame_index: u64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            version: 1,
            session_id,
            frame_index,
            reason: reason.into(),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_session_is_not_active() {
        let state = RuntimeSessionState::default();
        assert!(!state.is_active());
        assert!(!state.is_paused());
    }

    #[test]
    fn command_frame_is_provider_neutral() {
        let frame = RuntimeSessionCommandFrame::single(
            7,
            RUNTIME_SESSION_COMMAND_SOURCE_EDITOR,
            RuntimeSessionCommand::Start {
                mode: RuntimeSessionMode::Play,
            },
        );
        assert_eq!(frame.frame_index, 7);
        assert_eq!(frame.commands.len(), 1);
    }
}
