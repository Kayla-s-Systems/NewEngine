#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use abi_stable::std_types::RVec;
use newengine_assets::{wait_ready, AssetAccess, AssetService, AssetServiceClient};
use newengine_platform_api::PlatformAppIconV1;

pub fn collect_app_asset_roots(app_dir_name: &str, env_var: &str) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();

    if let Ok(p) = std::env::var(env_var) {
        roots.push(PathBuf::from(p));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("assets"));
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(Path::to_path_buf);
        for _ in 0..6 {
            let Some(base) = cur.clone() else {
                break;
            };

            let shared_assets = base.join("assets");
            if shared_assets.is_dir() {
                roots.push(shared_assets);
            }

            let app_assets = base.join("apps").join(app_dir_name).join("assets");
            if app_assets.is_dir() {
                roots.push(app_assets);
                break;
            }

            cur = base.parent().map(Path::to_path_buf);
        }
    }

    let mut out: Vec<PathBuf> = Vec::new();
    let mut dedup: HashSet<PathBuf> = HashSet::new();
    for root in roots {
        if dedup.insert(root.clone()) {
            out.push(root);
        }
    }
    out
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
    if let Err(e) = assets.mount_dir(&path_string) {
        log::warn!(
            "runtime host: asset.mount_dir failed path='{}' err='{}'",
            path.display(),
            e
        );
    }
}

pub fn try_load_window_icon_best_effort(
    icon_path: Option<&str>,
    assets: Option<&AssetServiceClient>,
    roots: &[PathBuf],
) -> Option<PlatformAppIconV1> {
    let Some(path) = icon_path else {
        return None;
    };

    if let Some(assets) = assets {
        let id_hex32 = match assets.load(path) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("window icon: asset.load failed path='{path}' err='{e}'");
                return None;
            }
        };

        if let Err(e) = wait_ready(assets, &id_hex32, Duration::from_millis(500)) {
            log::warn!("window icon: wait_ready failed path='{path}' err='{e:?}'");
            return None;
        }

        let (_meta_json, payload) = match assets.blob_wire_v1(&id_hex32) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("window icon: blob_wire_v1 failed path='{path}' err='{e}'");
                return None;
            }
        };

        return decode_window_icon(&payload, path);
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    let icon_path = PathBuf::from(path);

    if icon_path.is_absolute() {
        candidates.push(icon_path);
    } else {
        candidates.push(PathBuf::from(path));
        for root in roots {
            candidates.push(root.join(path));
        }
    }

    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }

        match std::fs::read(&candidate) {
            Ok(bytes) => {
                if let Some(icon) = decode_window_icon(&bytes, &candidate.to_string_lossy()) {
                    return Some(icon);
                }
            }
            Err(e) => {
                log::warn!(
                    "window icon: read failed file='{}' err='{}'",
                    candidate.display(),
                    e
                );
            }
        }
    }

    None
}

fn decode_window_icon(bytes: &[u8], label: &str) -> Option<PlatformAppIconV1> {
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            Some(PlatformAppIconV1 {
                rgba: RVec::from(rgba.into_raw()),
                width,
                height,
            })
        }
        Err(e) => {
            log::warn!("window icon: decode failed path='{}' err='{}'", label, e);
            None
        }
    }
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
