#![forbid(unsafe_op_in_unsafe_fn)]

use crate::module::Module;

/// Execution state of a module inside the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleState {
    Pending,
    Running,
    Disabled,
}

/// A module plus host-owned state (enabled/disabled, reason, shutdown status).
pub struct ModuleSlot<E: Send + 'static> {
    pub module: Box<dyn Module<E>>,
    pub state: ModuleState,
    pub disabled_reason: Option<String>,
    pub shutdown_called: bool,
}

impl<E: Send + 'static> ModuleSlot<E> {
    #[inline]
    pub fn new(module: Box<dyn Module<E>>) -> Self {
        Self {
            module,
            state: ModuleState::Pending,
            disabled_reason: None,
            shutdown_called: false,
        }
    }

    #[inline]
    pub fn id(&self) -> &'static str {
        self.module.id()
    }

    #[inline]
    pub fn is_running(&self) -> bool {
        self.state == ModuleState::Running
    }

    #[inline]
    pub fn is_disabled(&self) -> bool {
        self.state == ModuleState::Disabled
    }

    #[inline]
    pub fn disable(&mut self, reason: String) {
        self.state = ModuleState::Disabled;
        self.disabled_reason = Some(reason);
    }
}
