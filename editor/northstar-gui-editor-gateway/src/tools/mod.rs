use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::registry::ProviderDescriptor;

#[derive(Debug, Clone)]
pub struct ToolManifest {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub source: PathBuf,
    pub capabilities: Vec<String>,
    pub formats: Vec<String>,
}

impl ToolManifest {
    pub fn parse(text: &str, source: PathBuf) -> Result<Self, String> {
        let id = json_string_field(text, "id")
            .ok_or_else(|| format!("tool manifest misses id: {}", source.display()))?;
        let name = json_string_field(text, "name").unwrap_or_else(|| id.clone());
        let kind = json_string_field(text, "kind").unwrap_or_else(|| "tool".to_owned());
        let capabilities = json_string_array(text, "capabilities");
        let mut formats = json_string_array(text, "formats");
        formats.extend(infer_formats_from_text(text));
        formats.sort();
        formats.dedup();

        Ok(Self {
            id,
            name,
            kind,
            source,
            capabilities: normalize_capabilities(capabilities),
            formats,
        })
    }

    pub fn into_provider_descriptor(self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: self.id,
            name: self.name,
            kind: self.kind,
            source: self.source,
            capabilities: self.capabilities,
            formats: self.formats,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodecManifest {
    pub source: PathBuf,
    pub codecs: Vec<CodecManifestEntry>,
}

#[derive(Debug, Clone)]
pub struct CodecManifestEntry {
    pub codec: String,
    pub version: String,
    pub dll: String,
}

impl CodecManifest {
    pub fn parse(text: &str, source: PathBuf) -> Result<Self, String> {
        let mut codecs = Vec::new();
        let mut cursor = 0;
        while let Some(codec_pos) = text[cursor..].find("\"codec\"") {
            let start = cursor + codec_pos;
            let object_end = text[start..]
                .find('}')
                .map(|offset| start + offset)
                .unwrap_or(text.len());
            let object = &text[start..object_end];
            let codec = json_string_field(object, "codec").unwrap_or_default();
            if !codec.is_empty() {
                codecs.push(CodecManifestEntry {
                    codec,
                    version: json_string_field(object, "version").unwrap_or_default(),
                    dll: json_string_field(object, "dll").unwrap_or_default(),
                });
            }
            cursor = object_end.saturating_add(1);
        }

        Ok(Self { source, codecs })
    }

    pub fn into_provider_descriptors(self) -> Vec<ProviderDescriptor> {
        let base_dir = self.source.parent().map(PathBuf::from).unwrap_or_default();
        let manifest_source = self.source.clone();
        self.codecs
            .into_iter()
            .map(|entry| {
                let source = if entry.dll.is_empty() {
                    manifest_source.clone()
                } else {
                    base_dir.join(&entry.dll)
                };
                let formats = infer_formats_from_codec(&entry.codec);
                ProviderDescriptor {
                    id: entry.codec.clone(),
                    name: if entry.version.is_empty() {
                        entry.codec.clone()
                    } else {
                        format!("{} {}", entry.codec, entry.version)
                    },
                    kind: "codec-manifest-entry".to_owned(),
                    source,
                    capabilities: codec_capabilities(&entry.codec),
                    formats,
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ToolPlaneBridge {
    newengine_root: PathBuf,
    repo_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ToolPlaneResult {
    pub command_id: String,
    pub available: bool,
    pub exit_code: Option<i32>,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub diagnostics: Vec<String>,
}

impl ToolPlaneBridge {
    pub fn new(newengine_root: PathBuf) -> Self {
        let repo_root = find_repo_root(&newengine_root).unwrap_or_else(|| newengine_root.clone());
        Self {
            newengine_root,
            repo_root,
        }
    }

    pub fn tools_list(&self) -> ToolPlaneResult {
        self.run_takesome_tool_command(
            "tools.list",
            &[
                vec!["tools", "list"],
                vec!["tools-list"],
                vec!["tools", "scan"],
            ],
        )
    }

    pub fn tools_doctor(&self) -> ToolPlaneResult {
        self.run_takesome_tool_command(
            "tools.doctor",
            &[
                vec!["tools", "doctor"],
                vec!["tools-doctor"],
                vec!["tools", "validate"],
                vec!["tools-validate"],
            ],
        )
    }

    fn run_takesome_tool_command(&self, command_id: &str, attempts: &[Vec<&str>]) -> ToolPlaneResult {
        let script = self.repo_root.join("tools").join("scripts").join("takesome.py");
        if !script.exists() {
            return ToolPlaneResult::unavailable(
                command_id,
                format!("tool-plane script not found: {}", script.display()),
            );
        }

        let python = std::env::var("NEWENGINE_PYTHON_CMD")
            .ok()
            .and_then(|value| value.split_whitespace().next().map(ToOwned::to_owned))
            .unwrap_or_else(|| "python".to_owned());

        let mut failed_attempts = Vec::new();
        for args in attempts {
            let output = Command::new(&python)
                .arg(&script)
                .args(args)
                .current_dir(&self.repo_root)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();

            match output {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    return ToolPlaneResult {
                        command_id: command_id.to_owned(),
                        available: true,
                        exit_code: output.status.code(),
                        stdout_tail: tail(&stdout, 80),
                        stderr_tail: tail(&stderr, 80),
                        diagnostics: vec![format!(
                            "tool-plane command '{}' succeeded via {} {} {}",
                            command_id,
                            python,
                            script.display(),
                            args.join(" ")
                        )],
                    };
                }
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    failed_attempts.push(format!(
                        "{} -> exit={:?}\nstdout:\n{}\nstderr:\n{}",
                        args.join(" "),
                        output.status.code(),
                        tail(&stdout, 12),
                        tail(&stderr, 12)
                    ));
                }
                Err(err) => {
                    failed_attempts.push(format!("{} -> failed to execute: {err}", args.join(" ")));
                }
            }
        }

        ToolPlaneResult {
            command_id: command_id.to_owned(),
            available: true,
            exit_code: Some(1),
            stdout_tail: String::new(),
            stderr_tail: failed_attempts.join("\n---\n"),
            diagnostics: vec![format!(
                "tool-plane command '{}' had no successful invocation through {} from {}",
                command_id,
                script.display(),
                self.newengine_root.display()
            )],
        }
    }
}

fn find_repo_root(newengine_root: &std::path::Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(newengine_root.to_path_buf());
    if let Ok(current) = std::env::current_dir() {
        candidates.push(current.clone());
        candidates.push(current.join(newengine_root));
    }
    if let Some(engine_repo) = newengine_root.parent() {
        candidates.push(engine_repo.to_path_buf());
        if let Some(repo_root) = engine_repo.parent() {
            candidates.push(repo_root.to_path_buf());
        }
    }

    for candidate in candidates {
        let mut cursor = Some(candidate.as_path());
        while let Some(path) = cursor {
            if path.join("tools").join("scripts").join("takesome.py").exists() {
                return Some(path.to_path_buf());
            }
            cursor = path.parent();
        }
    }

    None
}

impl ToolPlaneResult {
    fn unavailable(command_id: &str, message: String) -> Self {
        Self {
            command_id: command_id.to_owned(),
            available: false,
            exit_code: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            diagnostics: vec![message],
        }
    }

    pub fn print(&self) {
        println!("[TOOL-PLANE] command={}", self.command_id);
        println!("[TOOL-PLANE] available={}", self.available);
        println!("[TOOL-PLANE] exit_code={}", self.exit_code.map(|code| code.to_string()).unwrap_or_else(|| "<none>".to_owned()));
        for diagnostic in &self.diagnostics {
            println!("[TOOL-PLANE][DIAG] {diagnostic}");
        }
        if !self.stdout_tail.trim().is_empty() {
            println!("[TOOL-PLANE][STDOUT]");
            println!("{}", self.stdout_tail.trim_end());
        }
        if !self.stderr_tail.trim().is_empty() {
            println!("[TOOL-PLANE][STDERR]");
            println!("{}", self.stderr_tail.trim_end());
        }
    }

    pub fn is_success(&self) -> bool {
        self.available && self.exit_code == Some(0)
    }
}

fn codec_capabilities(codec: &str) -> Vec<String> {
    let mut capabilities = vec![
        "asset.format.read".to_owned(),
        "asset.format.inspect".to_owned(),
        "asset.format.validate".to_owned(),
    ];
    let lower = codec.to_ascii_lowercase();
    if lower.contains("listfile") || lower.contains("nepak") {
        capabilities.push("asset.format.write".to_owned());
        capabilities.push("asset.format.diff".to_owned());
    }
    if lower.contains("ytd") || lower.contains("texture") {
        capabilities.push("asset.preview.texture".to_owned());
        capabilities.push("asset.editor.texture.edit_schema".to_owned());
    }
    if lower.contains("ydd") || lower.contains("model") {
        capabilities.push("asset.preview.model".to_owned());
        capabilities.push("asset.editor.model.edit_schema".to_owned());
    }
    capabilities
}

fn normalize_capabilities(mut capabilities: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for capability in capabilities.drain(..) {
        normalized.push(capability.clone());
        if capability.ends_with("_inspect") || capability.ends_with(".inspect") {
            normalized.push("asset.format.inspect".to_owned());
        }
        if capability.ends_with("_extract") || capability.ends_with(".extract") {
            normalized.push("asset.format.read".to_owned());
        }
        if capability.ends_with("_pack") || capability.ends_with(".pack") {
            normalized.push("asset.format.write".to_owned());
        }
        if capability.contains("validation") || capability.ends_with(".validate") {
            normalized.push("asset.format.validate".to_owned());
        }
    }
    normalized.sort();
    normalized.dedup();
    normalized
}

fn infer_formats_from_text(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut formats = Vec::new();
    for token in ["ytd", "ydd", "ytyp", "ymap", "nemat", "nepak", "neui", "listfile", "nef8"] {
        if lower.contains(token) {
            formats.push(format!(".{token}"));
        }
    }
    formats
}

fn infer_formats_from_codec(codec: &str) -> Vec<String> {
    let lower = codec.to_ascii_lowercase();
    let mut formats = Vec::new();
    for token in ["listfile", "nepak", "ytd", "ydd", "ytyp", "ymap", "nemat", "neui", "nef8"] {
        if lower.contains(token) {
            formats.push(format!(".{token}"));
        }
    }
    formats
}

fn json_string_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let idx = text.find(&needle)?;
    let after_key = &text[idx + needle.len()..];
    let colon = after_key.find(':')?;
    let mut chars = after_key[colon + 1..].chars().peekable();
    while matches!(chars.peek(), Some(ch) if ch.is_whitespace()) {
        chars.next();
    }
    if chars.next()? != '"' {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    for ch in chars {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

fn json_string_array(text: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let Some(idx) = text.find(&needle) else { return Vec::new(); };
    let after_key = &text[idx + needle.len()..];
    let Some(colon) = after_key.find(':') else { return Vec::new(); };
    let after_colon = &after_key[colon + 1..];
    let Some(open) = after_colon.find('[') else { return Vec::new(); };
    let Some(close) = after_colon[open + 1..].find(']') else { return Vec::new(); };
    let array = &after_colon[open + 1..open + 1 + close];

    let mut values = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut current = String::new();

    for ch in array.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' if in_string => {
                in_string = false;
                values.push(current.clone());
                current.clear();
            }
            '"' => in_string = true,
            _ if in_string => current.push(ch),
            _ => {}
        }
    }

    values
}

fn tail(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}
