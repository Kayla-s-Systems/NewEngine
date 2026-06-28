#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorDiagnostic {
    pub severity: DiagnosticSeverity,
    pub domain: String,
    pub message: String,
}

impl EditorDiagnostic {
    pub fn info(domain: impl Into<String>, message: impl Into<String>) -> Self {
        Self { severity: DiagnosticSeverity::Info, domain: domain.into(), message: message.into() }
    }

    pub fn warn(domain: impl Into<String>, message: impl Into<String>) -> Self {
        Self { severity: DiagnosticSeverity::Warn, domain: domain.into(), message: message.into() }
    }

    pub fn error(domain: impl Into<String>, message: impl Into<String>) -> Self {
        Self { severity: DiagnosticSeverity::Error, domain: domain.into(), message: message.into() }
    }

    pub fn print(&self) {
        let level = match self.severity {
            DiagnosticSeverity::Info => "INFO",
            DiagnosticSeverity::Warn => "WARN",
            DiagnosticSeverity::Error => "ERROR",
        };
        println!("[DIAGNOSTIC][{level}][{}] {}", self.domain, self.message);
    }
}
