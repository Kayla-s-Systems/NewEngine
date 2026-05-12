#![forbid(unsafe_op_in_unsafe_fn)]

/// Coarse application/runtime FSM state owned by `newengine-core`.
///
/// External layers may observe this state, but they must not mirror it with
/// their own lifecycle flags. Runtime hosts, editors and gameplay runtimes
/// submit work to the core; the core decides when that work can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineRunState {
    Created,
    InitSystem,
    InitGame,
    Running,
    ShutdownGame,
    ShutdownSystem,
    Stopped,
    Faulted,
}

impl EngineRunState {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::InitSystem => "init-system",
            Self::InitGame => "init-game",
            Self::Running => "running",
            Self::ShutdownGame => "shutdown-game",
            Self::ShutdownSystem => "shutdown-system",
            Self::Stopped => "stopped",
            Self::Faulted => "faulted",
        }
    }

    #[inline]
    pub const fn is_booting(self) -> bool {
        matches!(self, Self::InitSystem | Self::InitGame)
    }

    #[inline]
    pub const fn is_running(self) -> bool {
        matches!(self, Self::Running)
    }

    #[inline]
    pub const fn is_shutting_down(self) -> bool {
        matches!(self, Self::ShutdownGame | Self::ShutdownSystem | Self::Stopped)
    }

    #[inline]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Stopped | Self::Faulted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineFsmTransition {
    pub previous: EngineRunState,
    pub next: EngineRunState,
    pub changed: bool,
    pub valid: bool,
}

/// Single source of truth for core lifecycle and cooperative shutdown.
///
/// This object deliberately replaces parallel lifecycle flags. The invariant is simple: code that needs lifecycle truth
/// reads this FSM; code that wants shutdown requests it through the FSM/token.
#[derive(Debug, Clone)]
pub struct EngineFsm {
    state: EngineRunState,
}

impl Default for EngineFsm {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl EngineFsm {
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: EngineRunState::Created,
        }
    }

    #[inline]
    pub const fn state(&self) -> EngineRunState {
        self.state
    }

    #[inline]
    pub const fn is_shutdown_requested(&self) -> bool {
        self.state.is_shutting_down() || self.state.is_terminal()
    }

    #[inline]
    pub const fn can_run_frame(&self) -> bool {
        matches!(self.state, EngineRunState::Running)
    }

    #[inline]
    pub fn request_shutdown(&mut self) -> EngineFsmTransition {
        if self.state.is_shutting_down() || self.state.is_terminal() {
            return EngineFsmTransition {
                previous: self.state,
                next: self.state,
                changed: false,
                valid: true,
            };
        }
        self.transition(EngineRunState::ShutdownGame)
    }

    #[inline]
    pub fn sync_external_shutdown(&mut self, requested: bool) -> EngineFsmTransition {
        if requested {
            self.request_shutdown()
        } else {
            EngineFsmTransition {
                previous: self.state,
                next: self.state,
                changed: false,
                valid: true,
            }
        }
    }

    #[inline]
    pub fn transition(&mut self, next: EngineRunState) -> EngineFsmTransition {
        let previous = self.state;
        if previous == next {
            return EngineFsmTransition {
                previous,
                next,
                changed: false,
                valid: true,
            };
        }

        let valid = is_valid_transition(previous, next);
        self.state = if valid { next } else { EngineRunState::Faulted };

        EngineFsmTransition {
            previous,
            next: self.state,
            changed: true,
            valid,
        }
    }
}

#[inline]
const fn is_valid_transition(previous: EngineRunState, next: EngineRunState) -> bool {
    use EngineRunState::*;

    matches!(
        (previous, next),
        (Created, InitSystem)
            | (Created, ShutdownGame)
            | (Created, Faulted)
            | (InitSystem, InitGame)
            | (InitSystem, ShutdownGame)
            | (InitSystem, Faulted)
            | (InitGame, Running)
            | (InitGame, ShutdownGame)
            | (InitGame, Faulted)
            | (Running, ShutdownGame)
            | (Running, Faulted)
            | (ShutdownGame, ShutdownSystem)
            | (ShutdownGame, Faulted)
            | (ShutdownSystem, Stopped)
            | (ShutdownSystem, Faulted)
            | (Stopped, Faulted)
    )
}
