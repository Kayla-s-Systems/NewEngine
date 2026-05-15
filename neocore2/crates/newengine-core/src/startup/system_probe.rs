#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

use crate::log_fmt::emit_boxed_kv;

#[derive(Clone, Debug, Default)]
pub struct SystemProbe {
    pub os: Option<String>,
    pub cpu: Option<String>,
    pub cpu_cores_logical: Option<u32>,
    pub ram_total_mb: Option<u64>,
    pub gpu: Option<String>,
    pub vram_dedicated_mb: Option<u64>,
    pub directx: Option<String>,
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

        #[cfg(all(windows, feature = "host-probe"))]
        {
            if let Some(dx) = probe_windows_dxgi_d3d12() {
                out.gpu = dx.gpu;
                out.vram_dedicated_mb = dx.vram_dedicated_mb;
                out.directx = dx.directx;
            }
        }

        out
    }

    pub fn emit_table(&self, stage: &str) {
        let title = format!("SystemProbe :: Host [{}]", stage);
        let rows = vec![
            ("run_tag", crate::run_id::run_tag().unwrap_or("<unknown>").to_owned()),
            ("run_id", crate::run_id::run_id().unwrap_or("<unknown>").to_owned()),
            ("os", self.value_or_unknown(self.os.as_deref())),
            ("cpu", self.value_or_unknown(self.cpu.as_deref())),
            (
                "cpu_cores_logical",
                self.cpu_cores_logical
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "<unknown>".to_owned()),
            ),
            (
                "ram_total_mb",
                self.ram_total_mb
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "<unknown>".to_owned()),
            ),
            ("gpu", self.value_or_unknown(self.gpu.as_deref())),
            (
                "vram_dedicated_mb",
                self.vram_dedicated_mb
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "<unknown>".to_owned()),
            ),
            ("directx", self.value_or_unknown(self.directx.as_deref())),
        ];

        emit_boxed_kv(&title, &rows);
    }

    #[inline]
    fn value_or_unknown(&self, value: Option<&str>) -> String {
        value
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("<unknown>")
            .to_owned()
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
        Some(format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)),
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

    let ram_mb = match sys.total_memory() as u64 {
        0 => probe_memory_mb_fallback(),
        raw => Some(normalize_mem_to_mb(raw)),
    };

    (os, cpu, cores, ram_mb)
}

#[cfg(not(feature = "host-probe"))]
fn probe_sysinfo() -> (Option<String>, Option<String>, Option<u32>, Option<u64>) {
    let os = Some(format!("{} {}", std::env::consts::OS, std::env::consts::ARCH));
    let cpu = first_non_empty([
        std::env::var("PROCESSOR_IDENTIFIER").ok(),
        std::env::var("PROCESSOR_ARCHITECTURE").ok(),
    ]);
    let cores = std::thread::available_parallelism().ok().map(|n| n.get() as u32);
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
    gpu: Option<String>,
    vram_dedicated_mb: Option<u64>,
    directx: Option<String>,
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

    let mut best_adapter: Option<IDXGIAdapter1> = None;
    let mut best_vram: u64 = 0;
    let mut best_name: Option<String> = None;

    let mut index: u32 = 0;
    loop {
        let adapter: IDXGIAdapter1 = match unsafe { factory.EnumAdapters1(index) } {
            Ok(a) => a,
            Err(_) => break,
        };

        let desc = match unsafe { adapter.GetDesc1() } {
            Ok(desc) => desc,
            Err(err) => {
                log::debug!(
                    "system probe: dxgi adapter desc failed index={} err='{}'",
                    index,
                    err
                );
                index = index.wrapping_add(1);
                continue;
            }
        };

        let dedicated = desc.DedicatedVideoMemory as u64;
        let name = wide_to_string(&desc.Description);

        if best_name.is_none() || dedicated > best_vram {
            best_vram = dedicated;
            best_name = name;
            best_adapter = Some(adapter);
        }

        index = index.wrapping_add(1);
    }

    let vram_dedicated_mb = if best_vram > 0 {
        Some(best_vram / (1024 * 1024))
    } else {
        None
    };

    let directx = best_adapter
        .as_ref()
        .and_then(probe_d3d12_feature_level)
        .map(|fl| format!("D3D12 feature_level={}", feature_level_str(fl)));

    Some(WinDxInfo {
        gpu: best_name,
        vram_dedicated_mb,
        directx,
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
