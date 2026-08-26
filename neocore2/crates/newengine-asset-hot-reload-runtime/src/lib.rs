use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use newengine_assets::{AssetService, AssetServiceClient};
use newengine_assets_api::{
    plan_asset_invalidation_v1, AssetInvalidationPlanV1, AssetInvalidationRequestV1,
};
use newengine_core::{Resources, TaskLane, TaskPriority, TaskRequest, ThreadPoolHandle};
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
    #[inline]
    fn from_metadata(metadata: &fs::Metadata) -> Option<Self> {
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
struct ScanBatch {
    scan_unix_ms: u64,
    files: BTreeMap<PathBuf, FileStamp>,
}

#[derive(Debug)]
pub struct AssetFileWatcherRuntime {
    config: AssetFileWatcherConfig,
    known: Option<BTreeMap<PathBuf, FileStamp>>,
    pending: BTreeMap<String, u64>,
    scan_rx: Option<Receiver<ScanBatch>>,
    next_scan_unix_ms: u64,
    primed: bool,
}

impl Default for AssetFileWatcherRuntime {
    fn default() -> Self {
        Self {
            config: AssetFileWatcherConfig::default(),
            known: None,
            pending: BTreeMap::new(),
            scan_rx: None,
            next_scan_unix_ms: 0,
            primed: false,
        }
    }
}

impl AssetFileWatcherRuntime {
    pub fn new(config: AssetFileWatcherConfig) -> Self {
        Self {
            config,
            known: None,
            pending: BTreeMap::new(),
            scan_rx: None,
            next_scan_unix_ms: 0,
            primed: false,
        }
    }

    /// Primes hot reload without walking the filesystem on the frame thread.
    /// All recursive directory/metadata I/O is submitted to the engine-owned
    /// `AssetIo` lane so it participates in the common CPU budget and profiler.
    pub fn prime(&mut self, registry: &ContentMountRegistry, jobs: &ThreadPoolHandle) {
        self.known = None;
        self.pending.clear();
        self.scan_rx = None;
        self.next_scan_unix_ms = 0;
        self.primed = true;
        self.schedule_scan(registry, jobs, unix_ms_now());
    }

    /// Drains completed scan snapshots and returns only debounce-ready logical refs.
    /// No AssetServiceClient is constructed on the idle frame path.
    pub fn poll(
        &mut self,
        registry: &ContentMountRegistry,
        jobs: &ThreadPoolHandle,
    ) -> Option<(Vec<String>, u64)> {
        if !self.primed {
            self.prime(registry, jobs);
            return None;
        }

        let now = unix_ms_now();
        let mut scan_finished = false;
        if let Some(scan_rx) = self.scan_rx.as_ref() {
            match scan_rx.try_recv() {
                Ok(batch) => {
                    scan_finished = true;
                    if let Some(previous) = self.known.as_ref() {
                        for path in diff_changed_paths(previous, &batch.files) {
                            if let Some(logical) = registry.asset_ref_for_physical(&path) {
                                self.pending.insert(logical, batch.scan_unix_ms);
                            }
                        }
                    }
                    self.known = Some(batch.files);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    scan_finished = true;
                }
            }
        }
        if scan_finished {
            self.scan_rx = None;
        }

        if self.scan_rx.is_none() && now >= self.next_scan_unix_ms {
            self.schedule_scan(registry, jobs, now);
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
        Some((ready, now))
    }

    fn schedule_scan(
        &mut self,
        registry: &ContentMountRegistry,
        jobs: &ThreadPoolHandle,
        now: u64,
    ) {
        if self.scan_rx.is_some() {
            return;
        }

        let roots = watched_roots(registry, &self.config);
        let config = self.config.clone();
        let interval_ms = config.poll_interval.as_millis().max(1) as u64;
        let (scan_tx, scan_rx) = mpsc::channel::<ScanBatch>();
        let request = TaskRequest::new("asset-hot-reload-scan")
            .with_source("newengine-asset-hot-reload-runtime")
            .with_owner("engine.assets")
            .with_category("asset-hot-reload-scan")
            .with_lane(TaskLane::Background)
            .with_priority(TaskPriority::Background)
            .with_task_domain("engine.assets")
            .with_task_pass("hot-reload-scan")
            .pausable(false)
            .cancellable(false);

        let _ticket = jobs.submit_request(request, move || {
            let files = scan_watched_roots(&roots, &config);
            let _ = scan_tx.send(ScanBatch {
                scan_unix_ms: unix_ms_now(),
                files,
            });
        });
        self.scan_rx = Some(scan_rx);
        self.next_scan_unix_ms = now.saturating_add(interval_ms);
    }
}

pub fn install_asset_file_watcher(resources: &mut Resources) {
    if asset_hot_reload_disabled() || resources.get::<AssetFileWatcherRuntime>().is_some() {
        return;
    }
    let Some(registry) = resources.get::<ContentMountRegistry>().cloned() else {
        return;
    };
    let Some(jobs) = resources.get::<ThreadPoolHandle>().cloned() else {
        return;
    };
    let mut watcher = AssetFileWatcherRuntime::default();
    watcher.prime(&registry, &jobs);
    resources.insert(watcher);
}

pub fn poll_asset_file_watcher(resources: &mut Resources) -> Option<AssetHotReloadReportV1> {
    if asset_hot_reload_disabled() {
        return None;
    }
    let registry = resources.get::<ContentMountRegistry>()?.clone();
    let jobs = resources.get::<ThreadPoolHandle>()?.clone();
    let mut watcher = resources.remove::<AssetFileWatcherRuntime>()?;
    let ready = watcher.poll(&registry, &jobs);
    resources.insert(watcher);

    let (changed_refs, now) = ready?;
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let report = execute_hot_reload_batch(&assets, changed_refs, now);
    resources.insert(report.clone());
    Some(report)
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

fn watched_roots(registry: &ContentMountRegistry, config: &AssetFileWatcherConfig) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    for mount in registry.mounts() {
        if !config.watch_engine_mounts && mount.namespace == ContentMountNamespace::Engine {
            continue;
        }
        if mount.root.is_dir() {
            roots.insert(mount.root.clone());
        }
    }
    roots.into_iter().collect()
}

#[cfg(test)]
fn scan_watched_files(
    registry: &ContentMountRegistry,
    config: &AssetFileWatcherConfig,
) -> BTreeMap<PathBuf, FileStamp> {
    let roots = watched_roots(registry, config);
    scan_watched_roots(&roots, config)
}

fn scan_watched_roots(
    roots: &[PathBuf],
    config: &AssetFileWatcherConfig,
) -> BTreeMap<PathBuf, FileStamp> {
    let mut out = BTreeMap::new();
    for root in roots {
        scan_dir(root, config.max_files, &mut out);
        if out.len() >= config.max_files {
            break;
        }
    }
    out
}

fn diff_changed_paths(
    previous: &BTreeMap<PathBuf, FileStamp>,
    next: &BTreeMap<PathBuf, FileStamp>,
) -> Vec<PathBuf> {
    let mut changed_paths = BTreeSet::<PathBuf>::new();
    for (path, stamp) in next {
        if previous.get(path) != Some(stamp) {
            changed_paths.insert(path.clone());
        }
    }
    for path in previous.keys() {
        if !next.contains_key(path) {
            changed_paths.insert(path.clone());
        }
    }
    changed_paths.into_iter().collect()
}

fn scan_dir(root: &Path, max_files: usize, out: &mut BTreeMap<PathBuf, FileStamp>) {
    if out.len() >= max_files {
        return;
    }

    // Iterative traversal avoids recursive call growth on deeply nested content
    // trees. More importantly, `DirEntry::file_type` + `DirEntry::metadata`
    // removes the previous `Path::is_dir()` followed by a second metadata lookup
    // for every file.
    let mut pending_dirs = Vec::with_capacity(64);
    pending_dirs.push(root.to_path_buf());

    while let Some(directory) = pending_dirs.pop() {
        if out.len() >= max_files {
            break;
        }
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };

        for entry in entries.flatten() {
            if out.len() >= max_files {
                break;
            }

            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".git" || name == "target" || name == ".idea" || name == ".vs" {
                continue;
            }

            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                pending_dirs.push(entry.path());
                continue;
            }
            if !file_type.is_file() {
                continue;
            }

            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if let Some(stamp) = FileStamp::from_metadata(&metadata) {
                out.insert(entry.path(), stamp);
            }
        }
    }
}

fn asset_hot_reload_disabled() -> bool {
    newengine_plugin_host::current_host_context()
        .environment_var(ASSET_HOT_RELOAD_DISABLE_ENV)
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

        let config = AssetFileWatcherConfig {
            poll_interval: Duration::ZERO,
            debounce: Duration::ZERO,
            ..Default::default()
        };
        let before = scan_watched_files(&registry, &config);
        fs::write(&file, b"changed").unwrap();
        let after = scan_watched_files(&registry, &config);
        let changed = diff_changed_paths(&before, &after);
        assert!(changed.contains(&file));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn diff_detects_added_changed_and_removed_paths() {
        let a = PathBuf::from("a.nemat");
        let b = PathBuf::from("b.nemat");
        let c = PathBuf::from("c.nemat");
        let mut before = BTreeMap::new();
        before.insert(
            a.clone(),
            FileStamp {
                len: 1,
                modified_ns: 1,
            },
        );
        before.insert(
            b.clone(),
            FileStamp {
                len: 2,
                modified_ns: 2,
            },
        );
        let mut after = BTreeMap::new();
        after.insert(
            a.clone(),
            FileStamp {
                len: 3,
                modified_ns: 3,
            },
        );
        after.insert(
            c.clone(),
            FileStamp {
                len: 4,
                modified_ns: 4,
            },
        );

        assert_eq!(diff_changed_paths(&before, &after), vec![a, b, c]);
    }
}
