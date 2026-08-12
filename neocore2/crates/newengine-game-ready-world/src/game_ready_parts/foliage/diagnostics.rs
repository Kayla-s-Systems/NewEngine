#[inline]
pub(super) fn log_foliage_prefab_placement(
    prefab_id: &str,
    source: &str,
    proxy: &str,
    mode: &str,
    parts: usize,
    placed: usize,
    max_count: u32,
    grid_min: i32,
    grid_max: i32,
    spacing: f32,
) {
    newengine_ulog_api::ulog::info!(
        "game-ready: foliage prefab placement prefab='{}' source='{}' proxy='{}' mode='{}' parts={} placed={} max_count={} grid={}..{} spacing={:.2}",
        prefab_id,
        source,
        proxy,
        mode,
        parts,
        placed,
        max_count,
        grid_min,
        grid_max,
        spacing,
    );
}
