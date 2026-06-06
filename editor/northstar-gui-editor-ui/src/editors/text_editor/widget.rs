use crate::editors::text_editor::diagnostics::{DiagnosticsOverlay, TextDiagnostic};
use crate::editors::text_editor::folding::{FoldRegion, FoldingBuilder};
use crate::editors::text_editor::highlighter::SyntaxHighlighter;
use crate::editors::text_editor::minimap::MinimapModel;
use crate::editors::text_editor::outline::{OutlineBuilder, OutlineSymbol};
use crate::editors::text_editor::syntax::{SyntaxProfile, SyntaxRegistry};
use crate::editors::text_editor::token::TokenSpan;
use crate::editors::text_editor::TextDocument;

#[derive(Debug, Clone)]
pub struct TextEditorWidget {
    pub document: TextDocument,
}

impl TextEditorWidget {
    pub fn syntax_profile<'a>(&self, registry: &'a SyntaxRegistry) -> Option<&'a SyntaxProfile> {
        registry.for_content_kind(&self.document.content_kind)
    }

    pub fn highlighted_spans(&self, registry: &SyntaxRegistry) -> Vec<TokenSpan> {
        let Some(profile) = self.syntax_profile(registry) else {
            return Vec::new();
        };
        SyntaxHighlighter::highlight(self.document.buffer.as_str(), profile)
    }

    pub fn outline_symbols(&self, registry: &SyntaxRegistry) -> Vec<OutlineSymbol> {
        let Some(profile) = self.syntax_profile(registry) else {
            return Vec::new();
        };
        if !profile.supports_outline {
            return Vec::new();
        }
        let spans = self.highlighted_spans(registry);
        OutlineBuilder::build(&self.document.buffer, &spans, profile)
    }

    pub fn fold_regions(&self, registry: &SyntaxRegistry) -> Vec<FoldRegion> {
        let Some(profile) = self.syntax_profile(registry) else {
            return Vec::new();
        };
        if !profile.supports_folding {
            return Vec::new();
        }
        let spans = self.highlighted_spans(registry);
        FoldingBuilder::build(&self.document.buffer, &spans, profile)
    }

    pub fn minimap(&self) -> MinimapModel {
        MinimapModel::from_text(self.document.buffer.as_str())
    }

    pub fn diagnostics_overlay(&self, diagnostics: Vec<TextDiagnostic>) -> DiagnosticsOverlay {
        let mut overlay = DiagnosticsOverlay::new();
        overlay.set(diagnostics);
        overlay
    }
}
