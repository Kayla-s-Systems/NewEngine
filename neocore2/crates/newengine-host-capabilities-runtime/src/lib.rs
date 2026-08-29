#![forbid(unsafe_op_in_unsafe_fn)]

mod service;

pub use service::native_host_capabilities_service;

use newengine_host_capabilities_api::{
    CpuCapabilities, DisplayCapabilities, GpuCapabilities, HostCapabilities,
    HostEnvironmentSnapshot, HostPlatformServices, HostPreInitSnapshot, InputCapabilities,
    MemoryCapabilities, RuntimeCapabilityPolicy, StorageCapabilities,
    HOST_CAPABILITIES_SCHEMA_VERSION,
};

pub type EnvironmentProbe = fn() -> HostEnvironmentSnapshot;
pub type PlatformServicesProbe = fn() -> HostPlatformServices;
pub type CpuProbe = fn() -> CpuCapabilities;
pub type MemoryProbe = fn() -> MemoryCapabilities;
pub type GpuProbe = fn() -> Vec<GpuCapabilities>;
pub type GpuSelector = fn(&[GpuCapabilities]) -> Option<usize>;
pub type GpuPreferenceReason = fn(&[GpuCapabilities], Option<usize>) -> Option<String>;
pub type StorageProbe = fn() -> Vec<StorageCapabilities>;
pub type DisplayProbe = fn() -> Vec<DisplayCapabilities>;
pub type InputProbe = fn() -> InputCapabilities;
pub type PolicyDeriver = fn(&HostEnvironmentSnapshot, &HostCapabilities) -> RuntimeCapabilityPolicy;

/// Typed composition root for host PreInit discovery.
///
/// Every field is a narrow leaf operation. The factory is the only place that knows
/// the complete HostPreInitSnapshot shape; probes never know about each other, the Host,
/// service registration, render backends, gameplay, or editor code.
#[derive(Clone, Copy)]
pub struct HostCapabilityFactory {
    pub environment: EnvironmentProbe,
    pub platform_services: PlatformServicesProbe,
    pub cpu: CpuProbe,
    pub memory: MemoryProbe,
    pub gpu: GpuProbe,
    pub select_gpu: GpuSelector,
    pub gpu_preference_reason: GpuPreferenceReason,
    pub storage: StorageProbe,
    pub displays: DisplayProbe,
    pub input: InputProbe,
    pub derive_policy: PolicyDeriver,
}

impl HostCapabilityFactory {
    /// Native factory assembled from single-purpose leaf probes.
    pub const fn native() -> Self {
        Self {
            environment: newengine_host_probe_environment::discover,
            platform_services: newengine_host_probe_platform_services::discover,
            cpu: newengine_host_probe_cpu::discover,
            memory: newengine_host_probe_memory::discover,
            gpu: newengine_host_probe_gpu::discover,
            select_gpu: newengine_host_probe_gpu::select_preferred,
            gpu_preference_reason: newengine_host_probe_gpu::preferred_reason,
            storage: newengine_host_probe_storage::discover,
            displays: newengine_host_probe_display::discover,
            input: newengine_host_probe_input::discover,
            derive_policy: newengine_host_capability_policy::derive,
        }
    }

    pub fn discover(&self) -> HostPreInitSnapshot {
        let environment = (self.environment)();
        let platform_services = (self.platform_services)();
        let mut gpu = (self.gpu)();
        gpu.sort_by_key(|adapter| adapter.index);
        let preferred_gpu_index = (self.select_gpu)(&gpu);
        let preferred_gpu_reason = (self.gpu_preference_reason)(&gpu, preferred_gpu_index);
        let capabilities = HostCapabilities {
            cpu: (self.cpu)(),
            memory: (self.memory)(),
            gpu,
            preferred_gpu_index,
            preferred_gpu_reason,
            storage: (self.storage)(),
            displays: (self.displays)(),
            input: (self.input)(),
        };
        let runtime_policy = (self.derive_policy)(&environment, &capabilities);
        HostPreInitSnapshot {
            schema_version: HOST_CAPABILITIES_SCHEMA_VERSION,
            environment,
            platform_services,
            capabilities,
            runtime_policy,
        }
    }
}

impl Default for HostCapabilityFactory {
    fn default() -> Self {
        Self::native()
    }
}

/// Compatibility entry point used by the native provider service.
pub fn discover_preinit_snapshot() -> HostPreInitSnapshot {
    HostCapabilityFactory::native().discover()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_environment() -> HostEnvironmentSnapshot {
        HostEnvironmentSnapshot {
            os: "test-os".to_owned(),
            arch: "test-arch".to_owned(),
            pid: 7,
            ..Default::default()
        }
    }
    fn fake_platform() -> HostPlatformServices {
        HostPlatformServices::default()
    }
    fn fake_cpu() -> CpuCapabilities {
        CpuCapabilities {
            logical_cores: Some(4),
            ..Default::default()
        }
    }
    fn fake_memory() -> MemoryCapabilities {
        MemoryCapabilities {
            total_mb: Some(4096),
        }
    }
    fn fake_gpu() -> Vec<GpuCapabilities> {
        vec![GpuCapabilities {
            index: 2,
            stable_id: "fake-gpu".to_owned(),
            ..Default::default()
        }]
    }
    fn fake_select(_: &[GpuCapabilities]) -> Option<usize> {
        Some(0)
    }
    fn fake_reason(_: &[GpuCapabilities], _: Option<usize>) -> Option<String> {
        Some("test".to_owned())
    }
    fn fake_storage() -> Vec<StorageCapabilities> {
        Vec::new()
    }
    fn fake_displays() -> Vec<DisplayCapabilities> {
        Vec::new()
    }
    fn fake_input() -> InputCapabilities {
        InputCapabilities::default()
    }
    fn fake_policy(
        _: &HostEnvironmentSnapshot,
        capabilities: &HostCapabilities,
    ) -> RuntimeCapabilityPolicy {
        RuntimeCapabilityPolicy {
            worker_threads: capabilities.cpu.logical_cores,
            ..Default::default()
        }
    }

    #[test]
    fn factory_composes_leaf_results_without_leaf_cross_dependencies() {
        let snapshot = HostCapabilityFactory {
            environment: fake_environment,
            platform_services: fake_platform,
            cpu: fake_cpu,
            memory: fake_memory,
            gpu: fake_gpu,
            select_gpu: fake_select,
            gpu_preference_reason: fake_reason,
            storage: fake_storage,
            displays: fake_displays,
            input: fake_input,
            derive_policy: fake_policy,
        }
        .discover();
        assert_eq!(snapshot.schema_version, HOST_CAPABILITIES_SCHEMA_VERSION);
        assert_eq!(snapshot.environment.os, "test-os");
        assert_eq!(snapshot.capabilities.cpu.logical_cores, Some(4));
        assert_eq!(
            snapshot
                .capabilities
                .preferred_gpu()
                .map(|gpu| gpu.stable_id.as_str()),
            Some("fake-gpu")
        );
        assert_eq!(snapshot.runtime_policy.worker_threads, Some(4));
    }
}
