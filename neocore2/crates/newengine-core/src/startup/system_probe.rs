#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

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
    const TABLE_WIDTH: usize = 47;

    #[inline]
    pub fn probe() -> Self {
        let mut out = Self::default();

        let (os, cpu, cores, ram_mb) = probe_sysinfo();
        out.os = os;
        out.cpu = cpu;
        out.cpu_cores_logical = cores;
        out.ram_total_mb = ram_mb;

        #[cfg(windows)]
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

        log::info!("┌{}", "─".repeat(Self::TABLE_WIDTH));
        log::info!("│ {}", title);
        log::info!("├{}", "─".repeat(Self::TABLE_WIDTH));

        self.emit_row("run_tag", crate::run_id::run_tag());
        self.emit_row("run_id", crate::run_id::run_id());

        self.emit_row("os", self.os.as_deref());
        self.emit_row("cpu", self.cpu.as_deref());
        self.emit_row(
            "cpu_cores_logical",
            self.cpu_cores_logical
                .map(|v| v.to_string())
                .as_deref(),
        );
        self.emit_row(
            "ram_total_mb",
            self.ram_total_mb.map(|v| v.to_string()).as_deref(),
        );
        self.emit_row("gpu", self.gpu.as_deref());
        self.emit_row(
            "vram_dedicated_mb",
            self.vram_dedicated_mb
                .map(|v| v.to_string())
                .as_deref(),
        );
        self.emit_row("directx", self.directx.as_deref());

        log::info!("└{}", "─".repeat(Self::TABLE_WIDTH));
    }

    fn emit_row(&self, key: &str, value: Option<&str>) {
        let value = value
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("<unknown>");

        log::info!("│   {:<18} : {}", key, value);
    }
}

fn probe_sysinfo() -> (Option<String>, Option<String>, Option<u32>, Option<u64>) {
    use sysinfo::System;

    let mut sys = System::new_all();
    sys.refresh_all();

    let os = System::long_os_version()
        .or_else(System::name)
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());

    let cpu = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_owned())
        .filter(|s| !s.is_empty());

    let cores = Some(sys.cpus().len() as u32);

    let ram_mb = Some(normalize_mem_to_mb(sys.total_memory() as u64));

    (os, cpu, cores, ram_mb)
}

#[cfg(windows)]
#[derive(Clone, Debug)]
struct WinDxInfo {
    gpu: Option<String>,
    vram_dedicated_mb: Option<u64>,
    directx: Option<String>,
}

fn normalize_mem_to_mb(raw: u64) -> u64 {
    if raw >= 1_000_000_000 {
        raw / (1024 * 1024)
    } else {
        raw / 1024
    }
}

#[cfg(windows)]
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

        let desc = unsafe { adapter.GetDesc1().ok()? };

        let dedicated = desc.DedicatedVideoMemory as u64;
        if dedicated > best_vram {
            best_vram = dedicated;
            best_name = wide_to_string(&desc.Description);
            best_adapter = Some(adapter);
        }

        index = index.wrapping_add(1);
    }

    let gpu = best_name;
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
        gpu,
        vram_dedicated_mb,
        directx,
    })
}

#[cfg(windows)]
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

#[cfg(windows)]
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

#[cfg(windows)]
fn wide_to_string(wide: &[u16]) -> Option<String> {
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    let s = String::from_utf16_lossy(&wide[..end]).trim().to_owned();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}