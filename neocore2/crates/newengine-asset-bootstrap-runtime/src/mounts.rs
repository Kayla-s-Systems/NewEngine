use crate::roots::{collect_content_set_roots, ProfileMountSpec};
use newengine_assets::{AssetService, AssetServiceClient};
use newengine_math::collections_prelude::NeHashSet as HashSet;
use std::path::{Path, PathBuf};

pub fn mount_profile_content_best_effort(assets: &AssetServiceClient, spec: ProfileMountSpec) {
    let mut mounted: HashSet<PathBuf> = HashSet::default();
    for content in spec.content_sets {
        for root in collect_content_set_roots(*content) {
            let identity = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
            if !mounted.insert(identity) {
                continue;
            }
            try_mount_with_policy(assets, &root, content.priority, content.mount, content.id);
        }
    }
}

pub fn mount_asset_roots_best_effort(assets: &AssetServiceClient, roots: &[PathBuf]) {
    let mut mounted: HashSet<PathBuf> = HashSet::default();
    for root in roots {
        let identity = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        if mounted.insert(identity) {
            try_mount_with_policy(assets, root, 200, "", "engine-app-roots");
        }
    }
}

fn try_mount_with_policy(
    assets: &AssetServiceClient,
    path: &Path,
    priority: i32,
    mount: &str,
    content_set: &str,
) {
    if !path.is_dir() {
        return;
    }

    let path_string = path.to_string_lossy().to_string();
    if let Err(error) = assets.mount_source_json_v1(serde_json::json!({
        "kind": "filesystem",
        "priority": priority,
        "mount": mount,
        "asset_role": newengine_assets::asset_source_role::COMPILED,
        "config": { "root": path_string }
    })) {
        newengine_ulog_api::ulog::warn!(
            "runtime host: asset.mount_source_json_v1(dir) failed content_set='{}' path='{}' err='{}'",
            content_set,
            path.display(),
            error
        );
    }
}
