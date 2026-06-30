#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ENGINE_DEFAULT_PRESTART_BACKGROUND_REF: &str =
    "textures/ui/loading/loaderWindow.ytd@north_star_preload_background";
pub const ENGINE_DEFAULT_PRESTART_LOGO_REF: &str =
    "textures/ui/loading/loaderWindow.ytd@north_star_engine_logo";
pub const ENGINE_DEFAULT_PRESTART_SPINNER_REF: &str =
    "textures/ui/loading/loaderWindow.ytd@north_star_loading_spinner";

pub const ENGINE_LOADING_PLUGIN_ID: &str = "engine.loading";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadingPhase {
    PreStart,
    RuntimeLoading,
    WorldHandoff,
}

impl LoadingPhase {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreStart => "PreStart",
            Self::RuntimeLoading => "RuntimeLoading",
            Self::WorldHandoff => "WorldHandoff",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadingVisualRole {
    Background,
    Logo,
    Spinner,
}

impl LoadingVisualRole {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Logo => "logo",
            Self::Spinner => "spinner",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadingVisualRefs {
    pub background: Option<String>,
    pub logo: Option<String>,
    pub spinner: Option<String>,
    pub source: String,
}

impl LoadingVisualRefs {
    pub fn engine_default() -> Self {
        Self {
            background: Some(ENGINE_DEFAULT_PRESTART_BACKGROUND_REF.to_owned()),
            logo: Some(ENGINE_DEFAULT_PRESTART_LOGO_REF.to_owned()),
            spinner: Some(ENGINE_DEFAULT_PRESTART_SPINNER_REF.to_owned()),
            source: "engine-default".to_owned(),
        }
    }

    pub fn image_layer_count(&self) -> usize {
        [
            self.background.as_ref(),
            self.logo.as_ref(),
            self.spinner.as_ref(),
        ]
        .into_iter()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .count()
    }

    pub fn diagnostic_summary(&self) -> String {
        format!(
            "visuals source='{}' background={} logo={} spinner={} image_layers={}",
            self.source,
            display_ref(self.background.as_deref()),
            display_ref(self.logo.as_deref()),
            display_ref(self.spinner.as_deref()),
            self.image_layer_count()
        )
    }

    pub fn from_startup_config_or_default(startup: &crate::startup::StartupConfig) -> Self {
        LoadingProfile::from_startup_config(startup).visuals
    }

    pub fn from_last_startup_config_or_default() -> Self {
        crate::startup::last_startup_config()
            .map(Self::from_startup_config_or_default)
            .unwrap_or_else(Self::engine_default)
    }
}

fn display_ref(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("<none>")
        .to_owned()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadingProfile {
    pub manifest_id: String,
    pub brand_id: String,
    pub display_name: String,
    pub visuals: LoadingVisualRefs,
    pub source: String,
}

impl LoadingProfile {
    pub fn engine_default() -> Self {
        Self {
            manifest_id: "engine.loading.default".to_owned(),
            brand_id: "brand.northstar.engine.default".to_owned(),
            display_name: "NORTH STAR ENGINE".to_owned(),
            visuals: LoadingVisualRefs::engine_default(),
            source: "engine-default".to_owned(),
        }
    }

    pub fn from_startup_config(startup: &crate::startup::StartupConfig) -> Self {
        let mut profile = Self::engine_default();
        let Some(value) = startup
            .plugins
            .get(ENGINE_LOADING_PLUGIN_ID)
            .or_else(|| nested_engine_loading_plugin_value(&startup.plugins))
        else {
            return profile;
        };

        let Some(object) = value.as_object() else {
            profile.source = "invalid-consumer-config-fallback-engine-default".to_owned();
            profile.visuals.source = profile.source.clone();
            return profile;
        };

        profile.source = "consumer:plugins.engine.loading".to_owned();
        profile.visuals.source = profile.source.clone();

        if let Some(value) = string_field(object.get("manifest_id")) {
            profile.manifest_id = value;
        }
        if let Some(value) = string_field(object.get("brand_id")) {
            profile.brand_id = value;
        }
        if let Some(value) = string_field(object.get("display_name"))
            .or_else(|| string_field(object.get("title")))
            .or_else(|| string_field(object.get("name")))
        {
            profile.display_name = value;
        }

        let visuals = object
            .get("prestart")
            .or_else(|| object.get("visuals"))
            .or_else(|| object.get("visual"))
            .and_then(Value::as_object)
            .unwrap_or(object);

        if let Some(value) = visual_ref_field(visuals.get("background"))
            .or_else(|| visual_ref_field(visuals.get("bg")))
            .or_else(|| visual_ref_field(visuals.get("background_ref")))
        {
            profile.visuals.background = Some(value);
        }
        if let Some(value) = visual_ref_field(visuals.get("logo"))
            .or_else(|| visual_ref_field(visuals.get("logo_ref")))
            .or_else(|| visual_ref_field(visuals.get("prestart_logo")))
        {
            profile.visuals.logo = Some(value);
        }
        if let Some(value) = visual_ref_field(visuals.get("spinner"))
            .or_else(|| visual_ref_field(visuals.get("spinner_ref")))
            .or_else(|| visual_ref_field(visuals.get("loading_spinner")))
        {
            profile.visuals.spinner = Some(value);
        }

        profile
    }

    pub fn from_last_startup_config_or_default() -> Self {
        crate::startup::last_startup_config()
            .map(Self::from_startup_config)
            .unwrap_or_else(Self::engine_default)
    }
}

fn nested_engine_loading_plugin_value(
    plugins: &newengine_math::collections_prelude::NeHashMap<String, Value>,
) -> Option<&Value> {
    plugins.get("engine").and_then(|value| value.get("loading"))
}

fn string_field(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn visual_ref_field(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(s) = value.as_str() {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return None;
        }
        return Some(trimmed.to_owned());
    }

    value.as_object().and_then(|object| {
        string_field(object.get("texture_ref"))
            .or_else(|| string_field(object.get("ref")))
            .or_else(|| string_field(object.get("uri")))
            .or_else(|| string_field(object.get("texture")))
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedLoadingAssignment {
    pub assignment_id: String,
    pub phase: LoadingPhase,
    pub pipeline: String,
    pub selected: Vec<String>,
    pub source: String,
    pub presenter: String,
    pub asset_mode: String,
    pub display_name: String,
    pub visuals: LoadingVisualRefs,
}

impl ResolvedLoadingAssignment {
    pub fn engine_default(phase: LoadingPhase) -> Self {
        Self::from_profile(phase, &LoadingProfile::engine_default())
    }

    pub fn from_profile(phase: LoadingPhase, profile: &LoadingProfile) -> Self {
        Self {
            assignment_id: format!("{}.{}", profile.manifest_id, phase.as_str()),
            phase,
            pipeline: format!("{}.boot", profile.manifest_id),
            selected: vec![profile.brand_id.clone()],
            source: profile.source.clone(),
            presenter: "engine.platform.boot_presenter".to_owned(),
            asset_mode: if profile.source.starts_with("consumer:") {
                "consumer-declared-ytd-entry"
            } else {
                "engine-default-ytd-entry"
            }
            .to_owned(),
            display_name: profile.display_name.clone(),
            visuals: profile.visuals.clone(),
        }
    }

    pub fn override_summary(&self) -> String {
        format!(
            "assignment resolved phase='{}' pipeline='{}' selected=[{}] source='{}' presenter='{}' asset_mode='{}' {}",
            self.phase.as_str(),
            self.pipeline,
            self.selected.join(","),
            self.source,
            self.presenter,
            self.asset_mode,
            self.visuals.diagnostic_summary()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BootViewport {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

impl Default for BootViewport {
    #[inline]
    fn default() -> Self {
        Self {
            width: 1600.0,
            height: 900.0,
            scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorRgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorRgba8 {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BootRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl BootRect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootTextRun {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size_px: f32,
    pub color: ColorRgba8,
}

impl BootTextRun {
    pub fn new(text: impl Into<String>, x: f32, y: f32, size_px: f32, color: ColorRgba8) -> Self {
        Self {
            text: text.into(),
            x,
            y,
            size_px,
            color,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BootDrawCommand {
    Clear {
        color: ColorRgba8,
    },
    Rect {
        rect: BootRect,
        color: ColorRgba8,
    },
    Text {
        run: BootTextRun,
    },
    Image {
        role: LoadingVisualRole,
        texture_ref: String,
        rect: BootRect,
        alpha: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadingProgressSnapshot {
    pub progress_01: f32,
    pub spinner_phase: u32,
    pub status: String,
    pub detail: String,
}

impl LoadingProgressSnapshot {
    pub fn new(
        progress_01: f32,
        spinner_phase: u32,
        status: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            progress_01: progress_01.clamp(0.0, 1.0),
            spinner_phase,
            status: status.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootFrameDto {
    pub assignment: ResolvedLoadingAssignment,
    pub viewport: BootViewport,
    pub clear: ColorRgba8,
    pub commands: Vec<BootDrawCommand>,
    pub progress: LoadingProgressSnapshot,
    pub diagnostics: Vec<String>,
}

impl BootFrameDto {
    #[allow(clippy::too_many_arguments)]
    pub fn from_status(
        assignment: ResolvedLoadingAssignment,
        viewport: BootViewport,
        title: impl Into<String>,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
        spinner_phase: u32,
    ) -> Self {
        let title = normalize_text(title.into(), "NORTH STAR ENGINE");
        let status = normalize_text(status.into(), "Preparing runtime...");
        let detail = normalize_text(detail.into(), "Boot-safe loading presenter is active.");
        let progress = LoadingProgressSnapshot::new(
            progress_01,
            spinner_phase,
            status.clone(),
            detail.clone(),
        );
        let clear = ColorRgba8::rgba(8, 12, 20, 255);
        let track = ColorRgba8::rgba(35, 45, 62, 255);
        let fill = ColorRgba8::rgba(96, 165, 250, 255);
        let text = ColorRgba8::rgba(235, 245, 255, 255);
        let muted = ColorRgba8::rgba(148, 163, 184, 255);

        let safe_w = viewport.width.max(1.0);
        let safe_h = viewport.height.max(1.0);
        let bar_w = (safe_w * 0.42).clamp(480.0, 780.0);
        let bar_h = 8.0;
        let bar_x = (safe_w - bar_w) * 0.5;
        let bar_y = (safe_h * 0.72).clamp(420.0, safe_h - 90.0);
        let fill_w = bar_w * progress.progress_01.clamp(0.0, 1.0);
        let logo_size = (safe_w.min(safe_h) * 0.28).clamp(180.0, 360.0);
        let logo_x = (safe_w - logo_size) * 0.5;
        let logo_y = (safe_h - logo_size) * 0.42;
        let spinner_size = 64.0;

        let mut commands = vec![BootDrawCommand::Clear { color: clear }];

        if let Some(texture_ref) = assignment
            .visuals
            .background
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            commands.push(BootDrawCommand::Image {
                role: LoadingVisualRole::Background,
                texture_ref: texture_ref.to_owned(),
                rect: BootRect::new(0.0, 0.0, safe_w, safe_h),
                alpha: 1.0,
            });
        }

        if let Some(texture_ref) = assignment
            .visuals
            .logo
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            commands.push(BootDrawCommand::Image {
                role: LoadingVisualRole::Logo,
                texture_ref: texture_ref.to_owned(),
                rect: BootRect::new(logo_x, logo_y, logo_size, logo_size),
                alpha: 1.0,
            });
        }

        if let Some(texture_ref) = assignment
            .visuals
            .spinner
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            commands.push(BootDrawCommand::Image {
                role: LoadingVisualRole::Spinner,
                texture_ref: texture_ref.to_owned(),
                rect: BootRect::new(
                    (safe_w - spinner_size) * 0.5,
                    bar_y + 26.0,
                    spinner_size,
                    spinner_size,
                ),
                alpha: 1.0,
            });
        }

        commands.extend([
            BootDrawCommand::Text {
                run: BootTextRun::new(title, bar_x, bar_y - 92.0, 24.0, text),
            },
            BootDrawCommand::Text {
                run: BootTextRun::new(status, bar_x, bar_y - 54.0, 16.0, text),
            },
            BootDrawCommand::Text {
                run: BootTextRun::new(detail, bar_x, bar_y - 28.0, 14.0, muted),
            },
            BootDrawCommand::Rect {
                rect: BootRect::new(bar_x, bar_y, bar_w, bar_h),
                color: track,
            },
            BootDrawCommand::Rect {
                rect: BootRect::new(bar_x, bar_y, fill_w, bar_h),
                color: fill,
            },
        ]);

        let diagnostics = vec![assignment.visuals.diagnostic_summary()];

        Self {
            assignment,
            viewport,
            clear,
            commands,
            progress,
            diagnostics,
        }
    }

    #[inline]
    pub fn diagnostic_summary(&self) -> String {
        format!(
            "boot_frame assignment='{}' phase='{}' commands={} image_layers={} viewport={:.0}x{:.0} progress={:.0}%",
            self.assignment.assignment_id,
            self.assignment.phase.as_str(),
            self.commands.len(),
            self.assignment.visuals.image_layer_count(),
            self.viewport.width,
            self.viewport.height,
            self.progress.progress_01 * 100.0
        )
    }
}

#[derive(Debug, Clone)]
pub struct EngineLoadingKernel {
    profile: LoadingProfile,
    active_assignment: Option<ResolvedLoadingAssignment>,
}

impl Default for EngineLoadingKernel {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl EngineLoadingKernel {
    #[inline]
    pub fn new() -> Self {
        Self::with_profile(LoadingProfile::engine_default())
    }

    #[inline]
    pub fn with_profile(profile: LoadingProfile) -> Self {
        Self {
            profile,
            active_assignment: None,
        }
    }

    #[inline]
    pub fn with_startup_config(startup: &crate::startup::StartupConfig) -> Self {
        Self::with_profile(LoadingProfile::from_startup_config(startup))
    }

    pub fn resolve_assignment(&mut self, phase: LoadingPhase) -> ResolvedLoadingAssignment {
        let assignment = ResolvedLoadingAssignment::from_profile(phase, &self.profile);
        self.active_assignment = Some(assignment.clone());
        assignment
    }

    pub fn boot_frame(&self, viewport: BootViewport) -> BootFrameDto {
        let assignment = self.active_assignment.clone().unwrap_or_else(|| {
            ResolvedLoadingAssignment::from_profile(LoadingPhase::PreStart, &self.profile)
        });

        BootFrameDto::from_status(
            assignment,
            viewport,
            self.profile.display_name.clone(),
            "PreStart loading assignment resolved.",
            "Boot-safe presenter frame is generated without engine.ui; visual refs are consumer-declared data.",
            0.05,
            0,
        )
    }
}

#[inline]
fn normalize_text(value: String, fallback: &'static str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_math::collections_prelude::NeHashMap;

    #[test]
    fn prestart_boot_frame_uses_engine_loading_assignment() {
        let mut kernel = EngineLoadingKernel::new();
        let assignment = kernel.resolve_assignment(LoadingPhase::PreStart);
        let frame = kernel.boot_frame(BootViewport::default());

        assert_eq!(assignment.phase, LoadingPhase::PreStart);
        assert_eq!(frame.assignment.phase, LoadingPhase::PreStart);
        assert!(!frame.commands.is_empty());
        assert_eq!(frame.assignment.presenter, "engine.platform.boot_presenter");
        assert_eq!(frame.assignment.visuals.image_layer_count(), 3);
    }

    #[test]
    fn runtime_boot_frame_can_project_status_without_ui() {
        let assignment = ResolvedLoadingAssignment::engine_default(LoadingPhase::RuntimeLoading);
        let frame = BootFrameDto::from_status(
            assignment,
            BootViewport::default(),
            "Title",
            "Status",
            "Detail",
            0.42,
            7,
        );

        assert_eq!(frame.assignment.phase, LoadingPhase::RuntimeLoading);
        assert_eq!(frame.progress.progress_01, 0.42);
        assert_eq!(frame.progress.spinner_phase, 7);
    }

    #[test]
    fn consumer_engine_loading_plugin_assigns_prestart_visuals() {
        let mut startup = crate::startup::StartupConfig::default();
        let mut plugins = NeHashMap::default();
        plugins.insert(
            ENGINE_LOADING_PLUGIN_ID.to_owned(),
            serde_json::json!({
                "manifest_id": "app.gamefps.loading",
                "brand_id": "brand.gamefps",
                "display_name": "GAME FPS",
                "prestart": {
                    "background": "textures/app/loading.ytd@bg",
                    "logo": "textures/app/loading.ytd@logo",
                    "spinner": "textures/app/loading.ytd@spinner"
                }
            }),
        );
        startup.plugins = plugins;

        let profile = LoadingProfile::from_startup_config(&startup);
        assert_eq!(profile.manifest_id, "app.gamefps.loading");
        assert_eq!(
            profile.visuals.logo.as_deref(),
            Some("textures/app/loading.ytd@logo")
        );

        let mut kernel = EngineLoadingKernel::with_startup_config(&startup);
        let assignment = kernel.resolve_assignment(LoadingPhase::PreStart);
        assert_eq!(assignment.source, "consumer:plugins.engine.loading");
        assert_eq!(assignment.selected, vec!["brand.gamefps".to_owned()]);
        assert_eq!(assignment.visuals.image_layer_count(), 3);
    }
}
