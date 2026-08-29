use serde::{Deserialize, Serialize};
use serde_json::Value;

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
            background: None,
            logo: None,
            spinner: None,
            source: "core-default-no-visual-assets".to_owned(),
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
            asset_mode: if profile.visuals.image_layer_count() > 0 {
                "consumer-declared-asset-ref"
            } else {
                "no-core-visual-assets"
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
