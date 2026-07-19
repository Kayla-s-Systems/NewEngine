mod browser;
mod opening;
mod preview_entries;
mod startup;

#[cfg(test)]
pub(super) use browser::prepend_parent_navigation;

#[inline]
fn activation_is_deferred(requested_frame: u64, frame_index: u64) -> bool {
    requested_frame >= frame_index
}

fn log_preview_open_timing(
    target: &str,
    asset_ref: &str,
    inspect_ms: f64,
    preview_ms: f64,
    document_cache_hit: bool,
    preview_cache_hit: bool,
) {
    newengine_ulog_api::ulog::info!(
        "asset inspector: preview {} timing ref='{}' inspect_ms={:.3} preview_request_ms={:.3} document_cache_hit={} preview_cache_hit={}",
        target,
        asset_ref,
        inspect_ms,
        preview_ms,
        document_cache_hit,
        preview_cache_hit,
    );
}
