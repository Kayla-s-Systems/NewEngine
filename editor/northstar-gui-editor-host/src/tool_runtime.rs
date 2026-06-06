use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use northstar_gui_editor_assets::format_types::{FormatTypeCapabilities, RuntimeTypeRegistration};
use northstar_gui_editor_gateway::registry::ProviderDescriptor;

#[derive(Debug, Clone)]

#[derive(PartialEq, Eq)]
pub struct ToolRouteDescriptor {
    pub extension: String,
    pub provider_id: String,
    pub executable: PathBuf,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPreviewOutput {
    pub provider_id: String,
    pub command: String,
    pub lines: Vec<String>,
    pub diagnostics: Vec<String>,
}

pub struct ToolRuntimeDiscoveryResult {
    pub providers: Vec<ProviderDescriptor>,
    pub registrations: Vec<RuntimeTypeRegistration>,
    pub diagnostics: Vec<String>,
}

pub struct ToolMountStore;

impl ToolMountStore {
    pub fn settings_path(newengine_root: &Path) -> PathBuf {
        newengine_root
            .join("editor")
            .join("northstar-gui-editor")
            .join("runtime_tool_mounts.json")
    }

    pub fn remember_result(newengine_root: &Path, root: &Path, result: &ToolRuntimeDiscoveryResult) -> Result<(), String> {
        let path = Self::settings_path(newengine_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }

        let mut accepted_tools = Vec::new();
        for provider in &result.providers {
            accepted_tools.push(provider.source.display().to_string());
        }
        let mut mounted_roots = Self::load_roots(newengine_root);
        let root_text = root.display().to_string();
        if !mounted_roots.iter().any(|item| item == &root_text) {
            mounted_roots.push(root_text);
        }
        mounted_roots.sort();
        mounted_roots.dedup();

        let mut text = String::new();
        text.push_str("{\n");
        text.push_str("  \"schema\": \"northstar.gui_editor.runtime_tool_mounts.v1\",\n");
        text.push_str("  \"tool_roots\": [\n");
        for (index, root) in mounted_roots.iter().enumerate() {
            let comma = if index + 1 == mounted_roots.len() { "" } else { "," };
            text.push_str(&format!("    \"{}\"{}\n", escape_json(root), comma));
        }
        text.push_str("  ],\n");
        text.push_str("  \"accepted_tools\": [\n");
        for (index, tool) in accepted_tools.iter().enumerate() {
            let comma = if index + 1 == accepted_tools.len() { "" } else { "," };
            text.push_str(&format!("    \"{}\"{}\n", escape_json(tool), comma));
        }
        text.push_str("  ]\n");
        text.push_str("}\n");
        fs::write(&path, text).map_err(|err| format!("failed to write {}: {err}", path.display()))
    }

    pub fn load_roots(newengine_root: &Path) -> Vec<String> {
        let path = Self::settings_path(newengine_root);
        let Ok(text) = fs::read_to_string(path) else { return Vec::new(); };
        json_string_array(&text, "tool_roots")
    }
}

pub fn discover_remembered_self_describing_tools(newengine_root: &Path) -> Result<ToolRuntimeDiscoveryResult, String> {
    let mut merged = ToolRuntimeDiscoveryResult {
        providers: Vec::new(),
        registrations: Vec::new(),
        diagnostics: Vec::new(),
    };

    for root in ToolMountStore::load_roots(newengine_root) {
        let path = PathBuf::from(&root);
        match discover_self_describing_tools(&path) {
            Ok(result) => {
                merged.diagnostics.push(format!("restored runtime tool mount: {root}"));
                merged.providers.extend(result.providers);
                merged.registrations.extend(result.registrations);
                merged.diagnostics.extend(result.diagnostics);
            }
            Err(err) => merged.diagnostics.push(format!("failed to restore runtime tool mount {root}: {err}")),
        }
    }
    Ok(merged)
}

pub fn discover_self_describing_tools(root: &Path) -> Result<ToolRuntimeDiscoveryResult, String> {
    if !root.exists() {
        return Err(format!("tool root does not exist: {}", root.display()));
    }

    let mut executables = Vec::new();
    collect_executables(root, &mut executables)
        .map_err(|err| format!("failed to scan tool root {}: {err}", root.display()))?;

    let mut providers = Vec::new();
    let mut registrations = Vec::new();
    let mut diagnostics = Vec::new();

    for executable in executables {
        match ask_tool(&executable) {
            Ok(stdout) => {
                let provider = provider_from_self_description(&executable, &stdout);
                diagnostics.push(format!("accepted self-describing tool: {}", executable.display()));
                registrations.extend(runtime_types_from_provider(&provider));
                providers.push(provider);
            }
            Err(err) => diagnostics.push(format!("rejected tool {}: {err}", executable.display())),
        }
    }

    Ok(ToolRuntimeDiscoveryResult { providers, registrations, diagnostics })
}

fn ask_tool(executable: &Path) -> Result<String, String> {
    for args in [["describe", "--schema", "northstar.tool_provider.v1"].as_slice(), ["accepted-inputs"].as_slice()] {
        let output = Command::new(executable)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        match output {
            Ok(output) if output.status.success() => return Ok(String::from_utf8_lossy(&output.stdout).to_string()),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if args == ["accepted-inputs"].as_slice() {
                    return Err(format!("self-description failed exit={:?} stderr={}", output.status.code(), tail(&stderr, 8)));
                }
            }
            Err(err) => return Err(format!("spawn failed: {err}")),
        }
    }
    Err("self-description command unavailable".to_owned())
}

fn collect_executables(root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(name.as_str(), ".git" | ".takesome" | "target" | "cache" | "logs" | "test" | "testData" | "_out") {
                continue;
            }
            collect_executables(&path, out)?;
        } else if path.extension().and_then(|value| value.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("exe")) {
            out.push(path);
        }
    }
    Ok(())
}

fn provider_from_self_description(executable: &Path, stdout: &str) -> ProviderDescriptor {
    let stem = executable.file_stem().and_then(|value| value.to_str()).unwrap_or("northstar-tool");
    let id = json_string_field(stdout, "provider_id")
        .or_else(|| json_string_field(stdout, "id"))
        .unwrap_or_else(|| format!("runtime.{}", stem.replace('-', "_")));
    let mut formats = json_string_array(stdout, "extensions");
    formats.extend(infer_formats(stdout));
    formats.extend(infer_formats(stem));
    formats = unique_sorted(formats);
    if formats.is_empty() {
        formats.push(".*".to_owned());
    }
    ProviderDescriptor {
        id,
        name: json_string_field(stdout, "name").unwrap_or_else(|| stem.to_owned()),
        kind: "runtime-self-describing-tool".to_owned(),
        source: executable.to_path_buf(),
        capabilities: capabilities_from_text(stdout),
        formats,
    }
}

fn runtime_types_from_provider(provider: &ProviderDescriptor) -> Vec<RuntimeTypeRegistration> {
    provider.formats.iter()
        .filter(|extension| extension.as_str() != ".*")
        .map(|extension| RuntimeTypeRegistration {
            type_id: format!("runtime.tool.{}", extension.trim_start_matches('.')),
            label: format!("{} asset", extension.to_ascii_uppercase()),
            content_kind: extension.trim_start_matches('.').to_owned(),
            extensions: vec![extension.clone()],
            provider_id: Some(provider.id.clone()),
            capabilities: FormatTypeCapabilities {
                can_read: provider.capabilities.iter().any(|item| item == "asset.format.read"),
                can_write: provider.capabilities.iter().any(|item| item == "asset.format.write"),
                can_inspect: provider.capabilities.iter().any(|item| item == "asset.format.inspect"),
                can_validate: provider.capabilities.iter().any(|item| item == "asset.format.validate"),
                can_diff: provider.capabilities.iter().any(|item| item == "asset.format.diff"),
                can_preview: provider.capabilities.iter().any(|item| item.contains("preview")),
                can_edit_schema: provider.capabilities.iter().any(|item| item.contains("edit_schema")),
            },
            schema_id: None,
            preview_surface: preview_surface_from_capabilities(&provider.capabilities),
            viewport: viewport_from_capabilities(&provider.capabilities),
        })
        .collect()
}

fn capabilities_from_text(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut caps = vec!["asset.format.inspect".to_owned()];
    if lower.contains("read") || lower.contains("input") || lower.contains("extract") { caps.push("asset.format.read".to_owned()); }
    if lower.contains("write") || lower.contains("output") || lower.contains("pack") { caps.push("asset.format.write".to_owned()); }
    if lower.contains("valid") { caps.push("asset.format.validate".to_owned()); }
    if lower.contains("diff") { caps.push("asset.format.diff".to_owned()); }
    if lower.contains("preview") || lower.contains("texture") || lower.contains("image") || lower.contains("model") { caps.push("asset.preview.provider".to_owned()); }
    normalize_capabilities(caps)
}

fn normalize_capabilities(mut values: Vec<String>) -> Vec<String> {
    for value in &mut values {
        *value = value.trim().to_ascii_lowercase();
        while value.starts_with('.') {
            value.remove(0);
        }
    }
    values.sort();
    values.dedup();
    values
}

fn infer_formats(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    for token in ["ytd", "ydd", "ytyp", "ymap", "xml", "nepak", "neui", "nemat", "listfile", "nef8"] {
        if lower.contains(token) {
            out.push(format!(".{token}"));
        }
    }
    out
}

fn unique_sorted(mut values: Vec<String>) -> Vec<String> {
    for value in &mut values {
        if !value.starts_with('.') && value != "*" {
            *value = format!(".{value}");
        }
        *value = value.to_ascii_lowercase();
    }
    values.sort();
    values.dedup();
    values
}

fn json_string_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let idx = text.find(&needle)?;
    let after_key = &text[idx + needle.len()..];
    let colon = after_key.find(':')?;
    let mut chars = after_key[colon + 1..].chars().peekable();
    while matches!(chars.peek(), Some(ch) if ch.is_whitespace()) { chars.next(); }
    if chars.next()? != '"' { return None; }
    let mut out = String::new();
    let mut escaped = false;
    for ch in chars {
        if escaped { out.push(ch); escaped = false; continue; }
        if ch == '\\' { escaped = true; continue; }
        if ch == '"' { return Some(out); }
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
    let mut current = String::new();
    let mut escaped = false;
    for ch in array.chars() {
        if escaped { current.push(ch); escaped = false; continue; }
        match ch {
            '\\' if in_string => escaped = true,
            '"' if in_string => { in_string = false; values.push(current.clone()); current.clear(); }
            '"' => in_string = true,
            _ if in_string => current.push(ch),
            _ => {}
        }
    }
    values
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn tail(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}


pub fn routes_from_providers(providers: &[ProviderDescriptor]) -> Vec<ToolRouteDescriptor> {
    let mut routes = Vec::new();
    for provider in providers {
        if !provider.source.extension().and_then(|value| value.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("exe")) {
            continue;
        }
        for format in &provider.formats {
            let extension = normalize_extension(format);
            if extension == ".*" || extension.is_empty() {
                continue;
            }
            routes.push(ToolRouteDescriptor {
                extension,
                provider_id: provider.id.clone(),
                executable: provider.source.clone(),
                capabilities: provider.capabilities.clone(),
            });
        }
    }
    routes.sort_by(|a, b| a.extension.cmp(&b.extension).then(a.provider_id.cmp(&b.provider_id)));
    routes.dedup_by(|a, b| a.extension == b.extension && a.provider_id == b.provider_id && a.executable == b.executable);
    routes
}

pub fn run_tool_preview(route: &ToolRouteDescriptor, asset_path: &Path) -> ToolPreviewOutput {
    let attempts: &[&[&str]] = &[
        &["preview", "--input"],
        &["preview"],
        &["read", "--input"],
        &["read"],
        &["inspect", "--input"],
        &["inspect"],
        &["list", "--input"],
        &["list"],
        &["accepted-inputs"],
    ];
    let mut diagnostics = Vec::new();
    for prefix in attempts {
        let mut command = Command::new(&route.executable);
        command.args(*prefix);
        if *prefix != ["accepted-inputs"].as_slice() {
            command.arg(asset_path);
        }
        let rendered = format!("{} {} {}", route.executable.display(), prefix.join(" "), if *prefix == ["accepted-inputs"].as_slice() { String::new() } else { asset_path.display().to_string() });
        match command.stdout(Stdio::piped()).stderr(Stdio::piped()).output() {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let mut lines: Vec<String> = stdout.lines().take(800).map(ToOwned::to_owned).collect();
                if lines.is_empty() && !stderr.trim().is_empty() {
                    lines = stderr.lines().take(120).map(ToOwned::to_owned).collect();
                }
                if lines.is_empty() {
                    lines.push("Tool command succeeded but produced no preview output.".to_owned());
                }
                return ToolPreviewOutput { provider_id: route.provider_id.clone(), command: rendered, lines, diagnostics };
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                diagnostics.push(format!("{} -> exit={:?} stderr={}", rendered, output.status.code(), tail(&stderr, 4)));
            }
            Err(err) => diagnostics.push(format!("{} -> spawn failed: {err}", rendered)),
        }
    }
    ToolPreviewOutput {
        provider_id: route.provider_id.clone(),
        command: format!("{} <no working preview/read command>", route.executable.display()),
        lines: vec![
            format!("No working tool preview/read command for {}", asset_path.display()),
            format!("Bound provider: {}", route.provider_id),
            format!("Executable: {}", route.executable.display()),
            "The tool is bound to this extension, but must implement one of: preview/read/inspect/list --input <file>.".to_owned(),
        ],
        diagnostics,
    }
}

fn normalize_extension(value: &str) -> String {
    let mut extension = value.trim().to_ascii_lowercase();
    if extension == "*" || extension == ".*" {
        return ".*".to_owned();
    }
    if !extension.starts_with('.') {
        extension = format!(".{extension}");
    }
    extension
}

fn preview_surface_from_capabilities(capabilities: &[String]) -> Option<String> {
    for capability in capabilities {
        if let Some(surface) = capability.strip_prefix("asset.preview.") {
            return Some(surface.to_owned());
        }
    }
    None
}

fn viewport_from_capabilities(capabilities: &[String]) -> Option<String> {
    for capability in capabilities {
        if let Some(viewport) = capability.strip_prefix("asset.viewport.") {
            return Some(viewport.to_owned());
        }
    }
    None
}
