use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxProfile {
    pub id: String,
    pub content_kind: String,
    pub grammar: String,
    pub keywords: BTreeSet<String>,
    pub line_comment_prefixes: Vec<String>,
    pub block_comment_pairs: Vec<(String, String)>,
    pub string_delimiters: Vec<char>,
    pub supports_folding: bool,
    pub supports_outline: bool,
    pub supports_completion: bool,
    pub outline_provider: Option<String>,
    pub completion_provider: Option<String>,
    pub diagnostics_provider: Option<String>,
    pub theme_family: Option<String>,
}

impl SyntaxProfile {
    pub fn plain_text(content_kind: impl Into<String>) -> Self {
        let content_kind = content_kind.into();
        Self {
            id: format!("syntax.{content_kind}"),
            content_kind,
            grammar: "generic".to_owned(),
            keywords: BTreeSet::new(),
            line_comment_prefixes: Vec::new(),
            block_comment_pairs: Vec::new(),
            string_delimiters: vec!['"'],
            supports_folding: false,
            supports_outline: false,
            supports_completion: false,
            outline_provider: None,
            completion_provider: None,
            diagnostics_provider: None,
            theme_family: None,
        }
    }

    pub fn with_grammar(mut self, grammar: impl Into<String>) -> Self {
        self.grammar = grammar.into();
        self
    }

    pub fn with_keywords(mut self, keywords: &[&str]) -> Self {
        self.keywords = keywords.iter().map(|item| item.to_string()).collect();
        self
    }

    pub fn with_line_comments(mut self, prefixes: &[&str]) -> Self {
        self.line_comment_prefixes = prefixes.iter().map(|item| item.to_string()).collect();
        self
    }

    pub fn with_editor_features(mut self, folding: bool, outline: bool, completion: bool) -> Self {
        self.supports_folding = folding;
        self.supports_outline = outline;
        self.supports_completion = completion;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct SyntaxRegistry {
    profiles_by_content_kind: BTreeMap<String, SyntaxProfile>,
}

impl SyntaxRegistry {
    pub fn with_builtin_profiles() -> Self {
        let mut registry = Self::default();
        registry.register(
            SyntaxProfile::plain_text("xml_document")
                .with_grammar("xml")
                .with_editor_features(true, true, false),
        );
        registry.register(
            SyntaxProfile::plain_text("json_document")
                .with_keywords(&["true", "false", "null"])
                .with_line_comments(&["//"])
                .with_editor_features(true, true, false),
        );
        registry.register(
            SyntaxProfile::plain_text("toml_document")
                .with_keywords(&["true", "false"])
                .with_line_comments(&["#"])
                .with_editor_features(true, true, false),
        );
        registry.register(
            SyntaxProfile::plain_text("lua_script")
                .with_keywords(&[
                    "and", "break", "do", "else", "elseif", "end", "false", "for",
                    "function", "if", "in", "local", "nil", "not", "or", "repeat",
                    "return", "then", "true", "until", "while",
                ])
                .with_line_comments(&["--"])
                .with_editor_features(true, true, true),
        );
        registry.register(
            SyntaxProfile::plain_text("source_document")
                .with_keywords(&[
                    "as", "async", "await", "break", "const", "continue", "crate",
                    "else", "enum", "extern", "false", "fn", "for", "if", "impl",
                    "in", "let", "loop", "match", "mod", "move", "mut", "pub",
                    "ref", "return", "self", "Self", "static", "struct", "super",
                    "trait", "true", "type", "unsafe", "use", "where", "while",
                    "class", "def", "import", "from", "namespace", "using", "public",
                    "private", "protected", "virtual", "override", "void", "int",
                    "float", "double", "char", "bool", "string", "auto", "new",
                    "delete", "try", "catch", "throw",
                ])
                .with_line_comments(&["//", "#"])
                .with_editor_features(true, true, true),
        );
        registry.register(
            SyntaxProfile::plain_text("text_document")
                .with_line_comments(&["#", "//"])
                .with_editor_features(false, false, false),
        );
        registry.register(
            SyntaxProfile::plain_text("shader_source")
                .with_keywords(&[
                    "bool", "break", "cbuffer", "continue", "discard", "do", "else",
                    "float", "float2", "float3", "float4", "for", "half", "if", "int",
                    "matrix", "return", "sampler", "struct", "Texture2D", "uint", "void",
                    "while",
                ])
                .with_line_comments(&["//"])
                .with_editor_features(true, true, true),
        );
        registry
    }

    pub fn register(&mut self, profile: SyntaxProfile) {
        self.profiles_by_content_kind
            .insert(profile.content_kind.clone(), profile);
    }

    pub fn for_content_kind(&self, content_kind: &str) -> Option<&SyntaxProfile> {
        self.profiles_by_content_kind.get(content_kind)
    }
}
