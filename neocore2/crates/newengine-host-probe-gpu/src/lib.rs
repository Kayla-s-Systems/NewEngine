#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_host_capabilities_api::GpuCapabilities;

/// Discover only GPU adapters and their graphics API compatibility.
pub fn discover() -> Vec<GpuCapabilities> {
    if let Some(fake) = fake_inventory_from_env() {
        return fake;
    }
    #[cfg(windows)]
    {
        discover_windows()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Select the preferred adapter from a completed GPU inventory.
pub fn select_preferred(gpus: &[GpuCapabilities]) -> Option<usize> {
    let mut best_index = None;
    let mut best_score = (0_u8, 0_u64, 0_u32);
    for (slot, gpu) in gpus.iter().enumerate() {
        let score = (
            u8::from(!gpu.is_software),
            gpu.dedicated_vram_mb.unwrap_or(0),
            u32::MAX.saturating_sub(gpu.index),
        );
        if best_index.is_none() || score > best_score {
            best_index = Some(slot);
            best_score = score;
        }
    }
    best_index
}

pub fn preferred_reason(gpus: &[GpuCapabilities], index: Option<usize>) -> Option<String> {
    index.and_then(|index| {
        gpus.get(index).map(|adapter| {
            if adapter.is_software {
                "fallback_software_adapter".to_owned()
            } else if adapter.dedicated_vram_mb.unwrap_or(0) > 0 {
                "highest_score_discrete_dedicated_vram".to_owned()
            } else {
                "first_hardware_adapter".to_owned()
            }
        })
    })
}

fn fake_inventory_from_env() -> Option<Vec<GpuCapabilities>> {
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
fn discover_windows() -> Vec<GpuCapabilities> {
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

#[cfg(windows)]
fn bytes_to_mb_option(bytes: u64) -> Option<u64> {
    (bytes > 0).then_some(bytes / (1024 * 1024))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(select_preferred(&gpus), Some(1));
    }
}
