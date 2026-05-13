#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_materials::api::{MaterialRegistryApi, MaterialTextureBindings};
use newengine_materials::binary::{
    decode_asset as decode_material_asset, encode_asset as encode_material_asset, MaterialBinaryAsset,
};
use newengine_materials::serde as mat_serde;
use newengine_materials::{parse_material_source_json, MaterialDescriptor, MaterialRegistry};
use newengine_assets::{AssetAccess, AssetServiceClient};

use parking_lot::RwLock;
use std::sync::Arc;

use newengine_math::collections_prelude::{NeHashMap as HashMap, NeHashSet as HashSet};
use std::path::{Path, PathBuf};

use newengine_plugin_host::default_host_api;
use newengine_runtime_host::asset_bootstrap::collect_app_asset_roots;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Editor-side material pipeline (dev-friendly, deterministic).
///
/// Responsibilities:
/// - Scan `assets/materials` roots for source materials.
/// - Compile `.json` to cached `.nemat` when content changes.
/// - Load `.nemat` into the in-memory `MaterialRegistry`.
/// - Remove stale `materials/*` entries when sources disappear.
///
/// Non-goals:
/// - Background threads.
/// - File watchers (polling only).
/// - Texture binding / graph compilation (layered later).
pub struct MaterialPipeline {
    scan_every: Duration,
    last_scan: Instant,

    roots: Vec<PathBuf>,
    cache_dir: PathBuf,

    seen: HashMap<PathBuf, u128>,
}

impl Default for MaterialPipeline {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl MaterialPipeline {
    pub fn new() -> Self {
        Self {
            scan_every: Duration::from_millis(750),
            last_scan: Instant::now() - Duration::from_secs(5),
            roots: collect_app_asset_roots(crate::EDITOR_APP_DIR_NAME, crate::EDITOR_APP_ASSETS_DIR_ENV),
            cache_dir: PathBuf::from("cache").join("materials"),
            seen: HashMap::default(),
        }
    }

    /// Polls for material source changes and updates the registry.
    pub fn pump(&mut self, reg: &Arc<RwLock<MaterialRegistry>>) {
        if self.last_scan.elapsed() < self.scan_every {
            return;
        }
        self.last_scan = Instant::now();

        if self.roots.is_empty() {
            self.roots = collect_app_asset_roots(crate::EDITOR_APP_DIR_NAME, crate::EDITOR_APP_ASSETS_DIR_ENV);
        }

        let mut compiled: Vec<(String, MaterialDescriptor, MaterialTextureBindings)> = Vec::new();
        let mut live_names: HashSet<String> = HashSet::default();

        // Avoid borrowing `self.roots` across the scan loop while we need `&mut self` inside.
        let roots = self.roots.clone();

        for root in roots {
            let dir = root.join("materials");
            if !dir.is_dir() {
                continue;
            }

            let mut files: Vec<PathBuf> = Vec::new();
            Self::collect_files(&dir, &mut files);

            // Stable order improves reproducibility of "last write wins" behavior.
            files.sort_unstable_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));

            for p in files {
                let Some(ext) = p.extension().and_then(|e| e.to_str()) else { continue };

                if ext.eq_ignore_ascii_case("json") {
                    if let Some((name, desc, textures)) = self.load_or_compile_json(&root, &p) {
                        live_names.insert(name.clone());
                        compiled.push((name, desc, textures));
                    }
                } else if ext.eq_ignore_ascii_case("nemat") {
                    if let Some((name, desc, textures)) = self.load_nemat(&root, &p) {
                        live_names.insert(name.clone());
                        compiled.push((name, desc, textures));
                    }
                }
            }
        }

        {
            // NOTE: `MaterialRegistry` in this project uses interior mutability for edits via API trait.
            let reg = reg.read();

            for (name, desc, textures) in compiled {
                let _id = reg.upsert_named_with_textures(&name, desc, textures);
            }

            // Remove stale `materials/*` assets that no longer exist on disk.
            // Keep builtins intact (they are not under this prefix).
            let snapshot = reg.snapshot();
            for it in snapshot {
                if !it.name.starts_with("materials/") {
                    continue;
                }
                if !live_names.contains(&it.name) {
                    let _ = reg.remove(it.id);
                }
            }
        }
    }

    fn collect_files(root: &Path, out: &mut Vec<PathBuf>) {
        let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];

        while let Some(dir) = stack.pop() {
            let rd = match std::fs::read_dir(&dir) {
                Ok(v) => v,
                Err(_) => continue,
            };

            for ent in rd.flatten() {
                let p = ent.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.is_file() {
                    out.push(p);
                }
            }
        }
    }

    fn load_nemat(&mut self, root: &Path, path: &Path) -> Option<(String, MaterialDescriptor, MaterialTextureBindings)> {
        let stamp = file_stamp(path)?;
        if self.seen.get(path).copied() == Some(stamp) {
            return None;
        }

        let logical_path = logical_asset_path(root, path)?;
        let bytes = read_runtime_asset_bytes(&logical_path)?;
        let asset = decode_material_asset(&bytes).ok()?;
        let name = normalize_material_name(&asset.name, path)?;
        let mut desc = asset.desc;
        desc.sanitize_in_place();

        self.seen.insert(path.to_path_buf(), stamp);
        Some((name, desc, MaterialTextureBindings::default()))
    }

    fn load_or_compile_json(&mut self, root: &Path, path: &Path) -> Option<(String, MaterialDescriptor, MaterialTextureBindings)> {
        let stamp = file_stamp(path)?;
        if self.seen.get(path).copied() == Some(stamp) {
            return None;
        }

        let logical_path = logical_asset_path(root, path)?;
        let bytes = read_runtime_asset_bytes(&logical_path)?;
        let hash_hex = blake3::hash(&bytes).to_hex().to_string();
        let stem = path.file_stem()?.to_string_lossy().to_string();
        let name = format!("materials/{stem}");

        let cache_nemat = self.cache_dir.join(format!("{stem}.nemat"));
        let cache_meta = self.cache_dir.join(format!("{stem}.src.hash"));

        // JSON is the canonical source because MaterialAssetDocument owns texture bindings.
        // The `.nemat` cache remains a descriptor-only optimization artifact and must not
        // be allowed to erase texture/UV fields on cache hits.

        // Compile from JSON.
        let json = std::str::from_utf8(&bytes).ok()?;
        let source = if let Ok(source) = parse_material_source_json(json) {
            source.with_fallback_name(name.clone())
        } else {
            let mut desc = mat_serde::from_json(json).ok()?;
            desc.sanitize_in_place();
            newengine_materials::MaterialSourceDocument::new(
                Some(name.clone()),
                desc,
                MaterialTextureBindings::default(),
            )
        };
        let name = source.name.unwrap_or(name);
        let desc = source.desc;
        let textures = source.textures.clone();

        // Best-effort cache write.
        let _ = std::fs::create_dir_all(&self.cache_dir);
        if let Ok(bin) = encode_material_asset(&MaterialBinaryAsset {
            name: name.clone(),
            desc,
        }) {
            let _ = std::fs::write(&cache_nemat, &bin);
            let _ = std::fs::write(&cache_meta, &hash_hex);
        }

        self.seen.insert(path.to_path_buf(), stamp);
        Some((name, desc, textures))
    }
}

#[inline]
fn logical_asset_path(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let s = rel.to_string_lossy().replace('\\', "/");
    Some(s.trim_start_matches('/').to_owned())
}

#[inline]
fn read_runtime_asset_bytes(logical_path: &str) -> Option<Vec<u8>> {
    let assets = AssetServiceClient::new(default_host_api());
    match assets.raw_bytes_v1(logical_path) {
        Ok(bytes) => {
            log::debug!(
                "editor material pipeline: loaded material asset via AssetManager path='{}' bytes={}",
                logical_path,
                bytes.len()
            );
            Some(bytes)
        }
        Err(err) => {
            log::warn!(
                "editor material pipeline: AssetManager read failed path='{}' err='{}'",
                logical_path,
                err
            );
            None
        }
    }
}

#[inline]
fn normalize_material_name(asset_name: &str, fallback_path: &Path) -> Option<String> {
    let a = asset_name.trim();
    if !a.is_empty() {
        return Some(a.to_string());
    }
    let stem = fallback_path.file_stem()?.to_string_lossy();
    Some(format!("materials/{stem}"))
}

#[inline]
fn file_stamp(path: &Path) -> Option<u128> {
    let md = std::fs::metadata(path).ok()?;
    let t = md.modified().ok()?;
    system_time_to_stamp(t)
}

#[inline]
fn system_time_to_stamp(t: SystemTime) -> Option<u128> {
    let d = t.duration_since(UNIX_EPOCH).ok()?;
    Some((d.as_secs() as u128) * 1_000_000_000u128 + (d.subsec_nanos() as u128))
}
