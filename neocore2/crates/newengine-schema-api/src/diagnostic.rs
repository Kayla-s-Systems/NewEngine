use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaDiagnosticV1 {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl Default for SchemaDiagnosticV1 {
    #[inline]
    fn default() -> Self {
        Self {
            severity: "info".to_owned(),
            code: String::new(),
            message: String::new(),
            path: None,
        }
    }
}

impl SchemaDiagnosticV1 {
    #[inline]
    fn with_severity(severity: &str, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: severity.to_owned(),
            code: code.into(),
            message: message.into(),
            path: None,
        }
    }

    #[inline]
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::with_severity("info", code, message)
    }

    #[inline]
    pub fn warn(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::with_severity("warning", code, message)
    }

    #[inline]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::with_severity("error", code, message)
    }
}
