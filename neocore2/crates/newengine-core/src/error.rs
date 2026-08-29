use std::error::Error;
use std::fmt;

/// Engine-wide error.
///
/// Keep this small and stable. Modules may define their own error types and map them into `EngineError`.
#[derive(Debug)]
pub enum EngineError {
    /// Graceful shutdown was requested.
    ExitRequested,

    /// Error produced by a module during a known lifecycle stage.
    Module {
        module_id: &'static str,
        stage: ModuleStage,
        cause: Box<EngineError>,
    },

    /// Generic error (fallback).
    Other(String),
}

/// Module lifecycle stage used for error attribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleStage {
    Init,
    Start,
    FixedUpdate,
    Update,
    Render,
    ExternalEvent,
    Shutdown,
}

impl EngineError {
    #[inline]
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    #[inline]
    pub fn with_module_stage(
        module_id: &'static str,
        stage: ModuleStage,
        err: EngineError,
    ) -> Self {
        match err {
            EngineError::ExitRequested => EngineError::ExitRequested,
            other => EngineError::Module {
                module_id,
                stage,
                cause: Box::new(other),
            },
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::ExitRequested => write!(f, "exit requested"),
            EngineError::Other(s) => write!(f, "{s}"),
            EngineError::Module {
                module_id,
                stage,
                cause,
            } => write!(f, "module '{module_id}' stage {stage:?}: {cause}"),
        }
    }
}

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            EngineError::Module { cause, .. } => Some(cause.as_ref()),
            _ => None,
        }
    }
}

impl From<&str> for EngineError {
    #[inline]
    fn from(value: &str) -> Self {
        EngineError::Other(value.to_string())
    }
}

impl From<String> for EngineError {
    #[inline]
    fn from(value: String) -> Self {
        EngineError::Other(value)
    }
}

pub type EngineResult<T> = Result<T, EngineError>;
