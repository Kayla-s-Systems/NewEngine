use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use newengine_assets::{AssetService, AssetServiceClient};
#[cfg(test)]
use newengine_project_api::PROJECT_MANIFEST_FILE;
use newengine_project_api::{ContentMountDescriptor, ContentMountNamespace, ContentMountRegistry};

use crate::{ProjectPaths, PROJECT_SOURCE_DIR};

pub fn register_engine_bootstrap_asset_roots(
    registry: &mut ContentMountRegistry,
    roots: &[PathBuf],
) -> Result<(), String> {
    for (index, root) in roots.iter().enumerate() {
        registry.register(ContentMountDescriptor {
            id: format!("engine.bootstrap.{index}"),
            namespace: ContentMountNamespace::Engine,
            root: root.clone(),
            mount: "engine".to_owned(),
            priority: ContentMountNamespace::Engine.default_priority() - index as i32,
            writable: false,
            required: false,
            owner: "runtime-host.bootstrap".to_owned(),
        })?;
    }
    Ok(())
}

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

fn collect_build_plan_aliases(
    value: &serde_json::Value,
    aliases: &mut BTreeMap<String, String>,
    path: &str,
) -> Result<(), String> {
    match value {
        serde_json::Value::Object(object) => {
            let source = object
                .get("source_dictionary")
                .or_else(|| object.get("source"))
                .or_else(|| object.get("source_dir"))
                .and_then(|value| value.as_str());
            let output = object.get("output").and_then(|value| value.as_str());
            if let (Some(source), Some(output)) = (source, output) {
                let logical_path = object
                    .get("logical_path")
                    .and_then(|value| value.as_str())
                    .and_then(normalize_project_relative_path)
                    .ok_or_else(|| {
                        format!(
                            "build plan entry '{path}' source='{source}' output='{output}' must author explicit logical_path"
                        )
                    })?;
                let source_path = normalize_project_relative_path(source).ok_or_else(|| {
                    format!("build plan entry '{path}' has unsafe source path '{source}'")
                })?;
                if source_path
                    .split('/')
                    .next()
                    .is_some_and(|root| root.eq_ignore_ascii_case("Source"))
                {
                    if let Some(previous) =
                        aliases.insert(logical_path.clone(), source_path.clone())
                    {
                        if previous != source_path {
                            return Err(format!(
                                "build plan logical_path collision '{logical_path}': '{previous}' vs '{source_path}'"
                            ));
                        }
                    }
                }
            }
            for (key, child) in object {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                collect_build_plan_aliases(child, aliases, &child_path)?;
            }
        }
        serde_json::Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                let child_path = format!("{path}[{index}]");
                collect_build_plan_aliases(child, aliases, &child_path)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn discover_project_source_aliases(project_root: &Path) -> Result<Vec<(String, String)>, String> {
    let plan_path = ProjectPaths::new(project_root.to_path_buf()).asset_build_plan_path();
    if !plan_path.is_file() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(&plan_path)
        .map_err(|error| format!("read '{}': {error}", plan_path.display()))?;
    let plan: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse '{}': {error}", plan_path.display()))?;
    let mut aliases = BTreeMap::new();
    collect_build_plan_aliases(&plan, &mut aliases, "root")?;
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
            let source_fallback_root = [
                Some(mount.root.clone()),
                mount.root.parent().map(Path::to_path_buf),
            ]
            .into_iter()
            .flatten()
            .find(|candidate| {
                candidate.join(PROJECT_SOURCE_DIR).is_dir()
                    && ProjectPaths::new(candidate.clone())
                        .asset_build_plan_path()
                        .is_file()
            });
            if let Some(project_root) = source_fallback_root {
                diagnostics.push(format!(
                    "FALLBACK: {message}; source_root='{}' policy={} action='compiled root unavailable; project Source remains eligible'",
                    project_root.display(),
                    newengine_assets::ASSET_RESOLUTION_POLICY_COMPILED_FIRST_SOURCE_FALLBACK_V1,
                ));
            } else if mount.required {
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

    // Source companions are discovered from the selected project only. The build plan
    // owns the runtime namespace explicitly: every build entry must author logical_path.
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
    fn build_plan_aliases_are_project_defined_when_logical_paths_exist() {
        let plan = serde_json::json!({
            "models": [{
                "source": "Source/models/hero.glb",
                "output": "Anything/cache/blob.bin",
                "logical_path": "avatars/main/body"
            }],
            "definitions": [{
                "source": "Source/definitions/hero.ytyp.xml",
                "output": "Build/out/definition.bin",
                "logical_path": "gameplay/characters/hero"
            }],
            "scripts": [{
                "source": "Source/scripts/game.lua",
                "output": "Generated/module.bin",
                "logical_path": "logic/bootstrap"
            }]
        });
        let mut aliases = BTreeMap::new();
        collect_build_plan_aliases(&plan, &mut aliases, "root").unwrap();
        assert_eq!(
            aliases.get("avatars/main/body").map(String::as_str),
            Some("Source/models/hero.glb")
        );
        assert_eq!(
            aliases.get("gameplay/characters/hero").map(String::as_str),
            Some("Source/definitions/hero.ytyp.xml")
        );
        assert_eq!(
            aliases.get("logic/bootstrap").map(String::as_str),
            Some("Source/scripts/game.lua")
        );
        assert_eq!(aliases.len(), 3);
    }

    #[test]
    fn build_plan_entry_without_logical_path_is_rejected() {
        let plan = serde_json::json!({
            "explicit": {
                "source": "Source/a.custom",
                "output": "Content/a.ydd",
                "logical_path": "project/a"
            },
            "implicit": {
                "source": "Source/b.custom",
                "output": "Content/b.ydd"
            }
        });
        let mut aliases = BTreeMap::new();
        let error = collect_build_plan_aliases(&plan, &mut aliases, "root").unwrap_err();
        assert!(error.contains("must author explicit logical_path"));
    }

    #[test]
    fn duplicate_logical_path_with_different_sources_is_rejected() {
        let plan = serde_json::json!({
            "a": {
                "source": "Source/a.custom",
                "output": "Content/a.ydd",
                "logical_path": "models/shared.ydd"
            },
            "b": {
                "source": "Source/b.custom",
                "output": "Content/b.ydd",
                "logical_path": "models/shared.ydd"
            }
        });
        let mut aliases = BTreeMap::new();
        let error = collect_build_plan_aliases(&plan, &mut aliases, "root").unwrap_err();
        assert!(error.contains("logical_path collision"));
    }
}
