#![forbid(unsafe_op_in_unsafe_fn)]

use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub(super) struct InitTimings {
    pub(super) config_defaults_ms: u128,
    pub(super) config_apply_ms: u128,
    pub(super) init_ms: u128,
}

#[derive(Debug, Clone, Default)]
pub(super) struct LoadTimings {
    pub(super) dlopen_ms: u128,
    pub(super) sym_ms: u128,
    pub(super) descriptor_v2_ms: u128,
    pub(super) root_ms: u128,
    pub(super) module_create_ms: u128,
    pub(super) identity_validation_ms: u128,
    pub(super) discovery_verify_ms: u128,
    pub(super) override_lookup_ms: u128,
    pub(super) provider_prepare_ms: u128,
    pub(super) init_total_ms: u128,
    pub(super) init_breakdown: Option<InitTimings>,
    pub(super) total_ms: u128,
}

pub(super) struct LoadProfilerJob {
    id: String,
    path: String,
    started: Instant,
    completed: bool,
}

impl LoadProfilerJob {
    pub(super) fn begin(path: &str) -> Self {
        let id = crate::diagnostics::next_job_id("host.plugin_load");
        crate::diagnostics::begin(serde_json::json!({
            "id": id.clone(),
            "name": format!("plugin_load:{}", path),
            "category": "plugin_lifecycle",
            "source": "newengine-plugin-host",
            "detail": "dynamic library load + canonical ABI root + init",
            "metadata": { "path": path, "operation": "load_one" }
        }));
        Self {
            id,
            path: path.to_owned(),
            started: Instant::now(),
            completed: false,
        }
    }

    pub(super) fn complete_ok(&mut self, plugin_id: &str, timings: &LoadTimings) {
        self.completed = true;
        let init = timings.init_breakdown.as_ref().cloned().unwrap_or_default();
        let attributed_ms = timings
            .dlopen_ms
            .saturating_add(timings.sym_ms)
            .saturating_add(timings.descriptor_v2_ms)
            .saturating_add(timings.root_ms)
            .saturating_add(timings.module_create_ms)
            .saturating_add(timings.identity_validation_ms)
            .saturating_add(timings.discovery_verify_ms)
            .saturating_add(timings.override_lookup_ms)
            .saturating_add(timings.provider_prepare_ms)
            .saturating_add(timings.init_total_ms);
        let unattributed_ms = timings.total_ms.saturating_sub(attributed_ms);
        let breakdown = format!(
            "dlopen={}ms sym={}ms descriptor_v2={}ms root={}ms module_create={}ms identity_validation={}ms discovery_verify={}ms override_lookup={}ms provider_prepare={}ms init_total={}ms unattributed={}ms",
            timings.dlopen_ms,
            timings.sym_ms,
            timings.descriptor_v2_ms,
            timings.root_ms,
            timings.module_create_ms,
            timings.identity_validation_ms,
            timings.discovery_verify_ms,
            timings.override_lookup_ms,
            timings.provider_prepare_ms,
            timings.init_total_ms,
            unattributed_ms,
        );
        crate::diagnostics::end(serde_json::json!({
            "id": self.id.clone(),
            "status": "completed",
            "detail": format!(
                "plugin loaded in {} ms (dlopen={} init={} config={}+{})",
                timings.total_ms,
                timings.dlopen_ms,
                init.init_ms,
                init.config_defaults_ms,
                init.config_apply_ms,
            ),
            "metadata": {
                "plugin_id": plugin_id,
                "path": self.path.clone(),
                "operation": "load_one",
                "total_ms": timings.total_ms,
                "dlopen_ms": timings.dlopen_ms,
                "sym_ms": timings.sym_ms,
                "descriptor_v2_ms": timings.descriptor_v2_ms,
                "root_ms": timings.root_ms,
                "module_create_ms": timings.module_create_ms,
                "identity_validation_ms": timings.identity_validation_ms,
                "discovery_verify_ms": timings.discovery_verify_ms,
                "override_lookup_ms": timings.override_lookup_ms,
                "provider_prepare_ms": timings.provider_prepare_ms,
                "init_total_ms": timings.init_total_ms,
                "init_breakdown": {
                    "config_defaults_ms": init.config_defaults_ms,
                    "config_apply_ms": init.config_apply_ms,
                    "init_call_ms": init.init_ms
                },
                "unattributed_ms": unattributed_ms,
                "breakdown": breakdown
            }
        }));
    }
}

impl Drop for LoadProfilerJob {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        crate::diagnostics::end(serde_json::json!({
            "id": self.id.clone(),
            "status": "failed",
            "error": "plugin load exited before completion",
            "detail": format!(
                "plugin load failed or was skipped after {:.3} ms",
                crate::diagnostics::elapsed_ms(self.started)
            ),
            "metadata": { "path": self.path.clone(), "operation": "load_one" }
        }));
    }
}
