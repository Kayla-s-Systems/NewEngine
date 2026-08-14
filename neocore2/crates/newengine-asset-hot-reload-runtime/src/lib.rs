use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use newengine_assets::{AssetService, AssetServiceClient};
use newengine_assets_api::{
    plan_asset_invalidation_v1, AssetInvalidationPlanV1, AssetInvalidationRequestV1,
};
use newengine_core::Resources;
use newengine_project_api::{ContentMountNamespace, ContentMountRegistry};
use serde::{Deserialize, Serialize};

pub const ASSET_HOT_RELOAD_REPORT_SCHEMA: &str = "newengine.assets.hot_reload.report.v1";
pub const ASSET_HOT_RELOAD_DISABLE_ENV: &str = "NEWENGINE_DISABLE_ASSET_HOT_RELOAD";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileStamp {
    len: u64,
    modified_ns: u128,
}

impl FileStamp {
    fn from_path(path: &Path) -> Option<Self> {
        let metadata = fs::metadata(path).ok()?;
        if !metadata.is_file() {
            return None;
        }
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        Some(Self {
            len: metadata.len(),
            modified_ns,
        })
    }
}

#[derive(Clone, Debug)]
pub struct AssetFileWatcherConfig {
    pub poll_interval: Duration,
    pub debounce: Duration,
    pub max_files: usize,
    pub watch_engine_mounts: bool,
}

impl Default for AssetFileWatcherConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(250),
            debounce: Duration::from_millis(120),
            max_files: 50_000,
            watch_engine_mounts: false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetHotReloadOperationV1 {
    pub logical_ref: String,
    pub ok: bool,
    pub response: serde_json::Value,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetHotReloadReportV1 {
    pub schema: String,
    pub scan_unix_ms: u64,
    pub changed_refs: Vec<String>,
    pub plan: AssetInvalidationPlanV1,
    pub operations: Vec<AssetHotReloadOperationV1>,
    pub warnings: Vec<String>,
}

impl Default for AssetHotReloadReportV1 {
    fn default() -> Self {
        Self {
            schema: ASSET_HOT_RELOAD_REPORT_SCHEMA.to_owned(),
            scan_unix_ms: 0,
            changed_refs: Vec::new(),
            plan: AssetInvalidationPlanV1::default(),
            operations: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct AssetFileWatcherRuntime {
    config: AssetFileWatcherConfig,
    known: BTreeMap<PathBuf, FileStamp>,
    pending: BTreeMap<String, u64>,
    last_scan_unix_ms: u64,
    primed: bool,
}

impl Default for AssetFileWatcherRuntime {
    fn default() -> Self {
        Self {
            config: AssetFileWatcherConfig::default(),
            known: BTreeMap::new(),
            pending: BTreeMap::new(),
            last_scan_unix_ms: 0,
            primed: false,
        }
    }
}

impl AssetFileWatcherRuntime {
    pub fn new(config: AssetFileWatcherConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    pub fn prime(&mut self, registry: &ContentMountRegistry) {
        self.known = scan_watched_files(registry, &self.config);
        self.pending.clear();
        self.last_scan_unix_ms = unix_ms_now();
        self.primed = true;
    }

    pub fn poll(
        &mut self,
        registry: &ContentMountRegistry,
        assets: &AssetServiceClient,
    ) -> Option<AssetHotReloadReportV1> {
        let now = unix_ms_now();
        if !self.primed {
            self.prime(registry);
            return None;
        }
        if now.saturating_sub(self.last_scan_unix_ms) < self.config.poll_interval.as_millis() as u64
        {
            return None;
        }
        self.last_scan_unix_ms = now;

        let next = scan_watched_files(registry, &self.config);
        let mut changed_paths = BTreeSet::<PathBuf>::new();
        for (path, stamp) in &next {
            if self.known.get(path) != Some(stamp) {
                changed_paths.insert(path.clone());
            }
        }
        for path in self.known.keys() {
            if !next.contains_key(path) {
                changed_paths.insert(path.clone());
            }
        }
        self.known = next;

        for path in changed_paths {
            if let Some(logical) = registry.asset_ref_for_physical(&path) {
                self.pending.insert(logical, now);
            }
        }

        let debounce_ms = self.config.debounce.as_millis() as u64;
        let ready = self
            .pending
            .iter()
            .filter(|(_, changed_at)| now.saturating_sub(**changed_at) >= debounce_ms)
            .map(|(logical, _)| logical.clone())
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return None;
        }
        for logical in &ready {
            self.pending.remove(logical);
        }
        Some(execute_hot_reload_batch(assets, ready, now))
    }
}

pub fn install_asset_file_watcher(resources: &mut Resources) {
    if asset_hot_reload_disabled() || resources.get::<AssetFileWatcherRuntime>().is_some() {
        return;
    }
    let Some(registry) = resources.get::<ContentMountRegistry>().cloned() else {
        return;
    };
    let mut watcher = AssetFileWatcherRuntime::default();
    watcher.prime(&registry);
    resources.insert(watcher);
}

pub fn poll_asset_file_watcher(resources: &mut Resources) -> Option<AssetHotReloadReportV1> {
    if asset_hot_reload_disabled() {
        return None;
    }
    let registry = resources.get::<ContentMountRegistry>()?.clone();
    let mut watcher = resources.remove::<AssetFileWatcherRuntime>()?;
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let report = watcher.poll(&registry, &assets);
    resources.insert(watcher);
    if let Some(report) = report.clone() {
        resources.insert(report);
    }
    report
}

fn execute_hot_reload_batch(
    assets: &AssetServiceClient,
    changed_refs: Vec<String>,
    now: u64,
) -> AssetHotReloadReportV1 {
    let mut warnings = Vec::new();
    let graph = match assets.runtime_graph_json_v1() {
        Ok(graph) => graph,
        Err(error) => {
            warnings.push(format!(
                "runtime graph unavailable; invalidating changed roots only: {error}"
            ));
            newengine_assets_api::AssetRuntimeGraphV1::default()
        }
    };
    let plan = plan_asset_invalidation_v1(
        &graph,
        AssetInvalidationRequestV1 {
            changed_sources: changed_refs.clone(),
            reason: "project file watcher".to_owned(),
        },
    );
    let mut operations = Vec::new();
    for logical_ref in &plan.invalidation_order {
        match assets.reimport_v1(serde_json::json!({
            "logical_path": logical_ref,
            "reason": format!("dependency-aware hot reload from {}", changed_refs.join(", ")),
        })) {
            Ok(response) => {
                let ok = response
                    .get("ok")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true);
                operations.push(AssetHotReloadOperationV1 {
                    logical_ref: logical_ref.clone(),
                    ok,
                    response,
                    error: None,
                });
            }
            Err(error) => operations.push(AssetHotReloadOperationV1 {
                logical_ref: logical_ref.clone(),
                ok: false,
                response: serde_json::Value::Null,
                error: Some(error),
            }),
        }
    }
    AssetHotReloadReportV1 {
        scan_unix_ms: now,
        changed_refs,
        plan,
        operations,
        warnings,
        ..AssetHotReloadReportV1::default()
    }
}

fn scan_watched_files(
    registry: &ContentMountRegistry,
    config: &AssetFileWatcherConfig,
) -> BTreeMap<PathBuf, FileStamp> {
    let mut out = BTreeMap::new();
    let mut roots = BTreeSet::new();
    for mount in registry.mounts() {
        if !config.watch_engine_mounts && mount.namespace == ContentMountNamespace::Engine {
            continue;
        }
        if mount.root.is_dir() {
            roots.insert(mount.root.clone());
        }
    }
    for root in roots {
        scan_dir(&root, config.max_files, &mut out);
        if out.len() >= config.max_files {
            break;
        }
    }
    out
}

fn scan_dir(root: &Path, max_files: usize, out: &mut BTreeMap<PathBuf, FileStamp>) {
    if out.len() >= max_files {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= max_files {
            break;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || name == "target" || name == ".idea" || name == ".vs" {
            continue;
        }
        if path.is_dir() {
            scan_dir(&path, max_files, out);
        } else if let Some(stamp) = FileStamp::from_path(&path) {
            out.insert(path, stamp);
        }
    }
}

fn asset_hot_reload_disabled() -> bool {
    std::env::var(ASSET_HOT_RELOAD_DISABLE_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_mount_maps_physical_change_to_logical_ref() {
        let root =
            std::env::temp_dir().join(format!("newengine-hot-reload-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("materials")).unwrap();
        let file = root.join("materials/test.nemat");
        fs::write(&file, b"a").unwrap();
        let mut registry = ContentMountRegistry::default();
        registry
            .register(newengine_project_api::ContentMountDescriptor {
                id: "test.game".to_owned(),
                namespace: ContentMountNamespace::Game,
                root: root.clone(),
                mount: "game".to_owned(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            registry.asset_ref_for_physical(&file),
            Some("game/materials/test.nemat".to_owned())
        );
        let mut watcher = AssetFileWatcherRuntime::new(AssetFileWatcherConfig {
            poll_interval: Duration::ZERO,
            debounce: Duration::ZERO,
            ..Default::default()
        });
        watcher.prime(&registry);
        fs::write(&file, b"changed").unwrap();
        let next = scan_watched_files(&registry, &watcher.config);
        assert_ne!(watcher.known.get(&file), next.get(&file));
        let _ = fs::remove_dir_all(&root);
    }
}
