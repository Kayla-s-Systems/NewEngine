use newengine_core::Resources;
use newengine_runtime_session_api::{
    RuntimeSessionCommand, RuntimeSessionCommandFrame, RuntimeSessionControlMode,
    RuntimeSessionFrameDecision, RuntimeSessionId, RuntimeSessionMode, RuntimeSessionPhase,
    RuntimeSessionState,
};

const MAX_STEP_BUDGET: u32 = 120;

pub fn install_runtime_session_resources(resources: &mut Resources) {
    if resources.get::<RuntimeSessionState>().is_none() {
        resources.insert(RuntimeSessionState::default());
    }
}

pub fn submit_runtime_session_command(
    resources: &mut Resources,
    frame_index: u64,
    source: impl Into<String>,
    command: RuntimeSessionCommand,
) {
    if let Some(frame) = resources.get_mut::<RuntimeSessionCommandFrame>() {
        if frame.frame_index == frame_index {
            frame.commands.push(command);
            return;
        }
    }
    resources.insert(RuntimeSessionCommandFrame::single(
        frame_index,
        source,
        command,
    ));
}

pub fn advance_runtime_session(resources: &mut Resources, frame_index: u64) -> RuntimeSessionState {
    install_runtime_session_resources(resources);
    let commands = resources
        .remove::<RuntimeSessionCommandFrame>()
        .map(|frame| frame.commands)
        .unwrap_or_default();
    let state = resources
        .get_mut::<RuntimeSessionState>()
        .expect("runtime session state installed");
    state.frame_index = frame_index;

    for command in commands {
        apply_command(state, frame_index, command);
    }
    state.clone()
}

pub fn begin_runtime_session_frame(
    resources: &mut Resources,
    frame_index: u64,
) -> RuntimeSessionFrameDecision {
    install_runtime_session_resources(resources);
    let state = resources
        .get_mut::<RuntimeSessionState>()
        .expect("runtime session state installed");
    state.frame_index = frame_index;
    let active = state.is_active();
    let step_this_frame = active && state.paused && state.step_budget > 0;
    if step_this_frame {
        state.step_budget = state.step_budget.saturating_sub(1);
        state.last_reason = "single fixed-step frame released while paused".to_owned();
    }
    RuntimeSessionFrameDecision {
        active,
        paused: active && state.paused,
        step_this_frame,
        possessed: state.is_possessed(),
        mode: state.mode,
    }
}

pub fn record_runtime_session_ticks(resources: &mut Resources, ticks: u32) {
    if ticks == 0 {
        return;
    }
    if let Some(state) = resources.get_mut::<RuntimeSessionState>() {
        if state.is_active() {
            state.simulation_tick = state.simulation_tick.saturating_add(ticks as u64);
        }
    }
}

fn apply_command(
    state: &mut RuntimeSessionState,
    frame_index: u64,
    command: RuntimeSessionCommand,
) {
    match command {
        RuntimeSessionCommand::Start { mode } => {
            if state.is_active() && state.mode == Some(mode) {
                state.paused = false;
                state.phase = RuntimeSessionPhase::Running;
                state.phase_frame_index = frame_index;
                state.step_budget = 0;
                state.last_reason = "active session resumed by start command".to_owned();
            } else {
                start_session(state, frame_index, mode, "runtime session started");
            }
        }
        RuntimeSessionCommand::Pause => {
            if state.is_active() {
                state.paused = true;
                state.phase = RuntimeSessionPhase::Paused;
                state.phase_frame_index = frame_index;
                state.last_reason = "runtime session paused".to_owned();
            }
        }
        RuntimeSessionCommand::Resume => {
            if state.is_active() {
                state.paused = false;
                state.phase = RuntimeSessionPhase::Running;
                state.phase_frame_index = frame_index;
                state.step_budget = 0;
                state.last_reason = "runtime session resumed".to_owned();
            }
        }
        RuntimeSessionCommand::TogglePause => {
            if state.is_active() {
                if state.paused {
                    apply_command(state, frame_index, RuntimeSessionCommand::Resume);
                } else {
                    apply_command(state, frame_index, RuntimeSessionCommand::Pause);
                }
            }
        }
        RuntimeSessionCommand::Stop => {
            enter_idle(state, frame_index, "runtime session stopped");
        }
        RuntimeSessionCommand::Restart => {
            if let Some(mode) = state.mode {
                start_session(
                    state,
                    frame_index,
                    mode,
                    "runtime session restarted in the live world",
                );
            }
        }
        RuntimeSessionCommand::Eject => {
            if state.is_active() && state.mode == Some(RuntimeSessionMode::Play) {
                state.control_mode = RuntimeSessionControlMode::Ejected;
                state.last_reason =
                    "player ejected from live runtime possession; simulation continues".to_owned();
            }
        }
        RuntimeSessionCommand::Possess => {
            if state.is_active() && state.mode == Some(RuntimeSessionMode::Play) {
                state.control_mode = RuntimeSessionControlMode::Possessed;
                state.last_reason =
                    "player possession restored in active live runtime session".to_owned();
            }
        }
        RuntimeSessionCommand::Step { frames } => {
            if state.is_active() && state.paused {
                let frames = frames.clamp(1, MAX_STEP_BUDGET);
                state.step_budget = state
                    .step_budget
                    .saturating_add(frames)
                    .min(MAX_STEP_BUDGET);
                state.last_reason = format!("queued {frames} paused simulation step(s)");
            }
        }
    }
}

fn start_session(
    state: &mut RuntimeSessionState,
    frame_index: u64,
    mode: RuntimeSessionMode,
    reason: &str,
) {
    state.session_id = RuntimeSessionId(state.session_id.0.wrapping_add(1).max(1));
    state.generation = state.generation.wrapping_add(1);
    state.phase = RuntimeSessionPhase::Running;
    state.mode = Some(mode);
    state.paused = false;
    state.control_mode = if mode == RuntimeSessionMode::Play {
        RuntimeSessionControlMode::Possessed
    } else {
        RuntimeSessionControlMode::Ejected
    };
    state.frame_index = frame_index;
    state.phase_frame_index = frame_index;
    state.simulation_tick = 0;
    state.step_budget = 0;
    state.last_reason = reason.to_owned();
}

fn enter_idle(state: &mut RuntimeSessionState, frame_index: u64, reason: &str) {
    state.phase = RuntimeSessionPhase::Idle;
    state.mode = None;
    state.paused = false;
    state.control_mode = RuntimeSessionControlMode::Possessed;
    state.frame_index = frame_index;
    state.phase_frame_index = frame_index;
    state.step_budget = 0;
    state.last_reason = reason.to_owned();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_pause_step_resume_stop_flow_is_stable() {
        let mut resources = Resources::default();
        submit_runtime_session_command(
            &mut resources,
            1,
            "test",
            RuntimeSessionCommand::Start {
                mode: RuntimeSessionMode::Play,
            },
        );
        let state = advance_runtime_session(&mut resources, 1);
        assert!(state.is_active());
        assert_eq!(state.mode, Some(RuntimeSessionMode::Play));

        submit_runtime_session_command(&mut resources, 2, "test", RuntimeSessionCommand::Pause);
        let state = advance_runtime_session(&mut resources, 2);
        assert!(state.is_paused());

        submit_runtime_session_command(
            &mut resources,
            3,
            "test",
            RuntimeSessionCommand::Step { frames: 1 },
        );
        advance_runtime_session(&mut resources, 3);
        let decision = begin_runtime_session_frame(&mut resources, 3);
        assert!(decision.step_this_frame);
        let decision = begin_runtime_session_frame(&mut resources, 3);
        assert!(!decision.step_this_frame);

        submit_runtime_session_command(&mut resources, 4, "test", RuntimeSessionCommand::Resume);
        assert!(!advance_runtime_session(&mut resources, 4).paused);

        submit_runtime_session_command(&mut resources, 5, "test", RuntimeSessionCommand::Stop);
        assert!(!advance_runtime_session(&mut resources, 5).is_active());
    }
}

pub const RUNTIME_SESSION_COMMAND_SERVICE_ID: &str = "engine.runtime.session.commands";

pub mod runtime_session_command_method {
    pub const PLAY: &str = "runtime_session.play_v1";
    pub const SIMULATE: &str = "runtime_session.simulate_v1";
    pub const PAUSE: &str = "runtime_session.pause_v1";
    pub const RESUME: &str = "runtime_session.resume_v1";
    pub const STOP: &str = "runtime_session.stop_v1";
    pub const RESTART: &str = "runtime_session.restart_v1";
    pub const EJECT: &str = "runtime_session.eject_v1";
    pub const POSSESS: &str = "runtime_session.possess_v1";
    pub const STEP: &str = "runtime_session.step_v1";
}

fn external_command_queue(
) -> &'static std::sync::Mutex<std::collections::VecDeque<RuntimeSessionCommand>> {
    static QUEUE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::VecDeque<RuntimeSessionCommand>>,
    > = std::sync::OnceLock::new();
    QUEUE.get_or_init(|| std::sync::Mutex::new(std::collections::VecDeque::new()))
}

pub fn enqueue_external_runtime_session_command(command: RuntimeSessionCommand) {
    let mut queue = external_command_queue()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    queue.push_back(command);
}

pub fn drain_external_runtime_session_commands() -> Vec<RuntimeSessionCommand> {
    let mut queue = external_command_queue()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    queue.drain(..).collect()
}

struct RuntimeSessionCommandService;

impl newengine_plugin_api::ServiceV1 for RuntimeSessionCommandService {
    fn id(&self) -> newengine_plugin_api::CapabilityId {
        abi_stable::std_types::RString::from(RUNTIME_SESSION_COMMAND_SERVICE_ID)
    }

    fn describe(&self) -> abi_stable::std_types::RString {
        use runtime_session_command_method as method;
        abi_stable::std_types::RString::from(
            serde_json::json!({
                "id": RUNTIME_SESSION_COMMAND_SERVICE_ID,
                "version": 1,
                "protocol": "newengine.runtime-session-command/v1",
                "methods": [
                    { "name": method::PLAY, "payload": "empty" },
                    { "name": method::SIMULATE, "payload": "empty" },
                    { "name": method::PAUSE, "payload": "empty" },
                    { "name": method::RESUME, "payload": "empty" },
                    { "name": method::STOP, "payload": "empty" },
                    { "name": method::RESTART, "payload": "empty" },
                    { "name": method::EJECT, "payload": "empty" },
                    { "name": method::POSSESS, "payload": "empty" },
                    { "name": method::STEP, "payload": "utf8 optional frame count" }
                ],
                "console": {
                    "contract": "newengine.command-descriptor/v1",
                    "commands": [
                        console_command("runtime.play", "Start the live runtime play session", method::PLAY, "empty", "runtime.play"),
                        console_command("runtime.simulate", "Start simulation without player possession", method::SIMULATE, "empty", "runtime.simulate"),
                        console_command("runtime.pause", "Pause the active runtime session", method::PAUSE, "empty", "runtime.pause"),
                        console_command("runtime.resume", "Resume the active runtime session", method::RESUME, "empty", "runtime.resume"),
                        console_command("runtime.stop", "Stop runtime control without replacing the live world", method::STOP, "empty", "runtime.stop"),
                        console_command("runtime.restart", "Restart the active runtime session", method::RESTART, "empty", "runtime.restart"),
                        console_command("runtime.eject", "Eject player controls while the live world keeps running", method::EJECT, "empty", "runtime.eject"),
                        console_command("runtime.possess", "Return player possession to the active live runtime", method::POSSESS, "empty", "runtime.possess"),
                        console_command("runtime.step", "Advance paused simulation by N fixed steps", method::STEP, "raw", "runtime.step [frames]")
                    ]
                }
            })
            .to_string(),
        )
    }

    fn call(
        &self,
        method: newengine_plugin_api::MethodName,
        payload: newengine_plugin_api::Blob,
    ) -> abi_stable::std_types::RResult<newengine_plugin_api::Blob, abi_stable::std_types::RString>
    {
        use runtime_session_command_method as m;
        let method = method.to_string();
        let command = match method.as_str() {
            m::PLAY => RuntimeSessionCommand::Start {
                mode: RuntimeSessionMode::Play,
            },
            m::SIMULATE => RuntimeSessionCommand::Start {
                mode: RuntimeSessionMode::Simulate,
            },
            m::PAUSE => RuntimeSessionCommand::Pause,
            m::RESUME => RuntimeSessionCommand::Resume,
            m::STOP => RuntimeSessionCommand::Stop,
            m::RESTART => RuntimeSessionCommand::Restart,
            m::EJECT => RuntimeSessionCommand::Eject,
            m::POSSESS => RuntimeSessionCommand::Possess,
            m::STEP => {
                let raw = String::from_utf8_lossy(payload.as_slice());
                let frames = if raw.trim().is_empty() {
                    1
                } else {
                    match raw.trim().parse::<u32>() {
                        Ok(frames) if frames > 0 => frames.min(MAX_STEP_BUDGET),
                        _ => {
                            return abi_stable::std_types::RResult::RErr(
                                abi_stable::std_types::RString::from(
                                    "runtime.step expects a positive integer frame count",
                                ),
                            );
                        }
                    }
                };
                RuntimeSessionCommand::Step { frames }
            }
            _ => {
                return abi_stable::std_types::RResult::RErr(abi_stable::std_types::RString::from(
                    "unknown runtime-session command method",
                ));
            }
        };
        enqueue_external_runtime_session_command(command);
        abi_stable::std_types::RResult::ROk(newengine_plugin_api::Blob::from(
            serde_json::json!({ "ok": true, "queued": method })
                .to_string()
                .into_bytes(),
        ))
    }
}

fn console_command(
    name: &str,
    help: &str,
    method: &str,
    payload: &str,
    usage: &str,
) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "help": help,
        "usage": usage,
        "kind": "service_call",
        "service_id": RUNTIME_SESSION_COMMAND_SERVICE_ID,
        "method": method,
        "payload": payload,
        "owner": "newengine-runtime-session-runtime",
        "flags": {
            "developer": false,
            "read_only": false,
            "remote_allowed": false
        }
    })
}

pub fn init_runtime_session_command_service() {
    let service = RuntimeSessionCommandService;
    let dyn_service =
        newengine_plugin_api::ServiceV1Dyn::from_value(service, abi_stable::sabi_trait::TD_Opaque);
    let _ = newengine_plugin_host::host_register_service_impl(dyn_service);
}
