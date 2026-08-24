#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub const HOST_CAPABILITIES_SCHEMA_VERSION: u32 = 1;

pub const ENGINE_HOST_CAPABILITIES_GATEWAY_ID: &str = "engine.host.capabilities";
pub const HOST_CAPABILITIES_PROVIDER_SERVICE_ID: &str =
    "newengine.host.capabilities.native";
pub const HOST_CAPABILITIES_PROVIDER_ROUTE: &str =
    "newengine.host.capabilities.runtime";
pub const HOST_CAPABILITIES_BACKEND_CAPABILITY_ID: &str =
    "host.capabilities.backend";
pub const HOST_CAPABILITIES_SERVICE_KIND: &str = "host.capabilities";
pub const HOST_CAPABILITIES_RUNTIME_CONTRACT: &str =
    "newengine.host.capabilities.runtime.v1";

pub mod method {
    pub const SNAPSHOT: &str = "host.capabilities.snapshot_v1";
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostEnvironmentSnapshot {
    pub executable: Option<String>,
    pub cwd: Option<String>,
    pub pid: u32,
    pub os: String,
    pub arch: String,
    pub family: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPlatformServices {
    pub native_threads: bool,
    pub filesystem: bool,
    pub process_environment: bool,
    pub dynamic_library_loading: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostAffinityPolicy {
    #[default]
    Automatic,
    ProcessDefault,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuFeatureSet {
    pub sse2: bool,
    pub sse41: bool,
    pub avx: bool,
    pub avx2: bool,
    pub f16c: bool,
    pub fma: bool,
    pub bmi1: bool,
    pub bmi2: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuCapabilities {
    pub brand: Option<String>,
    pub physical_cores: Option<u32>,
    pub logical_cores: Option<u32>,
    pub features: CpuFeatureSet,
    pub affinity_policy: HostAffinityPolicy,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryCapabilities {
    pub total_mb: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuCapabilities {
    pub index: u32,
    pub name: String,
    pub vendor_id: Option<u32>,
    pub device_id: Option<u32>,
    pub subsystem_id: Option<u32>,
    pub revision: Option<u32>,
    pub dedicated_vram_mb: Option<u64>,
    pub dedicated_system_mb: Option<u64>,
    pub shared_system_mb: Option<u64>,
    pub is_software: bool,
    pub is_discrete: bool,
    pub stable_id: String,
    pub graphics_api: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageCapabilities {
    pub name: String,
    pub mount_point: String,
    pub file_system: Option<String>,
    pub total_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub removable: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayCapabilities {
    pub index: u32,
    pub name: Option<String>,
    pub primary: bool,
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
    pub refresh_rate_millihz: Option<u32>,
    pub hdr_capable: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputCapabilities {
    pub keyboard_present: Option<bool>,
    pub mouse_present: Option<bool>,
    pub mouse_buttons: Option<u32>,
    pub mouse_wheel_present: Option<bool>,
    pub touch_present: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostCapabilities {
    pub cpu: CpuCapabilities,
    pub memory: MemoryCapabilities,
    pub gpu: Vec<GpuCapabilities>,
    pub preferred_gpu_index: Option<usize>,
    pub preferred_gpu_reason: Option<String>,
    pub storage: Vec<StorageCapabilities>,
    pub displays: Vec<DisplayCapabilities>,
    pub input: InputCapabilities,
}

impl HostCapabilities {
    #[inline]
    pub fn preferred_gpu(&self) -> Option<&GpuCapabilities> {
        self.preferred_gpu_index
            .and_then(|index| self.gpu.get(index))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSelectionHint {
    pub gateway_id: String,
    pub preferred_system_tags: Vec<String>,
    pub forbidden_system_tags: Vec<String>,
    pub preference_bonus: i32,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCapabilityPolicy {
    pub worker_threads: Option<u32>,
    pub preferred_gpu_stable_id: Option<String>,
    pub allow_software_rendering: bool,
    pub provider_hints: Vec<ProviderSelectionHint>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPreInitSnapshot {
    pub schema_version: u32,
    pub environment: HostEnvironmentSnapshot,
    pub platform_services: HostPlatformServices,
    pub capabilities: HostCapabilities,
    pub runtime_policy: RuntimeCapabilityPolicy,
}

impl Default for HostPreInitSnapshot {
    fn default() -> Self {
        Self {
            schema_version: HOST_CAPABILITIES_SCHEMA_VERSION,
            environment: HostEnvironmentSnapshot::default(),
            platform_services: HostPlatformServices::default(),
            capabilities: HostCapabilities::default(),
            runtime_policy: RuntimeCapabilityPolicy::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_gateway_and_native_provider_identity_are_distinct() {
        assert!(ENGINE_HOST_CAPABILITIES_GATEWAY_ID.starts_with("engine."));
        assert!(HOST_CAPABILITIES_PROVIDER_SERVICE_ID.starts_with("newengine."));
        assert_ne!(
            ENGINE_HOST_CAPABILITIES_GATEWAY_ID,
            HOST_CAPABILITIES_PROVIDER_SERVICE_ID
        );
        assert_ne!(
            ENGINE_HOST_CAPABILITIES_GATEWAY_ID,
            HOST_CAPABILITIES_PROVIDER_ROUTE
        );
    }

    #[test]
    fn neutral_snapshot_keeps_the_contract_version() {
        assert_eq!(
            HostPreInitSnapshot::default().schema_version,
            HOST_CAPABILITIES_SCHEMA_VERSION
        );
    }
}
