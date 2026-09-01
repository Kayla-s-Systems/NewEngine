use super::*;

#[derive(Debug)]
pub(super) struct CachedClip {
    pub(super) bytes: Arc<[u8]>,
    pub(super) source_duration: OnceLock<Option<Duration>>,
}

impl CachedClip {
    #[inline]
    pub(super) fn len(&self) -> usize {
        self.bytes.len()
    }
}

#[derive(Clone, Debug)]
pub(super) struct EmbeddedYscdClipLocator {
    pub(super) dictionary_path: String,
    pub(super) cue_name: String,
    pub(super) clip_index: usize,
}

#[derive(Clone, Debug)]
pub(super) struct YscdRuntimeLayer {
    pub(super) name: String,
    pub(super) role: String,
    pub(super) clips: Vec<SoundCueClip>,
    pub(super) gain: f32,
    pub(super) pitch: f32,
    pub(super) attenuation: Option<AudioAttenuationSettings>,
}

#[derive(Clone, Debug)]
pub(super) struct YscdRuntimeMeta {
    pub(super) dictionary_path: String,
    pub(super) cue_name: String,
    pub(super) embedded_bytes: usize,
}

