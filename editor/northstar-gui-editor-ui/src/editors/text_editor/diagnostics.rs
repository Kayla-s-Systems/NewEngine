#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDiagnostic {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticsOverlay {
    diagnostics: Vec<TextDiagnostic>,
}

impl DiagnosticsOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, diagnostics: Vec<TextDiagnostic>) {
        self.diagnostics = diagnostics;
        self.diagnostics.sort_by_key(|diagnostic| (diagnostic.line, diagnostic.column));
    }

    pub fn diagnostics(&self) -> &[TextDiagnostic] {
        &self.diagnostics
    }

    pub fn diagnostics_for_line(&self, line: usize) -> Vec<&TextDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.line == line)
            .collect()
    }

    pub fn highest_severity(&self) -> Option<DiagnosticSeverity> {
        if self.diagnostics.iter().any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error) {
            Some(DiagnosticSeverity::Error)
        } else if self.diagnostics.iter().any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning) {
            Some(DiagnosticSeverity::Warning)
        } else if !self.diagnostics.is_empty() {
            Some(DiagnosticSeverity::Info)
        } else {
            None
        }
    }
}
