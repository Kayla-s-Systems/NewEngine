use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptDiagnosticSeverity {
    Trace,
    Info,
    Warning,
    Error,
}

impl Default for ScriptDiagnosticSeverity {
    #[inline]
    fn default() -> Self {
        Self::Info
    }
}

pub type ScriptingDiagnosticSeverity = ScriptDiagnosticSeverity;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptDiagnostic {
    pub severity: ScriptDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub script_ref: String,
    pub payload_bytes: Vec<u8>,
}

impl Default for ScriptDiagnostic {
    #[inline]
    fn default() -> Self {
        Self {
            severity: ScriptDiagnosticSeverity::Info,
            code: String::new(),
            message: String::new(),
            script_ref: String::new(),
            payload_bytes: Vec::new(),
        }
    }
}

impl ScriptDiagnostic {
    #[inline]
    fn with_severity(
        severity: ScriptDiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::with_severity(ScriptDiagnosticSeverity::Info, code, message)
    }

    #[inline]
    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::with_severity(ScriptDiagnosticSeverity::Warning, code, message)
    }

    #[inline]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::with_severity(ScriptDiagnosticSeverity::Error, code, message)
    }
}

pub type ScriptingDiagnostic = ScriptDiagnostic;
