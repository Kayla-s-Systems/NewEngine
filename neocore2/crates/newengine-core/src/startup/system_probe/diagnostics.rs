use super::{GpuAdapterInfo, SystemProbe};

use crate::log_fmt::emit_boxed_kv;

impl SystemProbe {
pub fn emit_table(&self, stage: &str) {
    let title = format!("SystemProbe :: Host [{}]", stage);
    let mut rows: Vec<(String, String)> = vec![
        (
            "run_tag".to_owned(),
            crate::run_id::run_tag().unwrap_or("<unknown>").to_owned(),
        ),
        (
            "run_id".to_owned(),
            crate::run_id::run_id().unwrap_or("<unknown>").to_owned(),
        ),
        ("os".to_owned(), self.value_or_unknown(self.os.as_deref())),
        ("cpu".to_owned(), self.value_or_unknown(self.cpu.as_deref())),
        (
            "cpu_cores_logical".to_owned(),
            self.cpu_cores_logical
                .map(|v| v.to_string())
                .unwrap_or_else(|| "<unknown>".to_owned()),
        ),
        (
            "ram_total_mb".to_owned(),
            self.ram_total_mb
                .map(|v| v.to_string())
                .unwrap_or_else(|| "<unknown>".to_owned()),
        ),
        (
            "gpu".to_owned(),
            self.value_or_unknown(self.primary_gpu_name()),
        ),
        (
            "vram_dedicated_mb".to_owned(),
            self.primary_vram_dedicated_mb()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "<unknown>".to_owned()),
        ),
        (
            "directx".to_owned(),
            self.value_or_unknown(self.primary_directx()),
        ),
        ("gpu_count".to_owned(), self.gpu_count().to_string()),
        (
            "gpu_primary".to_owned(),
            self.value_or_unknown(self.primary_gpu_summary().as_deref()),
        ),
    ];

    for adapter in &self.gpu_inventory.adapters {
        rows.push((
            format!("gpu[{}]", adapter.index),
            format_gpu_adapter_row(adapter),
        ));
    }

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

fn format_gpu_adapter_row(adapter: &GpuAdapterInfo) -> String {
    format!(
        "name='{}' stable_id='{}' vendor={} device={} vram_dedicated_mb={} shared_system_mb={} type={} directx={}",
        adapter.name,
        adapter.stable_id,
        format_hex_u32(adapter.vendor_id, 4),
        format_hex_u32(adapter.device_id, 4),
        format_optional_u64(adapter.dedicated_vram_mb),
        format_optional_u64(adapter.shared_system_mb),
        adapter.kind_label(),
        adapter.directx.as_deref().unwrap_or("<unknown>")
    )
}

fn format_optional_u64(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "<unknown>".to_owned())
}

fn format_hex_u32(value: Option<u32>, width: usize) -> String {
    value
        .map(|v| format!("0x{v:0width$x}", width = width))
        .unwrap_or_else(|| "<unknown>".to_owned())
}
