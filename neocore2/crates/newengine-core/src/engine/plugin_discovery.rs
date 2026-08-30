use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

use crate::task_core::{TaskLane, TaskPriority, TaskRequest};
use crate::threading::{TaskTicket, ThreadPoolHandle};

pub(super) struct PluginDiscoveryScanTask {
    dir: PathBuf,
    result: Arc<
        Mutex<
            Option<
                Result<
                    newengine_plugin_host::PluginDiscoveryGraph,
                    newengine_plugin_host::PluginLoadError,
                >,
            >,
        >,
    >,
    _ticket: TaskTicket,
}

impl PluginDiscoveryScanTask {
    pub(super) fn submit(thread_pool: ThreadPoolHandle, dir: PathBuf) -> Self {
        let result = Arc::new(Mutex::new(None));
        let result_worker = Arc::clone(&result);
        let dir_worker = dir.clone();
        let request = TaskRequest::new("plugin.discovery.scan")
            .with_lane(TaskLane::Plugin)
            .with_priority(TaskPriority::Critical)
            .with_source("newengine-core.engine.plugins")
            .with_owner("newengine-core")
            .with_category("plugin-discovery")
            .with_task_pass("discovery-scan")
            .with_dependency_group("engine-plugin-discovery")
            .pausable(false)
            .cancellable(true);

        let ticket = thread_pool.submit_controlled(request, move |context| {
            context.publish_progress(
                0.05,
                "Scanning runtime plugin descriptors...",
                format!("dir='{}'", dir_worker.display()),
            );

            if !context.checkpoint() {
                *result_worker.lock() = Some(Err(newengine_plugin_host::PluginLoadError {
                    path: dir_worker,
                    message: "plugin discovery scan cancelled before descriptor probing".to_owned(),
                }));
                return;
            }

            let outcome = newengine_plugin_host::scan_plugin_discovery_graph(&dir_worker);
            context.publish_progress(
                1.0,
                "Runtime plugin discovery scan completed.",
                "DiscoveryGraph is ready for main-thread commit.",
            );
            *result_worker.lock() = Some(outcome);
        });

        Self {
            dir,
            result,
            _ticket: ticket,
        }
    }

    #[inline]
    pub(super) fn dir(&self) -> &PathBuf {
        &self.dir
    }

    #[inline]
    pub(super) fn take_result(
        &self,
    ) -> Option<
        Result<newengine_plugin_host::PluginDiscoveryGraph, newengine_plugin_host::PluginLoadError>,
    > {
        self.result.lock().take()
    }
}
