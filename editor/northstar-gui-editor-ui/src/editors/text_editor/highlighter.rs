use crate::editors::text_editor::lexers::{generic, xml};
use crate::editors::text_editor::syntax::SyntaxProfile;
use crate::editors::text_editor::token::TokenSpan;

pub struct SyntaxHighlighter;

impl SyntaxHighlighter {
    pub fn highlight(text: &str, profile: &SyntaxProfile) -> Vec<TokenSpan> {
        match profile.grammar.as_str() {
            "xml" => xml::highlight(text),
            _ => generic::highlight(text, profile),
        }
    }
}
