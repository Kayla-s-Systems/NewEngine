#![forbid(unsafe_op_in_unsafe_fn)]

use env_logger::fmt::Target;

/// Console destination.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LogOutput {
    Stdout,
    Stderr,
}

impl LogOutput {
    #[inline]
    pub fn to_env_target(self) -> Target {
        match self {
            LogOutput::Stdout => Target::Stdout,
            LogOutput::Stderr => Target::Stderr,
        }
    }
}
