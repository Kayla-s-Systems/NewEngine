const DEFAULT_LOADING_SPINNER_RPS: f32 = 0.90;

#[derive(Debug, Clone)]
pub(super) struct LoadingSpinnerAnimationSpec {
    pub(super) rotation_rps: f32,
    pub(super) sprite_fps: f32,
    pub(super) sprite_frames: Option<usize>,
    pub(super) sprite_columns: Option<usize>,
    pub(super) sprite_rows: Option<usize>,
    pub(super) frame_width: Option<usize>,
    pub(super) frame_height: Option<usize>,
    pub(super) source: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LoadingSpinnerRuntimeAnimation {
    pub(super) rotation_radians: f32,
    pub(super) sprite_frame_index: Option<usize>,
    pub(super) sprite_frames: Option<usize>,
    pub(super) sprite_columns: Option<usize>,
    pub(super) sprite_rows: Option<usize>,
    pub(super) frame_width: Option<usize>,
    pub(super) frame_height: Option<usize>,
}

impl LoadingSpinnerAnimationSpec {
    fn fallback() -> Self {
        Self {
            rotation_rps: DEFAULT_LOADING_SPINNER_RPS,
            sprite_fps: 24.0,
            sprite_frames: Some(1),
            sprite_columns: Some(1),
            sprite_rows: Some(1),
            frame_width: Some(64),
            frame_height: Some(64),
            source: "engine-default",
        }
    }

    pub(super) fn runtime(&self, now_ms: u64) -> LoadingSpinnerRuntimeAnimation {
        let t_sec = now_ms as f64 * 0.001;
        let rotation_radians =
            (t_sec * f64::from(self.rotation_rps.max(0.0)) * std::f64::consts::TAU)
                .rem_euclid(std::f64::consts::TAU) as f32;
        let sprite_frame_index = if self.sprite_fps > 0.0 {
            let frame_count = self.sprite_frames.unwrap_or(usize::MAX).max(1);
            Some(
                ((t_sec * f64::from(self.sprite_fps)).floor() as u64
                    % frame_count.min(u64::MAX as usize) as u64) as usize,
            )
        } else {
            None
        };
        LoadingSpinnerRuntimeAnimation {
            rotation_radians,
            sprite_frame_index,
            sprite_frames: self.sprite_frames,
            sprite_columns: self.sprite_columns,
            sprite_rows: self.sprite_rows,
            frame_width: self.frame_width,
            frame_height: self.frame_height,
        }
    }
}

static LOADING_SPINNER_ANIMATION_SPEC: std::sync::OnceLock<LoadingSpinnerAnimationSpec> =
    std::sync::OnceLock::new();

pub(super) fn loading_spinner_animation_spec() -> &'static LoadingSpinnerAnimationSpec {
    LOADING_SPINNER_ANIMATION_SPEC.get_or_init(load_spinner_animation_spec_from_neui)
}

fn load_spinner_animation_spec_from_neui() -> LoadingSpinnerAnimationSpec {
    let Some(source) = read_loading_neui_source() else {
        return LoadingSpinnerAnimationSpec::fallback();
    };
    let Some(tag) = extract_image_tag_by_id(&source, "loading.spinner") else {
        return LoadingSpinnerAnimationSpec::fallback();
    };

    let mut spec = LoadingSpinnerAnimationSpec::fallback();
    spec.source = "loading.neui";
    if let Some(value) = neui_param_f32(&tag, "rotation_rps") {
        spec.rotation_rps = value.clamp(0.0, 20.0);
    } else if let Some(value) = neui_param_f32(&tag, "rotation_rad_per_sec")
        .or_else(|| neui_param_f32(&tag, "rotation_rad_s"))
    {
        spec.rotation_rps = (value / std::f32::consts::TAU).clamp(0.0, 20.0);
    }
    if let Some(value) = neui_param_f32(&tag, "sprite_fps") {
        spec.sprite_fps = value.clamp(0.0, 240.0);
    } else if let Some(ms) = neui_param_usize(&tag, "sprite_frame_ms").filter(|ms| *ms > 0) {
        spec.sprite_fps = (1000.0 / ms as f32).clamp(0.0, 240.0);
    }
    spec.sprite_frames = neui_param_usize(&tag, "sprite_frames")
        .or_else(|| neui_param_usize(&tag, "frame_count"))
        .or(spec.sprite_frames);
    spec.sprite_columns = neui_param_usize(&tag, "sprite_columns")
        .or_else(|| neui_param_usize(&tag, "columns"))
        .or(spec.sprite_columns);
    spec.sprite_rows = neui_param_usize(&tag, "sprite_rows")
        .or_else(|| neui_param_usize(&tag, "rows"))
        .or(spec.sprite_rows);
    spec.frame_width = neui_param_usize(&tag, "frame_width")
        .or_else(|| neui_param_usize(&tag, "sprite_frame_width"))
        .or(spec.frame_width);
    spec.frame_height = neui_param_usize(&tag, "frame_height")
        .or_else(|| neui_param_usize(&tag, "sprite_frame_height"))
        .or(spec.frame_height);
    spec
}

fn read_loading_neui_source() -> Option<String> {
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        for rel in [
            "gameAssets/ui/src/engine/loading.neui.xml",
            "../gameAssets/ui/src/engine/loading.neui.xml",
            "../../gameAssets/ui/src/engine/loading.neui.xml",
            "../../../gameAssets/ui/src/engine/loading.neui.xml",
            "NorthStar/gameAssets/ui/src/engine/loading.neui.xml",
        ] {
            candidates.push(cwd.join(rel));
        }
    }
    candidates.push(std::path::PathBuf::from(
        "NorthStar/gameAssets/ui/src/engine/loading.neui.xml",
    ));

    for path in candidates {
        if let Ok(source) = std::fs::read_to_string(&path) {
            return Some(source);
        }
    }
    None
}

fn extract_image_tag_by_id(source: &str, id: &str) -> Option<String> {
    let mut search_from = 0usize;
    while let Some(offset) = source[search_from..].find("<Image") {
        let start = search_from + offset;
        let end = source[start..].find('>').map(|offset| start + offset + 1)?;
        let tag = &source[start..end];
        if extract_xml_attr(tag, "id").as_deref() == Some(id) {
            return Some(tag.to_owned());
        }
        search_from = end;
    }
    None
}

fn neui_param_f32(tag: &str, key: &str) -> Option<f32> {
    neui_param_raw(tag, key).and_then(|value| value.parse::<f32>().ok())
}

fn neui_param_usize(tag: &str, key: &str) -> Option<usize> {
    neui_param_raw(tag, key).and_then(|value| value.parse::<usize>().ok())
}

fn neui_param_raw(tag: &str, key: &str) -> Option<String> {
    extract_xml_attr(tag, key).or_else(|| {
        for attr in ["tags", "args", "class"] {
            if let Some(value) = extract_xml_attr(tag, attr) {
                if let Some(found) = token_param(&value, key) {
                    return Some(found);
                }
            }
        }
        None
    })
}

fn token_param(value: &str, key: &str) -> Option<String> {
    for token in value.split(|ch: char| ch.is_whitespace() || ch == ';' || ch == ',') {
        if let Some(rest) = token.strip_prefix(key) {
            let rest = rest.strip_prefix('=').or_else(|| rest.strip_prefix(':'))?;
            if !rest.trim().is_empty() {
                return Some(rest.trim().to_owned());
            }
        }
    }
    None
}

fn extract_xml_attr(tag: &str, key: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let key_bytes = key.as_bytes();
    let mut i = 0usize;
    while i + key_bytes.len() < bytes.len() {
        if &bytes[i..i + key_bytes.len()] == key_bytes {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
            let mut j = i + key_bytes.len();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if before_ok && j < bytes.len() && bytes[j] == b'=' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                    let quote = bytes[j];
                    j += 1;
                    let start = j;
                    while j < bytes.len() && bytes[j] != quote {
                        j += 1;
                    }
                    if j <= bytes.len() {
                        return Some(String::from_utf8_lossy(&bytes[start..j]).trim().to_owned());
                    }
                }
            }
        }
        i += 1;
    }
    None
}
