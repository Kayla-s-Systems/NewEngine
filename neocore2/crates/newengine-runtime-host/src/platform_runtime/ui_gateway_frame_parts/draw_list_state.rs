use super::*;

const LOADING_TEXTURE_SESSION_RESET_MAX_FRAME: u64 = 4;

static LOADING_ANIMATION_EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

#[derive(Default)]
struct LoadingTextureResidencyState {
    last_frame_index: Option<u64>,
    resident_refs: BTreeMap<u32, String>,
}

static LOADING_TEXTURE_RESIDENCY: std::sync::OnceLock<
    std::sync::Mutex<LoadingTextureResidencyState>,
> = std::sync::OnceLock::new();

#[inline]
pub(crate) fn loading_animation_now_ms() -> u64 {
    LOADING_ANIMATION_EPOCH
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn loading_texture_residency() -> &'static std::sync::Mutex<LoadingTextureResidencyState> {
    LOADING_TEXTURE_RESIDENCY
        .get_or_init(|| std::sync::Mutex::new(LoadingTextureResidencyState::default()))
}

fn lock_loading_texture_residency() -> std::sync::MutexGuard<'static, LoadingTextureResidencyState>
{
    loading_texture_residency()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn reset_loading_texture_session() {
    let mut state = lock_loading_texture_residency();
    state.last_frame_index = None;
    state.resident_refs.clear();
}

pub(super) fn begin_loading_texture_frame(frame_index: u64) {
    let mut state = lock_loading_texture_residency();
    let starts_new_session = state.last_frame_index.is_some_and(|last| {
        frame_index < last
            || (frame_index <= LOADING_TEXTURE_SESSION_RESET_MAX_FRAME
                && last > LOADING_TEXTURE_SESSION_RESET_MAX_FRAME)
    });
    if starts_new_session {
        state.resident_refs.clear();
    }
    state.last_frame_index = Some(frame_index);
}

pub(super) fn loading_texture_is_resident(texture_id: UiTexId, texture_ref: &str) -> bool {
    lock_loading_texture_residency()
        .resident_refs
        .get(&texture_id.0)
        .is_some_and(|resident_ref| resident_ref == texture_ref)
}

pub(super) fn mark_loading_texture_resident(texture_id: UiTexId, texture_ref: &str) {
    lock_loading_texture_residency()
        .resident_refs
        .insert(texture_id.0, texture_ref.to_owned());
}
