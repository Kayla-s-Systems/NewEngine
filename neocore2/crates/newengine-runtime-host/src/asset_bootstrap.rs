#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections_prelude::NeHashSet as HashSet;
use std::path::{Path, PathBuf};

#[cfg(feature = "window-icon")]
use std::time::Duration;
#[cfg(feature = "window-icon")]
use abi_stable::std_types::RVec;
#[cfg(feature = "window-icon")]
use newengine_assets::{wait_ready, AssetAccess};
use newengine_assets::{AssetService, AssetServiceClient};
use newengine_platform_api::PlatformAppIconV1;

pub fn collect_app_asset_roots(app_dir_name: &str, env_var: &str) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();

    if let Ok(p) = std::env::var(env_var) {
        roots.push(PathBuf::from(p));
    }

    // Cargo --manifest-path does not guarantee that the process CWD is the
    // workspace root. Resolve both executable-relative and CWD-relative asset
    // roots so VFS mount discovery is deterministic during editor/dev runs.
    if let Ok(cwd) = std::env::current_dir() {
        push_asset_roots_from_base(&mut roots, &cwd, app_dir_name);
        let mut cur = Some(cwd);
        for _ in 0..8 {
            let Some(base) = cur.clone() else { break; };
            push_asset_roots_from_base(&mut roots, &base, app_dir_name);
            cur = base.parent().map(Path::to_path_buf);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("assets"));
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(Path::to_path_buf);
        for _ in 0..8 {
            let Some(base) = cur.clone() else {
                break;
            };

            push_asset_roots_from_base(&mut roots, &base, app_dir_name);

            cur = base.parent().map(Path::to_path_buf);
        }
    }

    let mut out: Vec<PathBuf> = Vec::new();
    let mut dedup: HashSet<PathBuf> = HashSet::default();
    for root in roots {
        if dedup.insert(root.clone()) {
            out.push(root);
        }
    }
    out
}

fn push_asset_roots_from_base(roots: &mut Vec<PathBuf>, base: &Path, app_dir_name: &str) {
    let shared_assets = base.join("assets");
    if shared_assets.is_dir() {
        roots.push(shared_assets);
    }

    let app_assets = base.join("apps").join(app_dir_name).join("assets");
    if app_assets.is_dir() {
        roots.push(app_assets);
    }

    // Canonical outer-root runtime layout: commands may run from the outer
    // NorthStar directory, from NewEngine, or from an app workspace while game
    // assets live at NorthStar/gameAssets. This is still a VFS mount, not a
    // direct asset read.
    let game_assets = base.join("gameAssets");
    if game_assets.is_dir() {
        roots.push(game_assets);
    }

    // Legacy local layouts kept as explicit compatibility candidates for old
    // source snapshots, not as the preferred runtime layout.
    let legacy_newengine_assets = base.join("NewEngine").join("assets");
    if legacy_newengine_assets.is_dir() {
        roots.push(legacy_newengine_assets);
    }

    let legacy_neocore_assets = base.join("NewEngine").join("neocore2").join("assets");
    if legacy_neocore_assets.is_dir() {
        roots.push(legacy_neocore_assets);
    }
}


pub fn mount_asset_roots_best_effort(assets: &AssetServiceClient, roots: &[PathBuf]) {
    for root in roots {
        try_mount(assets, root);
    }
}

fn try_mount(assets: &AssetServiceClient, path: &Path) {
    if !path.is_dir() {
        return;
    }

    let path_string = path.to_string_lossy().to_string();
    if let Err(e) = assets.mount_source_json_v1(serde_json::json!({
        "kind": "filesystem",
        "priority": 200,
        "mount": "",
        "config": { "root": path_string }
    })) {
        newengine_ulog_api::ulog::warn!(
            "runtime host: asset.mount_source_json_v1(dir) failed path='{}' err='{}'",
            path.display(),
            e
        );
    }
}

#[cfg(feature = "window-icon")]
pub fn try_load_window_icon_best_effort(
    icon_path: Option<&str>,
    assets: Option<&AssetServiceClient>,
    roots: &[PathBuf],
) -> Option<PlatformAppIconV1> {
    let _ = roots;
    let Some(path) = icon_path else {
        return None;
    };

    let Some(assets) = assets else {
        newengine_ulog_api::ulog::info!(
            "window icon: AssetManager unavailable; skipping icon path='{}' because runtime assets must not be read directly from filesystem",
            path
        );
        return None;
    };

    let id_hex32 = match assets.import_v1(path) {
        Ok(v) => v,
        Err(e) => {
            newengine_ulog_api::ulog::warn!("window icon: asset.import_v1 failed path='{path}' err='{e}'");
            return None;
        }
    };

    if let Err(e) = wait_ready(assets, &id_hex32, Duration::from_millis(500)) {
        newengine_ulog_api::ulog::warn!("window icon: wait_ready failed path='{path}' id='{id_hex32}' err='{e:?}'");
        return None;
    }

    let texture = match assets.texture_rgba8_v1(&id_hex32) {
        Ok(v) => v,
        Err(e) => {
            newengine_ulog_api::ulog::warn!("window icon: texture_rgba8_v1 failed path='{path}' id='{id_hex32}' err='{e}'");
            return None;
        }
    };

    Some(PlatformAppIconV1 {
        rgba: RVec::from(texture.rgba),
        width: texture.width,
        height: texture.height,
    })
}

#[cfg(not(feature = "window-icon"))]
pub fn try_load_window_icon_best_effort(
    icon_path: Option<&str>,
    assets: Option<&AssetServiceClient>,
    roots: &[PathBuf],
) -> Option<PlatformAppIconV1> {
    let _ = icon_path;
    let _ = assets;
    let _ = roots;
    None
}


#[inline]
pub fn shard_log_path_by_run_id(original: &str, run_id: &str) -> Option<String> {
    let source = original.trim();
    if source.is_empty() {
        return None;
    }

    let path = Path::new(source);
    let parent = path.parent();
    let file_name = path.file_name()?.to_string_lossy();
    let (stem, ext) = match (path.file_stem(), path.extension()) {
        (Some(stem), Some(ext)) => (stem.to_string_lossy(), Some(ext.to_string_lossy())),
        (Some(stem), None) => (stem.to_string_lossy(), None),
        _ => return None,
    };

    let new_file = match ext.as_deref() {
        Some("log") => format!("{stem}.{run_id}.log"),
        Some(ext) if !ext.is_empty() => format!("{stem}.{run_id}.{ext}"),
        _ => format!("{file_name}.{run_id}.log"),
    };

    Some(
        parent
            .map(|dir| dir.join(&new_file).to_string_lossy().to_string())
            .unwrap_or(new_file),
    )
}
