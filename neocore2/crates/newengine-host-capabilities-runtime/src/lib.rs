#![forbid(unsafe_op_in_unsafe_fn)]

mod service;

pub use service::native_host_capabilities_service;

use newengine_host_capabilities_api::{
    CpuCapabilities, CpuFeatureSet, DisplayCapabilities, GpuCapabilities, HostAffinityPolicy,
    HostCapabilities, HostEnvironmentSnapshot, HostPlatformServices, HostPreInitSnapshot,
    InputCapabilities, MemoryCapabilities, ProviderSelectionHint, RuntimeCapabilityPolicy,
    StorageCapabilities, HOST_CAPABILITIES_SCHEMA_VERSION,
};
use sysinfo::{Disks, System};

pub fn discover_preinit_snapshot() -> HostPreInitSnapshot {
    let environment = discover_environment();
    let platform_services = discover_platform_services();
    let cpu = discover_cpu();
    let memory = discover_memory();
    let mut gpu = discover_gpu();
    gpu.sort_by_key(|adapter| adapter.index);
    let preferred_gpu_index = select_preferred_gpu(&gpu);
    let preferred_gpu_reason = preferred_gpu_index.and_then(|index| {
        gpu.get(index).map(|adapter| {
            if adapter.is_software {
                "fallback_software_adapter".to_owned()
            } else if adapter.dedicated_vram_mb.unwrap_or(0) > 0 {
                "highest_score_discrete_dedicated_vram".to_owned()
            } else {
                "first_hardware_adapter".to_owned()
            }
        })
    });
    let storage = discover_storage();
    let displays = discover_displays();
    let input = discover_input();
    let capabilities = HostCapabilities {
        cpu,
        memory,
        gpu,
        preferred_gpu_index,
        preferred_gpu_reason,
        storage,
        displays,
        input,
    };
    let runtime_policy = derive_runtime_policy(&environment, &capabilities);
    HostPreInitSnapshot {
        schema_version: HOST_CAPABILITIES_SCHEMA_VERSION,
        environment,
        platform_services,
        capabilities,
        runtime_policy,
    }
}

pub fn emit_preinit_diagnostics(snapshot: &HostPreInitSnapshot) {
    let cpu = &snapshot.capabilities.cpu;
    newengine_ulog_api::ulog::info!(
        "[NxHost] PreInit os='{}' arch='{}' pid={} physical_cores={} logical_cores={} affinity='auto' avx={} f16c={} avx2={}",
        snapshot.environment.os,
        snapshot.environment.arch,
        snapshot.environment.pid,
        opt_u32(cpu.physical_cores),
        opt_u32(cpu.logical_cores),
        cpu.features.avx as u8,
        cpu.features.f16c as u8,
        cpu.features.avx2 as u8,
    );
    newengine_ulog_api::ulog::info!(
        "[NxHost] HardwareDiscovery gpu={} storage={} displays={} keyboard={} mouse={} preferred_gpu='{}'",
        snapshot.capabilities.gpu.len(),
        snapshot.capabilities.storage.len(),
        snapshot.capabilities.displays.len(),
        opt_bool(snapshot.capabilities.input.keyboard_present),
        opt_bool(snapshot.capabilities.input.mouse_present),
        snapshot
            .capabilities
            .preferred_gpu()
            .map(|gpu| gpu.stable_id.as_str())
            .unwrap_or("<none>"),
    );
    for gpu in &snapshot.capabilities.gpu {
        newengine_ulog_api::ulog::info!(
            "[NxHost] GPU index={} name='{}' stable_id='{}' discrete={} software={} vram_mb={} graphics_api='{}'",
            gpu.index,
            gpu.name,
            gpu.stable_id,
            gpu.is_discrete,
            gpu.is_software,
            gpu.dedicated_vram_mb.map(|v| v.to_string()).unwrap_or_else(|| "<unknown>".to_owned()),
            gpu.graphics_api.as_deref().unwrap_or("<unknown>"),
        );
    }
    newengine_ulog_api::ulog::info!(
        "[NxHost] CapabilityPolicy worker_threads={} allow_software_rendering={} provider_hints={}",
        opt_u32(snapshot.runtime_policy.worker_threads),
        snapshot.runtime_policy.allow_software_rendering,
        snapshot.runtime_policy.provider_hints.len(),
    );
}

fn discover_environment() -> HostEnvironmentSnapshot {
    HostEnvironmentSnapshot {
        executable: std::env::current_exe().ok().map(display_path),
        cwd: std::env::current_dir().ok().map(display_path),
        pid: std::process::id(),
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        family: std::env::consts::FAMILY.to_owned(),
    }
}

fn display_path(path: std::path::PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

fn discover_platform_services() -> HostPlatformServices {
    HostPlatformServices {
        native_threads: std::thread::available_parallelism().is_ok(),
        filesystem: true,
        process_environment: true,
        dynamic_library_loading: matches!(std::env::consts::OS, "windows" | "linux" | "macos"),
    }
}

fn discover_cpu() -> CpuCapabilities {
    let mut sys = System::new_all();
    sys.refresh_cpu_all();
    let brand = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var("PROCESSOR_IDENTIFIER").ok());
    let logical_cores = match sys.cpus().len() {
        0 => std::thread::available_parallelism()
            .ok()
            .map(|n| n.get() as u32),
        count => Some(count as u32),
    };
    let physical_cores = System::physical_core_count().map(|count| count as u32);
    CpuCapabilities {
        brand,
        physical_cores,
        logical_cores,
        features: discover_cpu_features(),
        affinity_policy: HostAffinityPolicy::Automatic,
    }
}

fn discover_cpu_features() -> CpuFeatureSet {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        CpuFeatureSet {
            sse2: std::arch::is_x86_feature_detected!("sse2"),
            sse41: std::arch::is_x86_feature_detected!("sse4.1"),
            avx: std::arch::is_x86_feature_detected!("avx"),
            avx2: std::arch::is_x86_feature_detected!("avx2"),
            f16c: std::arch::is_x86_feature_detected!("f16c"),
            fma: std::arch::is_x86_feature_detected!("fma"),
            bmi1: std::arch::is_x86_feature_detected!("bmi1"),
            bmi2: std::arch::is_x86_feature_detected!("bmi2"),
        }
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        CpuFeatureSet::default()
    }
}

fn discover_memory() -> MemoryCapabilities {
    let mut sys = System::new_all();
    sys.refresh_memory();
    MemoryCapabilities {
        total_mb: (sys.total_memory() > 0).then(|| sys.total_memory() / (1024 * 1024)),
    }
}

fn discover_storage() -> Vec<StorageCapabilities> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .map(|disk| StorageCapabilities {
            name: disk.name().to_string_lossy().into_owned(),
            mount_point: disk.mount_point().to_string_lossy().into_owned(),
            file_system: Some(disk.file_system().to_string_lossy().into_owned()),
            total_bytes: Some(disk.total_space()),
            available_bytes: Some(disk.available_space()),
            removable: Some(disk.is_removable()),
        })
        .collect()
}

fn derive_runtime_policy(
    environment: &HostEnvironmentSnapshot,
    capabilities: &HostCapabilities,
) -> RuntimeCapabilityPolicy {
    let preferred_gpu = capabilities.preferred_gpu();
    let hardware_gpu = capabilities.gpu.iter().any(|gpu| !gpu.is_software);
    let mut provider_hints = Vec::new();

    let mut render_tags = Vec::new();
    let mut forbidden_render_tags = Vec::new();
    match environment.os.as_str() {
        "windows" => {
            if preferred_gpu
                .and_then(|gpu| gpu.graphics_api.as_deref())
                .is_some_and(|api| api.contains("D3D12"))
            {
                render_tags.push("backend.d3d12".to_owned());
            }
            render_tags.push("backend.vulkan".to_owned());
            forbidden_render_tags.push("backend.metal".to_owned());
        }
        "linux" => {
            render_tags.push("backend.vulkan".to_owned());
            forbidden_render_tags.push("backend.d3d12".to_owned());
            forbidden_render_tags.push("backend.metal".to_owned());
        }
        "macos" => {
            render_tags.push("backend.metal".to_owned());
            render_tags.push("backend.vulkan".to_owned());
            forbidden_render_tags.push("backend.d3d12".to_owned());
        }
        _ => render_tags.push("backend.vulkan".to_owned()),
    }
    provider_hints.push(ProviderSelectionHint {
        gateway_id: "engine.render".to_owned(),
        preferred_system_tags: render_tags,
        forbidden_system_tags: forbidden_render_tags,
        preference_bonus: 2_000,
        reason: "host OS/hardware graphics API compatibility policy".to_owned(),
    });

    provider_hints.push(ProviderSelectionHint {
        gateway_id: "engine.input".to_owned(),
        preferred_system_tags: vec![format!("platform.{}", environment.os)],
        forbidden_system_tags: Vec::new(),
        preference_bonus: 1_000,
        reason: "native platform input preference".to_owned(),
    });

    RuntimeCapabilityPolicy {
        worker_threads: capabilities.cpu.logical_cores,
        preferred_gpu_stable_id: preferred_gpu.map(|gpu| gpu.stable_id.clone()),
        allow_software_rendering: !hardware_gpu,
        provider_hints,
    }
}

fn select_preferred_gpu(gpus: &[GpuCapabilities]) -> Option<usize> {
    let mut best_index = None;
    let mut best_score = (0_u8, 0_u64, 0_u32);
    for (slot, gpu) in gpus.iter().enumerate() {
        let hardware = u8::from(!gpu.is_software);
        let vram = gpu.dedicated_vram_mb.unwrap_or(0);
        let stable_tie_break = u32::MAX.saturating_sub(gpu.index);
        let score = (hardware, vram, stable_tie_break);
        if best_index.is_none() || score > best_score {
            best_index = Some(slot);
            best_score = score;
        }
    }
    best_index
}

fn discover_gpu() -> Vec<GpuCapabilities> {
    if let Some(fake) = fake_gpu_inventory_from_env() {
        return fake;
    }
    #[cfg(windows)]
    {
        return discover_windows_gpu();
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn fake_gpu_inventory_from_env() -> Option<Vec<GpuCapabilities>> {
    let raw = std::env::var("NEWENGINE_GPU_PROBE_FAKE").ok()?;
    let adapters = raw
        .split([';', '|'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .enumerate()
        .filter_map(|(index, entry)| {
            let (name, vram) = match entry.rsplit_once(':') {
                Some((name, vram)) => (name.trim(), vram.trim().parse::<u64>().ok()),
                None => (entry, None),
            };
            if name.is_empty() {
                return None;
            }
            let vram = vram.filter(|value| *value > 0);
            Some(GpuCapabilities {
                index: index as u32,
                name: name.to_owned(),
                dedicated_vram_mb: vram,
                is_discrete: vram.unwrap_or(0) > 0,
                stable_id: format!("fake:{index}:{}", stable_id_slug(name)),
                graphics_api: Some("fake".to_owned()),
                ..GpuCapabilities::default()
            })
        })
        .collect::<Vec<_>>();
    (!adapters.is_empty()).then_some(adapters)
}

fn stable_id_slug(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            out.push('-');
            previous_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "adapter".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(windows)]
fn discover_windows_gpu() -> Vec<GpuCapabilities> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1};
    let Ok(factory): Result<IDXGIFactory1, _> = (unsafe { CreateDXGIFactory1() }) else {
        return Vec::new();
    };
    let mut adapters = Vec::new();
    let mut index = 0_u32;
    loop {
        let adapter: IDXGIAdapter1 = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(_) => break,
        };
        let desc = match unsafe { adapter.GetDesc1() } {
            Ok(desc) => desc,
            Err(_) => {
                index = index.wrapping_add(1);
                continue;
            }
        };
        let name =
            wide_to_string(&desc.Description).unwrap_or_else(|| format!("DXGI Adapter {index}"));
        let dedicated_vram_mb = bytes_to_mb_option(desc.DedicatedVideoMemory as u64);
        let is_software = (desc.Flags & 0x2) != 0;
        adapters.push(GpuCapabilities {
            index,
            name,
            vendor_id: Some(desc.VendorId),
            device_id: Some(desc.DeviceId),
            subsystem_id: Some(desc.SubSysId),
            revision: Some(desc.Revision),
            dedicated_vram_mb,
            dedicated_system_mb: bytes_to_mb_option(desc.DedicatedSystemMemory as u64),
            shared_system_mb: bytes_to_mb_option(desc.SharedSystemMemory as u64),
            is_software,
            is_discrete: !is_software && dedicated_vram_mb.unwrap_or(0) > 0,
            stable_id: format!(
                "dxgi:{index}:vendor={:04x}:device={:04x}:subsys={:08x}:rev={:02x}",
                desc.VendorId, desc.DeviceId, desc.SubSysId, desc.Revision
            ),
            graphics_api: probe_d3d12_feature_level(&adapter)
                .map(|level| format!("D3D12 feature_level={level}")),
        });
        index = index.wrapping_add(1);
    }
    adapters
}

#[cfg(windows)]
fn probe_d3d12_feature_level(
    adapter: &windows::Win32::Graphics::Dxgi::IDXGIAdapter1,
) -> Option<&'static str> {
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D12::{D3D12CreateDevice, ID3D12Device};
    let levels = [
        (D3D_FEATURE_LEVEL_12_2, "12_2"),
        (D3D_FEATURE_LEVEL_12_1, "12_1"),
        (D3D_FEATURE_LEVEL_12_0, "12_0"),
        (D3D_FEATURE_LEVEL_11_1, "11_1"),
        (D3D_FEATURE_LEVEL_11_0, "11_0"),
    ];
    for (level, label) in levels {
        let mut device: Option<ID3D12Device> = None;
        if unsafe { D3D12CreateDevice(adapter, level, &mut device).is_ok() } && device.is_some() {
            return Some(label);
        }
    }
    None
}

#[cfg(windows)]
fn wide_to_string(value: &[u16]) -> Option<String> {
    let end = value.iter().position(|ch| *ch == 0).unwrap_or(value.len());
    let value = String::from_utf16_lossy(&value[..end]).trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn bytes_to_mb_option(bytes: u64) -> Option<u64> {
    (bytes > 0).then(|| bytes / (1024 * 1024))
}

#[cfg(windows)]
fn discover_displays() -> Vec<DisplayCapabilities> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CMONITORS, SM_CXSCREEN, SM_CYSCREEN,
    };
    let count = unsafe { GetSystemMetrics(SM_CMONITORS) }.max(1) as u32;
    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(0) as u32;
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(0) as u32;
    (0..count)
        .map(|index| DisplayCapabilities {
            index,
            name: (index == 0).then(|| "primary".to_owned()),
            primary: index == 0,
            width_px: (index == 0 && width > 0).then_some(width),
            height_px: (index == 0 && height > 0).then_some(height),
            refresh_rate_millihz: None,
            hdr_capable: None,
        })
        .collect()
}

#[cfg(not(windows))]
fn discover_displays() -> Vec<DisplayCapabilities> {
    Vec::new()
}

#[cfg(windows)]
fn discover_input() -> InputCapabilities {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CMOUSEBUTTONS, SM_MOUSEPRESENT, SM_MOUSEWHEELPRESENT,
    };
    let mouse_present = unsafe { GetSystemMetrics(SM_MOUSEPRESENT) } != 0;
    let mouse_buttons = unsafe { GetSystemMetrics(SM_CMOUSEBUTTONS) }.max(0) as u32;
    let mouse_wheel = unsafe { GetSystemMetrics(SM_MOUSEWHEELPRESENT) } != 0;
    InputCapabilities {
        keyboard_present: Some(true),
        mouse_present: Some(mouse_present),
        mouse_buttons: Some(mouse_buttons),
        mouse_wheel_present: Some(mouse_wheel),
        touch_present: None,
    }
}

#[cfg(not(windows))]
fn discover_input() -> InputCapabilities {
    InputCapabilities {
        keyboard_present: None,
        mouse_present: None,
        mouse_buttons: None,
        mouse_wheel_present: None,
        touch_present: None,
    }
}

fn opt_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "<unknown>".to_owned())
}

fn opt_bool(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "1",
        Some(false) => "0",
        None => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preinit_discovery_produces_versioned_snapshot() {
        let snapshot = discover_preinit_snapshot();
        assert_eq!(snapshot.schema_version, HOST_CAPABILITIES_SCHEMA_VERSION);
        assert!(!snapshot.environment.os.is_empty());
        assert!(!snapshot.environment.arch.is_empty());
        assert!(snapshot.environment.pid > 0);
        assert!(snapshot.capabilities.cpu.logical_cores.unwrap_or(1) >= 1);
        if let Some(index) = snapshot.capabilities.preferred_gpu_index {
            assert!(index < snapshot.capabilities.gpu.len());
        }
    }

    #[test]
    fn preferred_gpu_chooses_discrete_vram() {
        let gpus = vec![
            GpuCapabilities {
                index: 0,
                name: "Integrated".to_owned(),
                stable_id: "a".to_owned(),
                ..Default::default()
            },
            GpuCapabilities {
                index: 1,
                name: "Discrete".to_owned(),
                stable_id: "b".to_owned(),
                dedicated_vram_mb: Some(8192),
                is_discrete: true,
                ..Default::default()
            },
        ];
        assert_eq!(select_preferred_gpu(&gpus), Some(1));
    }

    #[test]
    fn runtime_policy_is_immutable_data_derived_from_snapshot() {
        let environment = HostEnvironmentSnapshot {
            os: "windows".to_owned(),
            ..Default::default()
        };
        let capabilities = HostCapabilities {
            cpu: CpuCapabilities {
                logical_cores: Some(20),
                ..Default::default()
            },
            gpu: vec![GpuCapabilities {
                index: 0,
                stable_id: "gpu0".to_owned(),
                graphics_api: Some("D3D12 feature_level=12_2".to_owned()),
                ..Default::default()
            }],
            preferred_gpu_index: Some(0),
            ..Default::default()
        };
        let policy = derive_runtime_policy(&environment, &capabilities);
        assert_eq!(policy.worker_threads, Some(20));
        assert_eq!(policy.preferred_gpu_stable_id.as_deref(), Some("gpu0"));
        let render = policy
            .provider_hints
            .iter()
            .find(|hint| hint.gateway_id == "engine.render")
            .expect("render policy");
        assert!(render
            .preferred_system_tags
            .iter()
            .any(|tag| tag == "backend.d3d12"));
        assert!(render
            .forbidden_system_tags
            .iter()
            .any(|tag| tag == "backend.metal"));
    }
}
