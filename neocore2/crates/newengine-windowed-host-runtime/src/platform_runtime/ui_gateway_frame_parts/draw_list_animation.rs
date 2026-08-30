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
    fn unconfigured() -> Self {
        Self {
            rotation_rps: 0.0,
            sprite_fps: 0.0,
            sprite_frames: None,
            sprite_columns: None,
            sprite_rows: None,
            frame_width: None,
            frame_height: None,
            source: "unconfigured",
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
    LOADING_SPINNER_ANIMATION_SPEC.get_or_init(load_spinner_animation_spec_from_config)
}

fn load_spinner_animation_spec_from_config() -> LoadingSpinnerAnimationSpec {
    let Some(startup) = newengine_core::startup::last_startup_config() else {
        return LoadingSpinnerAnimationSpec::unconfigured();
    };
    let Some(config) = startup
        .plugins
        .get("engine.loading")
        .and_then(|plugin| plugin.get("spinner_animation"))
        .and_then(serde_json::Value::as_object)
    else {
        return LoadingSpinnerAnimationSpec::unconfigured();
    };

    let finite_f32 = |key: &str| {
        config
            .get(key)
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite())
            .map(|value| value as f32)
    };
    let positive_usize = |key: &str| {
        config
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value > 0)
    };

    LoadingSpinnerAnimationSpec {
        rotation_rps: finite_f32("rotation_rps").unwrap_or(0.0).clamp(0.0, 20.0),
        sprite_fps: finite_f32("sprite_fps").unwrap_or(0.0).clamp(0.0, 240.0),
        sprite_frames: positive_usize("sprite_frames"),
        sprite_columns: positive_usize("sprite_columns"),
        sprite_rows: positive_usize("sprite_rows"),
        frame_width: positive_usize("frame_width"),
        frame_height: positive_usize("frame_height"),
        source: "config:engine.loading.spinner_animation",
    }
}
