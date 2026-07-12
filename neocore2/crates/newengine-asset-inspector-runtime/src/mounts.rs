use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use newengine_assets::{AssetService, AssetServiceClient};

use crate::ASSET_INSPECTOR_ASSETS_ENV;

pub(crate) fn mount_asset_roots(client: &AssetServiceClient) -> Result<Vec<PathBuf>, String> {
    let roots = collect_asset_roots();
    if roots.is_empty() {
        return Err(format!(
            "no gameAssets root found; set {}",
            ASSET_INSPECTOR_ASSETS_ENV
        ));
    }
    let mut mounted = Vec::new();
    let mut errors = Vec::new();
    for root in roots {
        let root_text = root.to_string_lossy().to_string();
        match client.mount_source_json_v1(serde_json::json!({
            "kind": "filesystem",
            "priority": 260,
            "mount": "",
            "config": { "root": root_text }
        })) {
            Ok(_) => mounted.push(root),
            Err(error) => errors.push(format!("{}: {error}", root.display())),
        }
    }
    if mounted.is_empty() {
        Err(format!(
            "engine.assets rejected every gameAssets root: {}",
            errors.join(" | ")
        ))
    } else {
        Ok(mounted)
    }
}

fn collect_asset_roots() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(value) = std::env::var(ASSET_INSPECTOR_ASSETS_ENV) {
        let value = value.trim();
        if !value.is_empty() {
            candidates.push(PathBuf::from(value));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        push_ancestor_candidates(&mut candidates, &cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            push_ancestor_candidates(&mut candidates, dir);
        }
    }

    let mut dedup = BTreeSet::new();
    candidates
        .into_iter()
        .filter_map(|candidate| candidate.canonicalize().ok())
        .filter(|candidate| candidate.is_dir())
        .filter(|candidate| dedup.insert(candidate.clone()))
        .collect()
}

fn push_ancestor_candidates(out: &mut Vec<PathBuf>, start: &Path) {
    let mut current = Some(start.to_path_buf());
    for _ in 0..10 {
        let Some(base) = current else {
            break;
        };
        let direct = base.join("gameAssets");
        if direct.is_dir() {
            out.push(direct);
        }
        current = base.parent().map(Path::to_path_buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancestor_probe_is_bounded_and_deterministic() {
        let mut paths = Vec::new();
        push_ancestor_candidates(&mut paths, Path::new("C:/missing/a/b"));
        assert!(paths.is_empty());
    }
}
