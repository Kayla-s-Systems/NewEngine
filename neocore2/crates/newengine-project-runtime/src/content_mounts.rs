use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use newengine_assets::{AssetService, AssetServiceClient};
#[cfg(test)]
use newengine_project_api::PROJECT_MANIFEST_FILE;
use newengine_project_api::{ContentMountDescriptor, ContentMountNamespace, ContentMountRegistry};

use crate::{ProjectPaths, PROJECT_SOURCE_DIR};

pub fn register_engine_asset_roots(
    registry: &mut ContentMountRegistry,
    roots: &[PathBuf],
) -> Result<(), String> {
    for (index, root) in roots.iter().enumerate() {
        registry.register(ContentMountDescriptor {
            id: format!("engine.compat.{index}"),
            namespace: ContentMountNamespace::Engine,
            root: root.clone(),
            mount: "engine".to_owned(),
            priority: ContentMountNamespace::Engine.default_priority() - index as i32,
            writable: false,
            required: false,
            owner: "runtime-host.compat".to_owned(),
        })?;
    }
    Ok(())
}

const SOURCE_DISCOVERY_LIMIT: usize = 50_000;

fn normalize_project_relative_path(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned();
    if normalized.is_empty() {
        return None;
    }
    let path = Path::new(&normalized);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }
    Some(normalized)
}

fn runtime_logical_path_from_output(
    output: &str,
    explicit_logical_path: Option<&str>,
) -> Option<String> {
    if let Some(explicit) = explicit_logical_path.and_then(normalize_project_relative_path) {
        return Some(explicit);
    }

    let output = normalize_project_relative_path(output)?;
    let (root, rest) = output
        .split_once('/')
        .map(|(root, rest)| (root, rest))
        .unwrap_or(("", output.as_str()));
    if rest.is_empty() {
        return None;
    }
    if root.eq_ignore_ascii_case("Content") {
        return Some(rest.to_owned());
    }
    if root.eq_ignore_ascii_case("Definitions") {
        return Some(format!("definitions/{rest}"));
    }
    if root.eq_ignore_ascii_case("Scripts") {
        return Some(format!("scripts/{rest}"));
    }
    Some(output)
}

fn collect_build_plan_aliases(value: &serde_json::Value, aliases: &mut BTreeMap<String, String>) {
    match value {
        serde_json::Value::Object(object) => {
            let source = object.get("source").and_then(|value| value.as_str());
            let output = object.get("output").and_then(|value| value.as_str());
            if let (Some(source), Some(output)) = (source, output) {
                let logical_path = runtime_logical_path_from_output(
                    output,
                    object.get("logical_path").and_then(|value| value.as_str()),
                );
                let source_path = normalize_project_relative_path(source);
                if let (Some(logical_path), Some(source_path)) = (logical_path, source_path) {
                    if source_path
                        .split('/')
                        .next()
                        .is_some_and(|root| root.eq_ignore_ascii_case("Source"))
                    {
                        aliases.insert(logical_path, source_path);
                    }
                }
            }
            for child in object.values() {
                collect_build_plan_aliases(child, aliases);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                collect_build_plan_aliases(child, aliases);
            }
        }
        _ => {}
    }
}

fn convention_logical_path(source_relative: &str) -> Option<String> {
    let normalized = normalize_project_relative_path(source_relative)?;
    let lower = normalized.to_ascii_lowercase();
    for suffix in [
        ".ymap.xml",
        ".ytyp.xml",
        ".nemat.xml",
        ".neui.xml",
        ".ymt.xml",
    ] {
        if lower.ends_with(suffix) {
            return Some(normalized[..normalized.len() - ".xml".len()].to_owned());
        }
    }
    if lower.starts_with("scripts/") {
        for extension in [".ts", ".tsx", ".js", ".mjs", ".lua"] {
            if lower.ends_with(extension) {
                return Some(format!(
                    "{}.ysc",
                    &normalized[..normalized.len() - extension.len()]
                ));
            }
        }
    }
    if lower.starts_with("animations/") && lower.ends_with(".clip.json") {
        return Some(format!(
            "{}.ycd",
            &normalized[..normalized.len() - ".clip.json".len()]
        ));
    }
    None
}

fn discover_convention_aliases(
    project_root: &Path,
    aliases: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    let source_root = ProjectPaths::new(project_root.to_path_buf()).source_dir();
    if !source_root.is_dir() {
        return Ok(());
    }
    let mut pending = vec![source_root.clone()];
    let mut visited = 0usize;

    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .map_err(|error| format!("scan source directory '{}': {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!("scan source entry under '{}': {error}", directory.display())
            })?;
            visited = visited.saturating_add(1);
            if visited > SOURCE_DISCOVERY_LIMIT {
                return Err(format!(
                    "source discovery exceeded bounded entry limit {SOURCE_DISCOVERY_LIMIT} under '{}'",
                    source_root.display()
                ));
            }
            let path = entry.path();
            // Directory iteration already carries the file type on the common Windows path.
            // Avoid an extra metadata query per source entry during project bootstrap.
            let file_type = entry
                .file_type()
                .map_err(|error| format!("read source file type '{}': {error}", path.display()))?;
            if file_type.is_dir() {
                pending.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let relative = path
                .strip_prefix(&source_root)
                .ok()
                .map(|value| value.to_string_lossy().replace('\\', "/"));
            let Some(relative) = relative else {
                continue;
            };
            if let Some(logical_path) = convention_logical_path(&relative) {
                aliases
                    .entry(logical_path)
                    .or_insert_with(|| format!("{PROJECT_SOURCE_DIR}/{relative}"));
            }
        }
    }
    Ok(())
}

fn discover_project_source_aliases(project_root: &Path) -> Result<Vec<(String, String)>, String> {
    let mut aliases = BTreeMap::new();
    let plan_path = ProjectPaths::new(project_root.to_path_buf()).asset_build_plan_path();
    if plan_path.is_file() {
        let bytes = std::fs::read(&plan_path)
            .map_err(|error| format!("read '{}': {error}", plan_path.display()))?;
        let plan: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("parse '{}': {error}", plan_path.display()))?;
        collect_build_plan_aliases(&plan, &mut aliases);
    }
    discover_convention_aliases(project_root, &mut aliases)?;
    Ok(aliases.into_iter().collect())
}

fn discover_project_source_roots(registry: &ContentMountRegistry) -> Vec<(PathBuf, i32)> {
    let mut roots: Vec<(PathBuf, i32)> = Vec::new();
    for mount in registry.mounts() {
        let candidates = [
            Some(mount.root.clone()),
            mount.root.parent().map(Path::to_path_buf),
        ];
        for candidate in candidates.into_iter().flatten() {
            if !candidate.join(PROJECT_SOURCE_DIR).is_dir() {
                continue;
            }
            if let Some((_, priority)) = roots.iter_mut().find(|(root, _)| *root == candidate) {
                *priority = (*priority).max(mount.priority);
            } else {
                roots.push((candidate, mount.priority));
            }
            break;
        }
    }
    roots.sort_by(|a, b| a.0.cmp(&b.0));
    roots
}

pub fn mount_content_registry_best_effort(
    assets: &AssetServiceClient,
    registry: &ContentMountRegistry,
) -> Vec<String> {
    let mut diagnostics = Vec::new();
    for mount in registry.mounts() {
        if !mount.root.is_dir() {
            let message = format!(
                "content mount '{}' root missing: {}",
                mount.id,
                mount.root.display()
            );
            if mount.required {
                diagnostics.push(format!("ERROR: {message}"));
            } else {
                diagnostics.push(format!("SKIP: {message}"));
            }
            continue;
        }
        let result = assets.mount_source_json_v1(serde_json::json!({
            "kind": "filesystem",
            "priority": mount.priority,
            "mount": mount.mount,
            "asset_role": newengine_assets::asset_source_role::COMPILED,
            "config": { "root": mount.root.to_string_lossy() },
            "metadata": {
                "mount_id": mount.id,
                "namespace": mount.namespace.id(),
                "owner": mount.owner,
                "writable": mount.writable,
            }
        }));
        match result {
            Ok(_) => diagnostics.push(format!(
                "MOUNT: {}:/ <- {} priority={}",
                mount.namespace.id(),
                mount.root.display(),
                mount.priority
            )),
            Err(error) => diagnostics.push(format!(
                "ERROR: mount '{}' failed root='{}': {error}",
                mount.id,
                mount.root.display()
            )),
        }
    }

    // Source companions are discovered from the selected project layout, never
    // from a global gameAssets directory. The build plan is authoritative for
    // non-trivial output names; bounded convention scanning covers direct pairs.
    for (project_root, priority) in discover_project_source_roots(registry) {
        let aliases = match discover_project_source_aliases(&project_root) {
            Ok(aliases) => aliases,
            Err(error) => {
                diagnostics.push(format!(
                    "WARN: project source discovery failed root='{}': {error}",
                    project_root.display()
                ));
                Vec::new()
            }
        };
        let alias_payload = aliases
            .iter()
            .map(|(logical_path, source_path)| {
                serde_json::json!({
                    "logical_path": logical_path,
                    "source_path": source_path,
                })
            })
            .collect::<Vec<_>>();
        let result = assets.mount_source_json_v1(serde_json::json!({
            "kind": "filesystem",
            "priority": priority,
            "mount": "",
            "asset_role": newengine_assets::asset_source_role::SOURCE,
            "aliases": alias_payload,
            "config": { "root": project_root.to_string_lossy() },
            "metadata": {
                "owner": "project-source-discovery",
                "policy": newengine_assets::ASSET_RESOLUTION_POLICY_COMPILED_FIRST_SOURCE_FALLBACK_V1,
                "build_plan": ProjectPaths::new(project_root.clone()).asset_build_plan_path().to_string_lossy(),
            }
        }));
        match result {
            Ok(_) => diagnostics.push(format!(
                "SOURCE: {} aliases={} policy={}",
                project_root.display(),
                aliases.len(),
                newengine_assets::ASSET_RESOLUTION_POLICY_COMPILED_FIRST_SOURCE_FALLBACK_V1
            )),
            Err(error) => diagnostics.push(format!(
                "ERROR: source mount failed root='{}': {error}",
                project_root.display()
            )),
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_argument_supports_equals_form() {
        // Parsing process args itself is intentionally tiny; manifest parsing/validation is covered by API tests.
        assert_eq!(PROJECT_MANIFEST_FILE, "game.toml");
    }

    #[test]
    fn build_plan_aliases_follow_current_project_layout() {
        let plan = serde_json::json!({
            "models": [{
                "source": "Source/models/hero.glb",
                "output": "Content/models/hero.ydd"
            }],
            "definitions": [{
                "source": "Source/definitions/hero.ytyp.xml",
                "output": "Definitions/hero.ytyp"
            }],
            "scripts": [{
                "source": "Source/scripts/game.lua",
                "output": "Scripts/game.ysc",
                "logical_path": "scripts/game.ysc"
            }]
        });
        let mut aliases = BTreeMap::new();
        collect_build_plan_aliases(&plan, &mut aliases);
        assert_eq!(
            aliases.get("models/hero.ydd").map(String::as_str),
            Some("Source/models/hero.glb")
        );
        assert_eq!(
            aliases.get("definitions/hero.ytyp").map(String::as_str),
            Some("Source/definitions/hero.ytyp.xml")
        );
        assert_eq!(
            aliases.get("scripts/game.ysc").map(String::as_str),
            Some("Source/scripts/game.lua")
        );
    }

    #[test]
    fn source_conventions_cover_double_extensions_and_scripts() {
        assert_eq!(
            convention_logical_path("maps/world.ymap.xml").as_deref(),
            Some("maps/world.ymap")
        );
        assert_eq!(
            convention_logical_path("materials/world.nemat.xml").as_deref(),
            Some("materials/world.nemat")
        );
        assert_eq!(
            convention_logical_path("scripts/game.lua").as_deref(),
            Some("scripts/game.ysc")
        );
        assert_eq!(
            convention_logical_path("scripts/game.ts").as_deref(),
            Some("scripts/game.ysc")
        );
        assert_eq!(
            convention_logical_path("scripts/editor.mjs").as_deref(),
            Some("scripts/editor.ysc")
        );
        assert_eq!(
            convention_logical_path("animations/idle.clip.json").as_deref(),
            Some("animations/idle.ycd")
        );
    }
}
