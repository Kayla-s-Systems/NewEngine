use crate::editors::text_editor::syntax::SyntaxProfile;
use crate::editors::text_editor::token::{TokenKind, TokenSpan};
use crate::editors::text_editor::TextBuffer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineSymbol {
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub column: usize,
}

pub struct OutlineBuilder;

impl OutlineBuilder {
    pub fn build(buffer: &TextBuffer, tokens: &[TokenSpan], profile: &SyntaxProfile) -> Vec<OutlineSymbol> {
        match profile.grammar.as_str() {
            "xml" => build_xml_outline(buffer, tokens),
            _ => build_generic_outline(buffer, tokens, profile),
        }
    }
}

fn build_xml_outline(buffer: &TextBuffer, tokens: &[TokenSpan]) -> Vec<OutlineSymbol> {
    let mut symbols = Vec::new();
    let text = buffer.as_str();
    for window in tokens.windows(2) {
        let tag = &window[0];
        let name = &window[1];
        if tag.kind == TokenKind::Tag && &text[tag.start..tag.end] == "<" && name.kind == TokenKind::Identifier {
            let symbol_name = text[name.start..name.end].to_owned();
            if !symbol_name.starts_with('/') && !symbol_name.starts_with('!') && !symbol_name.starts_with('?') {
                let (line, column) = buffer.line_column_for_offset(name.start);
                symbols.push(OutlineSymbol {
                    name: symbol_name,
                    kind: "xml.tag".to_owned(),
                    line,
                    column,
                });
            }
        }
    }
    symbols
}

fn build_generic_outline(buffer: &TextBuffer, tokens: &[TokenSpan], profile: &SyntaxProfile) -> Vec<OutlineSymbol> {
    let mut symbols = Vec::new();
    let text = buffer.as_str();
    let mut previous_keyword: Option<&str> = None;

    for token in tokens {
        if token.kind == TokenKind::Keyword {
            previous_keyword = Some(&text[token.start..token.end]);
            continue;
        }

        if token.kind == TokenKind::Identifier {
            let Some(keyword) = previous_keyword.take() else {
                continue;
            };
            let is_symbol = match profile.content_kind.as_str() {
                "lua_script" => keyword == "function" || keyword == "local",
                "shader_source" => keyword == "struct" || keyword == "cbuffer" || keyword == "void" || keyword.starts_with("float") || keyword == "int" || keyword == "bool",
                _ => false,
            };
            if is_symbol {
                let (line, column) = buffer.line_column_for_offset(token.start);
                symbols.push(OutlineSymbol {
                    name: text[token.start..token.end].to_owned(),
                    kind: format!("{}.symbol", profile.content_kind),
                    line,
                    column,
                });
            }
        }
    }

    symbols
}
