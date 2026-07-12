#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

mod diagnostics;

#[derive(Clone, Debug, Default)]
pub struct SystemProbe {
    pub os: Option<String>,
    pub cpu: Option<String>,
    pub cpu_cores_logical: Option<u32>,
    pub ram_total_mb: Option<u64>,

    // GPU topology is a single source of truth. The startup table keeps the
    // historical row names `gpu`, `vram_dedicated_mb`, and `directx`, but those
    // values are projected from `gpu_inventory.primary_adapter()` instead of
    // being duplicated as legacy state.
    pub gpu_inventory: GpuInventory,
}

#[derive(Clone, Debug, Default)]
pub struct GpuInventory {
    pub adapters: Vec<GpuAdapterInfo>,
    pub primary_index: Option<usize>,
    pub primary_reason: Option<String>,
}

impl GpuInventory {
    pub fn from_adapters(mut adapters: Vec<GpuAdapterInfo>) -> Self {
        adapters.sort_by_key(|adapter| adapter.index);

        let primary_index = select_primary_adapter_index(&adapters);
        let primary_reason = primary_index
            .and_then(|index| adapters.get(index))
            .map(|adapter| {
                if adapter.is_software {
                    "fallback_software_adapter".to_owned()
                } else if adapter.dedicated_vram_mb.unwrap_or(0) > 0 {
                    "highest_score_discrete_dedicated_vram".to_owned()
                } else {
                    "first_hardware_adapter".to_owned()
                }
            });

        Self {
            adapters,
            primary_index,
            primary_reason,
        }
    }

    #[inline]
    pub fn adapter_count(&self) -> usize {
        self.adapters.len()
    }

    #[inline]
    pub fn primary_adapter(&self) -> Option<&GpuAdapterInfo> {
        self.primary_index
            .and_then(|index| self.adapters.get(index))
    }
}

#[derive(Clone, Debug, Default)]
pub struct GpuAdapterInfo {
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
    pub directx: Option<String>,
}

impl GpuAdapterInfo {
    #[inline]
    pub fn kind_label(&self) -> &'static str {
        if self.is_software {
            "software"
        } else if self.is_discrete {
            "discrete"
        } else {
            "integrated_or_shared"
        }
    }
}

impl SystemProbe {
    #[inline]
    pub fn probe() -> Self {
        let mut out = Self::default();

        let (os, cpu, cores, ram_mb) = probe_sysinfo();
        out.os = os;
        out.cpu = cpu;
        out.cpu_cores_logical = cores;
        out.ram_total_mb = ram_mb;

        if let Some(fake_inventory) = probe_fake_gpu_inventory_from_env() {
            out.apply_gpu_inventory(fake_inventory);
            return out;
        }

        #[cfg(all(windows, feature = "host-probe"))]
        {
            if let Some(dx) = probe_windows_dxgi_d3d12() {
                out.apply_gpu_inventory(dx.gpu_inventory);
            }
        }

        out
    }

    #[inline]
    pub fn gpu_count(&self) -> usize {
        self.gpu_inventory.adapter_count()
    }

    #[inline]
    pub fn primary_gpu(&self) -> Option<&GpuAdapterInfo> {
        self.gpu_inventory.primary_adapter()
    }

    #[inline]
    pub fn primary_gpu_name(&self) -> Option<&str> {
        self.primary_gpu().map(|adapter| adapter.name.as_str())
    }

    #[inline]
    pub fn primary_vram_dedicated_mb(&self) -> Option<u64> {
        self.primary_gpu()
            .and_then(|adapter| adapter.dedicated_vram_mb)
    }

    #[inline]
    pub fn primary_directx(&self) -> Option<&str> {
        self.primary_gpu()
            .and_then(|adapter| adapter.directx.as_deref())
    }

    #[inline]
    pub fn primary_gpu_summary(&self) -> Option<String> {
        let primary = self.primary_gpu()?;
        Some(format!(
            "index={} stable_id='{}' reason='{}'",
            primary.index,
            primary.stable_id,
            self.gpu_inventory
                .primary_reason
                .as_deref()
                .unwrap_or("primary_adapter_selected")
        ))
    }

    #[inline]
    fn apply_gpu_inventory(&mut self, inventory: GpuInventory) {
        self.gpu_inventory = inventory;
    }
}

#[cfg(feature = "host-probe")]
fn probe_sysinfo() -> (Option<String>, Option<String>, Option<u32>, Option<u64>) {
    use sysinfo::System;

    let mut sys = System::new_all();
    sys.refresh_all();

    let os = first_non_empty([
        System::long_os_version(),
        System::name(),
        Some(format!(
            "{} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )),
    ]);

    let cpu = first_non_empty([
        sys.cpus().first().map(|c| c.brand().to_owned()),
        std::env::var("PROCESSOR_IDENTIFIER").ok(),
        std::env::var("PROCESSOR_ARCHITECTURE").ok(),
    ]);

    let cores = match sys.cpus().len() {
        0 => std::thread::available_parallelism()
            .ok()
            .map(|n| n.get() as u32),
        n => Some(n as u32),
    };

    let ram_mb = match sys.total_memory() {
        0 => probe_memory_mb_fallback(),
        raw => Some(normalize_mem_to_mb(raw)),
    };

    (os, cpu, cores, ram_mb)
}

#[cfg(not(feature = "host-probe"))]
fn probe_sysinfo() -> (Option<String>, Option<String>, Option<u32>, Option<u64>) {
    let os = Some(format!(
        "{} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    let cpu = first_non_empty([
        std::env::var("PROCESSOR_IDENTIFIER").ok(),
        std::env::var("PROCESSOR_ARCHITECTURE").ok(),
    ]);
    let cores = std::thread::available_parallelism()
        .ok()
        .map(|n| n.get() as u32);
    let ram_mb = probe_memory_mb_fallback();
    (os, cpu, cores, ram_mb)
}

fn first_non_empty<const N: usize>(values: [Option<String>; N]) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_owned())
        .find(|value| !value.is_empty())
}

#[cfg(all(windows, feature = "host-probe"))]
#[derive(Clone, Debug)]
struct WinDxInfo {
    gpu_inventory: GpuInventory,
}

#[cfg(feature = "host-probe")]
fn normalize_mem_to_mb(raw: u64) -> u64 {
    // sysinfo has used different memory units across major versions. Current
    // versions report bytes; older builds reported KiB. Normalize both shapes
    // so startup diagnostics stay stable across plugin/runtime toolchains.
    if raw >= 1_000_000_000 {
        raw / (1024 * 1024)
    } else {
        raw / 1024
    }
}

fn probe_memory_mb_fallback() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("MemTotal:") else {
                continue;
            };
            let kb = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())?;
            return Some(kb / 1024);
        }
    }

    None
}

#[cfg(all(windows, feature = "host-probe"))]
fn probe_windows_dxgi_d3d12() -> Option<WinDxInfo> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1};

    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1().ok()? };
    let mut adapters: Vec<GpuAdapterInfo> = Vec::new();

    let mut index: u32 = 0;
    loop {
        let adapter: IDXGIAdapter1 = match unsafe { factory.EnumAdapters1(index) } {
            Ok(a) => a,
            Err(_) => break,
        };

        let desc = match unsafe { adapter.GetDesc1() } {
            Ok(desc) => desc,
            Err(err) => {
                newengine_ulog_api::ulog::debug!(
                    "system probe: dxgi adapter desc failed index={} err='{}'",
                    index,
                    err
                );
                index = index.wrapping_add(1);
                continue;
            }
        };

        let name =
            wide_to_string(&desc.Description).unwrap_or_else(|| format!("DXGI Adapter {}", index));
        let dedicated_vram_mb = bytes_to_mb_option(desc.DedicatedVideoMemory as u64);
        let dedicated_system_mb = bytes_to_mb_option(desc.DedicatedSystemMemory as u64);
        let shared_system_mb = bytes_to_mb_option(desc.SharedSystemMemory as u64);
        let is_software = dxgi_flags_include_software(desc.Flags);
        let is_discrete = !is_software && dedicated_vram_mb.unwrap_or(0) > 0;
        let directx = probe_d3d12_feature_level(&adapter)
            .map(|fl| format!("D3D12 feature_level={}", feature_level_str(fl)));

        adapters.push(GpuAdapterInfo {
            index,
            name,
            vendor_id: Some(desc.VendorId),
            device_id: Some(desc.DeviceId),
            subsystem_id: Some(desc.SubSysId),
            revision: Some(desc.Revision),
            dedicated_vram_mb,
            dedicated_system_mb,
            shared_system_mb,
            is_software,
            is_discrete,
            stable_id: make_dxgi_stable_id(
                index,
                desc.VendorId,
                desc.DeviceId,
                desc.SubSysId,
                desc.Revision,
            ),
            directx,
        });

        index = index.wrapping_add(1);
    }

    Some(WinDxInfo {
        gpu_inventory: GpuInventory::from_adapters(adapters),
    })
}

#[cfg(all(windows, feature = "host-probe"))]
fn probe_d3d12_feature_level(
    adapter: &windows::Win32::Graphics::Dxgi::IDXGIAdapter1,
) -> Option<windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL> {
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D12::{D3D12CreateDevice, ID3D12Device};

    let levels = [
        D3D_FEATURE_LEVEL_12_2,
        D3D_FEATURE_LEVEL_12_1,
        D3D_FEATURE_LEVEL_12_0,
        D3D_FEATURE_LEVEL_11_1,
        D3D_FEATURE_LEVEL_11_0,
    ];

    for lvl in levels {
        let mut dev: Option<ID3D12Device> = None;
        let ok = unsafe { D3D12CreateDevice(adapter, lvl, &mut dev).is_ok() } && dev.is_some();
        if ok {
            return Some(lvl);
        }
    }

    None
}

#[cfg(all(windows, feature = "host-probe"))]
fn feature_level_str(fl: windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL) -> &'static str {
    use windows::Win32::Graphics::Direct3D::*;
    match fl {
        D3D_FEATURE_LEVEL_12_2 => "12_2",
        D3D_FEATURE_LEVEL_12_1 => "12_1",
        D3D_FEATURE_LEVEL_12_0 => "12_0",
        D3D_FEATURE_LEVEL_11_1 => "11_1",
        D3D_FEATURE_LEVEL_11_0 => "11_0",
        _ => "unknown",
    }
}

#[cfg(all(windows, feature = "host-probe"))]
fn wide_to_string(wide: &[u16]) -> Option<String> {
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    let s = String::from_utf16_lossy(&wide[..end]).trim().to_owned();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(all(windows, feature = "host-probe"))]
fn dxgi_flags_include_software(flags: u32) -> bool {
    // DXGI_ADAPTER_FLAG_SOFTWARE = 2. Avoid depending on flag newtype shape
    // across windows-rs versions; we only need a stable diagnostic boolean.
    (flags & 0x2) != 0
}

#[cfg(all(windows, feature = "host-probe"))]
fn make_dxgi_stable_id(
    index: u32,
    vendor_id: u32,
    device_id: u32,
    subsystem_id: u32,
    revision: u32,
) -> String {
    format!(
        "dxgi:{index}:vendor={vendor_id:04x}:device={device_id:04x}:subsys={subsystem_id:08x}:rev={revision:02x}"
    )
}

fn bytes_to_mb_option(bytes: u64) -> Option<u64> {
    if bytes == 0 {
        None
    } else {
        Some(bytes / (1024 * 1024))
    }
}

fn select_primary_adapter_index(adapters: &[GpuAdapterInfo]) -> Option<usize> {
    let mut best_index: Option<usize> = None;
    let mut best_score: (u8, u64, u32) = (0, 0, u32::MAX);

    for (slot, adapter) in adapters.iter().enumerate() {
        let hardware_score = if adapter.is_software { 0 } else { 1 };
        let vram_score = adapter.dedicated_vram_mb.unwrap_or(0);
        let stable_tie_break = u32::MAX.saturating_sub(adapter.index);
        let score = (hardware_score, vram_score, stable_tie_break);

        if best_index.is_none() || score > best_score {
            best_index = Some(slot);
            best_score = score;
        }
    }

    best_index
}

fn probe_fake_gpu_inventory_from_env() -> Option<GpuInventory> {
    let raw = std::env::var("NEWENGINE_GPU_PROBE_FAKE").ok()?;
    parse_fake_gpu_inventory(&raw)
}

fn parse_fake_gpu_inventory(raw: &str) -> Option<GpuInventory> {
    let mut adapters = Vec::new();

    for (index, raw_entry) in raw
        .split([';', '|'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .enumerate()
    {
        let (name, dedicated_vram_mb) = match raw_entry.rsplit_once(':') {
            Some((raw_name, raw_vram)) => {
                let parsed_vram = raw_vram.trim().parse::<u64>().ok();
                (raw_name.trim(), parsed_vram)
            }
            None => (raw_entry, None),
        };

        if name.is_empty() {
            continue;
        }

        let index = index as u32;
        let dedicated_vram_mb = dedicated_vram_mb.filter(|value| *value > 0);
        let is_discrete = dedicated_vram_mb.unwrap_or(0) > 0;

        adapters.push(GpuAdapterInfo {
            index,
            name: name.to_owned(),
            vendor_id: None,
            device_id: None,
            subsystem_id: None,
            revision: None,
            dedicated_vram_mb,
            dedicated_system_mb: None,
            shared_system_mb: None,
            is_software: false,
            is_discrete,
            stable_id: format!("fake:{index}:{}", stable_id_slug(name)),
            directx: Some("fake".to_owned()),
        });
    }

    if adapters.is_empty() {
        None
    } else {
        Some(GpuInventory::from_adapters(adapters))
    }
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

    let trimmed = out.trim_matches('-').to_owned();
    if trimmed.is_empty() {
        "adapter".to_owned()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_gpu_inventory_parses_multiple_adapters() {
        let inventory = parse_fake_gpu_inventory(
            "NVIDIA GeForce GTX 1080 Ti:11107;NVIDIA GeForce GTX 1080 Ti:11107;NVIDIA GeForce GTX 1080 Ti:11107",
        )
        .expect("fake inventory must parse");

        assert_eq!(inventory.adapter_count(), 3);
        assert_eq!(inventory.primary_adapter().unwrap().index, 0);
        assert_eq!(
            inventory.primary_adapter().unwrap().dedicated_vram_mb,
            Some(11107)
        );
    }

    #[test]
    fn primary_adapter_prefers_hardware_vram_over_index() {
        let inventory = GpuInventory::from_adapters(vec![
            GpuAdapterInfo {
                index: 0,
                name: "Integrated".to_owned(),
                stable_id: "fake:0:integrated".to_owned(),
                directx: Some("fake".to_owned()),
                ..GpuAdapterInfo::default()
            },
            GpuAdapterInfo {
                index: 1,
                name: "Discrete".to_owned(),
                dedicated_vram_mb: Some(8192),
                is_discrete: true,
                stable_id: "fake:1:discrete".to_owned(),
                directx: Some("fake".to_owned()),
                ..GpuAdapterInfo::default()
            },
        ]);

        assert_eq!(inventory.primary_adapter().unwrap().name, "Discrete");
    }
}
