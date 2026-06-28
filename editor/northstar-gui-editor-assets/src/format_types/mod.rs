#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatTypeDescriptor {
    pub type_id: String,
    pub label: String,
    pub content_kind: String,
    pub extensions: Vec<String>,
    pub provider_id: Option<String>,
    pub capabilities: FormatTypeCapabilities,
    pub schema_id: Option<String>,
    pub preview_surface: Option<String>,
    pub viewport: Option<String>,
    pub source: FormatTypeSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormatTypeCapabilities {
    pub can_read: bool,
    pub can_write: bool,
    pub can_inspect: bool,
    pub can_validate: bool,
    pub can_diff: bool,
    pub can_preview: bool,
    pub can_edit_schema: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatTypeSource {
    DirectoryFile(PathBuf),
    RuntimeRegistration,
    ProviderProjection(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTypeRegistration {
    pub type_id: String,
    pub label: String,
    pub content_kind: String,
    pub extensions: Vec<String>,
    pub provider_id: Option<String>,
    pub capabilities: FormatTypeCapabilities,
    pub schema_id: Option<String>,
    pub preview_surface: Option<String>,
    pub viewport: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FormatTypeRegistry {
    descriptors: Vec<FormatTypeDescriptor>,
    by_type_id: BTreeMap<String, usize>,
    by_extension: BTreeMap<String, Vec<usize>>,
}

impl FormatTypeRegistry {
    pub fn empty() -> Self {
        Self {
            descriptors: Vec::new(),
            by_type_id: BTreeMap::new(),
            by_extension: BTreeMap::new(),
        }
    }

    pub fn load_from_roots(roots: &[PathBuf]) -> Result<Self, String> {
        let mut registry = Self::empty();
        for root in roots {
            registry.load_directory(root)?;
        }
        Ok(registry)
    }

    pub fn load_directory(&mut self, root: &Path) -> Result<(), String> {
        if !root.exists() {
            println!("[FORMAT-TYPES] skip missing root: {}", root.display());
            return Ok(());
        }

        let mut files = Vec::new();
        walk_files(root, &mut |path| {
            if is_format_type_file(path) {
                files.push(path.to_path_buf());
            }
            Ok(())
        })
        .map_err(|err| format!("failed to scan format type root {}: {err}", root.display()))?;

        for file in files {
            let text = fs::read_to_string(&file)
                .map_err(|err| format!("failed to read format type descriptor {}: {err}", file.display()))?;
            let descriptor = FormatTypeDescriptor::parse_json_like(&text, FormatTypeSource::DirectoryFile(file.clone()))?;
            self.register(descriptor)?;
        }

        Ok(())
    }

    pub fn register_runtime(&mut self, registration: RuntimeTypeRegistration) -> Result<(), String> {
        self.register(FormatTypeDescriptor {
            type_id: registration.type_id,
            label: registration.label,
            content_kind: registration.content_kind,
            extensions: registration.extensions,
            provider_id: registration.provider_id,
            capabilities: registration.capabilities,
            schema_id: registration.schema_id,
            preview_surface: registration.preview_surface,
            viewport: registration.viewport,
            source: FormatTypeSource::RuntimeRegistration,
        })
    }

    pub fn register(&mut self, mut descriptor: FormatTypeDescriptor) -> Result<(), String> {
        descriptor.type_id = descriptor.type_id.trim().to_owned();
        if descriptor.type_id.is_empty() {
            return Err("format type descriptor misses type_id".to_owned());
        }

        descriptor.extensions = descriptor
            .extensions
            .into_iter()
            .map(|extension| normalize_extension(&extension))
            .collect();
        descriptor.extensions.sort();
        descriptor.extensions.dedup();

        if let Some(existing) = self.by_type_id.get(&descriptor.type_id).copied() {
            println!(
                "[FORMAT-TYPES] replace type_id={} old_source={:?} new_source={:?}",
                descriptor.type_id,
                self.descriptors[existing].source,
                descriptor.source
            );
            self.descriptors[existing] = descriptor;
            self.rebuild_indexes();
            return Ok(());
        }

        let index = self.descriptors.len();
        self.descriptors.push(descriptor);
        self.by_type_id.insert(self.descriptors[index].type_id.clone(), index);
        for extension in &self.descriptors[index].extensions {
            self.by_extension.entry(extension.clone()).or_default().push(index);
        }
        Ok(())
    }

    pub fn descriptors(&self) -> &[FormatTypeDescriptor] {
        &self.descriptors
    }

    pub fn find_by_extension(&self, extension: &str) -> Vec<&FormatTypeDescriptor> {
        let key = normalize_extension(extension);
        self.by_extension
            .get(&key)
            .map(|indexes| indexes.iter().map(|index| &self.descriptors[*index]).collect())
            .unwrap_or_default()
    }

    pub fn providers_that_can_read(&self, extension: &str) -> Vec<String> {
        let mut providers = BTreeSet::new();
        for descriptor in self.find_by_extension(extension) {
            if descriptor.capabilities.can_read {
                if let Some(provider_id) = &descriptor.provider_id {
                    providers.insert(provider_id.clone());
                } else {
                    providers.insert(descriptor.type_id.clone());
                }
            }
        }
        providers.into_iter().collect()
    }

    pub fn capability_summary(&self) -> FormatTypeCapabilitySummary {
        let mut summary = FormatTypeCapabilitySummary::default();
        for descriptor in &self.descriptors {
            summary.total += 1;
            if descriptor.capabilities.can_read {
                summary.can_read += 1;
            }
            if descriptor.capabilities.can_write {
                summary.can_write += 1;
            }
            if descriptor.capabilities.can_preview {
                summary.can_preview += 1;
            }
            if descriptor.capabilities.can_edit_schema {
                summary.can_edit_schema += 1;
            }
            if descriptor.capabilities.can_validate {
                summary.can_validate += 1;
            }
        }
        summary
    }

    fn rebuild_indexes(&mut self) {
        self.by_type_id.clear();
        self.by_extension.clear();
        for (index, descriptor) in self.descriptors.iter().enumerate() {
            self.by_type_id.insert(descriptor.type_id.clone(), index);
            for extension in &descriptor.extensions {
                self.by_extension.entry(extension.clone()).or_default().push(index);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FormatTypeCapabilitySummary {
    pub total: usize,
    pub can_read: usize,
    pub can_write: usize,
    pub can_preview: usize,
    pub can_edit_schema: usize,
    pub can_validate: usize,
}

impl FormatTypeDescriptor {
    pub fn parse_json_like(text: &str, source: FormatTypeSource) -> Result<Self, String> {
        let type_id = json_string_field(text, "type_id")
            .or_else(|| json_string_field(text, "id"))
            .ok_or_else(|| "format type descriptor misses type_id/id".to_owned())?;
        let label = json_string_field(text, "label").unwrap_or_else(|| type_id.clone());
        let content_kind = json_string_field(text, "content_kind").unwrap_or_else(|| "unknown".to_owned());
        let extensions = json_string_array(text, "extensions");
        let provider_id = json_string_field(text, "provider_id")
            .or_else(|| json_string_field(text, "provider"));
        let schema_id = json_string_field(text, "schema_id");
        let preview_surface = json_string_field(text, "preview_surface");
        let viewport = json_string_field(text, "viewport");

        Ok(Self {
            type_id,
            label,
            content_kind,
            extensions,
            provider_id,
            capabilities: FormatTypeCapabilities {
                can_read: json_bool_field(text, "can_read"),
                can_write: json_bool_field(text, "can_write"),
                can_inspect: json_bool_field(text, "can_inspect"),
                can_validate: json_bool_field(text, "can_validate"),
                can_diff: json_bool_field(text, "can_diff"),
                can_preview: json_bool_field(text, "can_preview"),
                can_edit_schema: json_bool_field(text, "can_edit_schema"),
            },
            schema_id,
            preview_surface,
            viewport,
            source,
        })
    }
}

fn is_format_type_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    name == "format_type.json" || name.ends_with(".format_type.json") || name.ends_with(".asset_type.json")
}

fn normalize_extension(extension: &str) -> String {
    let trimmed = extension.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return trimmed;
    }
    if trimmed.starts_with('.') {
        trimmed
    } else {
        format!(".{trimmed}")
    }
}

fn walk_files<F>(root: &Path, f: &mut F) -> io::Result<()>
where
    F: FnMut(&Path) -> io::Result<()>,
{
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(name.as_str(), ".git" | ".takesome" | "target" | "node_modules" | "cache" | "logs") {
                continue;
            }
            walk_files(&path, f)?;
        } else {
            f(&path)?;
        }
    }
    Ok(())
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

fn json_bool_field(text: &str, key: &str) -> bool {
    let needle = format!("\"{key}\"");
    let Some(idx) = text.find(&needle) else { return false; };
    let after_key = &text[idx + needle.len()..];
    let Some(colon) = after_key.find(':') else { return false; };
    after_key[colon + 1..].trim_start().starts_with("true")
}
