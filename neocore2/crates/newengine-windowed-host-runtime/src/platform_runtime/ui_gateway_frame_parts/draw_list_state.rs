use super::*;

static UI_FRAME_EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

#[inline]
pub(crate) fn ui_frame_now_ms() -> u64 {
    UI_FRAME_EPOCH
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}
