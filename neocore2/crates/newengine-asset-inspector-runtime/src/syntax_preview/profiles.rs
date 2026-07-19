use super::*;

mod generic;
mod ini;
mod json;
mod text;
mod xml;

use generic::classify_generic;
use ini::classify_ini;
use json::classify_json;
use text::classify_text;
use xml::classify_xml;

pub(super) fn classify_line(
    chars: &[char],
    language: &str,
    state: &mut LexerState,
) -> Vec<SyntaxClass> {
    match normalize_language(language) {
        "xml" | "html" => classify_xml(chars, state),
        "json" | "json5" => classify_json(chars),
        "ini" | "cfg" => classify_ini(chars),
        "text" | "markdown" => classify_text(chars),
        other => classify_generic(chars, other, state),
    }
}

pub(super) fn normalize_language(language: &str) -> &str {
    match language.trim().to_ascii_lowercase().as_str() {
        "xml" => "xml",
        "html" => "html",
        "json" => "json",
        "json5" => "json5",
        "ini" => "ini",
        "cfg" => "cfg",
        "markdown" | "md" => "markdown",
        "plain" | "txt" | "text" => "text",
        "rust" | "rs" => "rust",
        "python" | "py" => "python",
        "javascript" | "js" => "javascript",
        "typescript" | "ts" => "typescript",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "lua" => "lua",
        "shell" | "sh" | "bash" => "shell",
        "powershell" | "ps1" => "powershell",
        "glsl" | "wgsl" | "hlsl" => "shader",
        _ => "code",
    }
}
