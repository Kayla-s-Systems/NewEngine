use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const ENGINE_LOADING_PLUGIN_ID: &str = "engine.loading";
pub const MAX_LOADING_LOGOS: usize = 8;

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

    #[inline]
    const fn config_key(self) -> &'static str {
        match self {
            Self::PreStart => "prestart",
            Self::RuntimeLoading => "runtime_loading",
            Self::WorldHandoff => "world_handoff",
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
    /// Compatibility alias for consumers authored before multi-logo support.
    pub logo: Option<String>,
    #[serde(default)]
    pub logos: Vec<String>,
    pub spinner: Option<String>,
    pub source: String,
}

impl Default for LoadingVisualRefs {
    fn default() -> Self {
        Self::engine_default()
    }
}

impl LoadingVisualRefs {
    pub fn engine_default() -> Self {
        Self {
            background: None,
            logo: None,
            logos: Vec::new(),
            spinner: None,
            source: "core-default-no-visual-assets".to_owned(),
        }
    }

    /// Returns normalized logo refs in presentation order.
    ///
    /// The legacy singular ref is kept first when present. Duplicate and empty
    /// entries are removed, and the bounded result protects boot presenters from
    /// untrusted or accidentally huge consumer manifests.
    pub fn logo_refs(&self) -> Vec<&str> {
        let mut resolved = Vec::with_capacity((self.logos.len() + 1).min(MAX_LOADING_LOGOS));

        for candidate in self
            .logo
            .iter()
            .chain(self.logos.iter())
            .map(String::as_str)
        {
            let candidate = candidate.trim();
            if candidate.is_empty() || resolved.iter().any(|existing| *existing == candidate) {
                continue;
            }
            resolved.push(candidate);
            if resolved.len() == MAX_LOADING_LOGOS {
                break;
            }
        }

        resolved
    }

    #[inline]
    pub fn primary_logo(&self) -> Option<&str> {
        self.logo_refs().into_iter().next()
    }

    pub fn image_layer_count(&self) -> usize {
        usize::from(non_empty_ref(self.background.as_deref()).is_some())
            + self.logo_refs().len()
            + usize::from(non_empty_ref(self.spinner.as_deref()).is_some())
    }

    pub fn diagnostic_summary(&self) -> String {
        let logos = self.logo_refs();
        format!(
            "visuals source='{}' background={} logos=[{}] spinner={} image_layers={}",
            self.source,
            display_ref(self.background.as_deref()),
            logos.join(","),
            display_ref(self.spinner.as_deref()),
            self.image_layer_count()
        )
    }

    pub fn from_startup_config_for_phase(
        startup: &crate::startup::StartupConfig,
        phase: LoadingPhase,
    ) -> Self {
        LoadingProfile::from_startup_config(startup)
            .visuals_for_phase(phase)
            .clone()
    }

    pub fn from_startup_config_or_default(startup: &crate::startup::StartupConfig) -> Self {
        Self::from_startup_config_for_phase(startup, LoadingPhase::PreStart)
    }

    pub fn from_last_startup_config_for_phase_or_default(phase: LoadingPhase) -> Self {
        crate::startup::last_startup_config()
            .map(|startup| Self::from_startup_config_for_phase(&startup, phase))
            .unwrap_or_else(Self::engine_default)
    }

    pub fn from_last_startup_config_or_default() -> Self {
        Self::from_last_startup_config_for_phase_or_default(LoadingPhase::PreStart)
    }
}

fn non_empty_ref(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn display_ref(value: Option<&str>) -> String {
    non_empty_ref(value).unwrap_or("<none>").to_owned()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadingProfile {
    pub manifest_id: String,
    pub brand_id: String,
    pub display_name: String,
    /// PreStart visuals retained under the original public field name.
    pub visuals: LoadingVisualRefs,
    #[serde(default)]
    pub runtime_loading_visuals: LoadingVisualRefs,
    #[serde(default)]
    pub world_handoff_visuals: LoadingVisualRefs,
    pub source: String,
}

impl LoadingProfile {
    pub fn engine_default() -> Self {
        let visuals = LoadingVisualRefs::engine_default();
        Self {
            manifest_id: "engine.loading.default".to_owned(),
            brand_id: "brand.northstar.engine.default".to_owned(),
            display_name: "NORTH STAR ENGINE".to_owned(),
            visuals: visuals.clone(),
            runtime_loading_visuals: visuals.clone(),
            world_handoff_visuals: visuals,
            source: "engine-default".to_owned(),
        }
    }

    pub fn visuals_for_phase(&self, phase: LoadingPhase) -> &LoadingVisualRefs {
        match phase {
            LoadingPhase::PreStart => &self.visuals,
            LoadingPhase::RuntimeLoading => &self.runtime_loading_visuals,
            LoadingPhase::WorldHandoff => &self.world_handoff_visuals,
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
            let fallback_source = profile.source.clone();
            set_visual_sources(&mut profile, fallback_source);
            return profile;
        };

        profile.source = "consumer:plugins.engine.loading".to_owned();

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

        let base_object = object
            .get("visuals")
            .or_else(|| object.get("visual"))
            .and_then(Value::as_object)
            .unwrap_or(object);
        let base = parse_visuals(
            LoadingVisualRefs::engine_default(),
            base_object,
            format!("{}:default", profile.source),
        );

        profile.visuals = parse_phase_visuals(
            object,
            LoadingPhase::PreStart,
            base.clone(),
            &profile.source,
        );
        profile.runtime_loading_visuals = parse_phase_visuals(
            object,
            LoadingPhase::RuntimeLoading,
            profile.visuals.clone(),
            &profile.source,
        );
        profile.world_handoff_visuals = parse_phase_visuals(
            object,
            LoadingPhase::WorldHandoff,
            profile.runtime_loading_visuals.clone(),
            &profile.source,
        );

        profile
    }

    pub fn from_last_startup_config_or_default() -> Self {
        crate::startup::last_startup_config()
            .map(Self::from_startup_config)
            .unwrap_or_else(Self::engine_default)
    }
}

fn set_visual_sources(profile: &mut LoadingProfile, source: String) {
    profile.visuals.source = source.clone();
    profile.runtime_loading_visuals.source = source.clone();
    profile.world_handoff_visuals.source = source;
}

fn parse_phase_visuals(
    object: &Map<String, Value>,
    phase: LoadingPhase,
    fallback: LoadingVisualRefs,
    profile_source: &str,
) -> LoadingVisualRefs {
    let phase_value = match phase {
        LoadingPhase::RuntimeLoading => object
            .get(phase.config_key())
            .or_else(|| object.get("runtime")),
        LoadingPhase::WorldHandoff => object
            .get(phase.config_key())
            .or_else(|| object.get("handoff")),
        LoadingPhase::PreStart => object.get(phase.config_key()),
    };

    let Some(phase_object) = phase_value.and_then(Value::as_object) else {
        let mut inherited = fallback;
        inherited.source = format!("{profile_source}:{}:inherited", phase.config_key());
        return inherited;
    };

    parse_visuals(
        fallback,
        phase_object,
        format!("{profile_source}:{}", phase.config_key()),
    )
}

fn parse_visuals(
    mut visuals: LoadingVisualRefs,
    object: &Map<String, Value>,
    source: String,
) -> LoadingVisualRefs {
    visuals.source = source;

    if let Some(value) = visual_ref_field(object.get("background"))
        .or_else(|| visual_ref_field(object.get("bg")))
        .or_else(|| visual_ref_field(object.get("background_ref")))
    {
        visuals.background = Some(value);
    }

    let explicit_logo = visual_ref_field(object.get("logo"))
        .or_else(|| visual_ref_field(object.get("logo_ref")))
        .or_else(|| visual_ref_field(object.get("prestart_logo")));
    let has_explicit_logo = explicit_logo.is_some();
    let logo_list = object
        .get("logos")
        .or_else(|| object.get("logo_refs"))
        .or_else(|| object.get("brand_logos"));
    if let Some(explicit_logo) = explicit_logo {
        visuals.logo = Some(explicit_logo);
        if logo_list.is_none() {
            visuals.logos.clear();
        }
    }

    if let Some(value) = logo_list {
        visuals.logos = visual_refs_field(value);
        if !has_explicit_logo {
            visuals.logo = visuals.logos.first().cloned();
        }
    }

    if let Some(value) = visual_ref_field(object.get("spinner"))
        .or_else(|| visual_ref_field(object.get("spinner_ref")))
        .or_else(|| visual_ref_field(object.get("loading_spinner")))
    {
        visuals.spinner = Some(value);
    }

    visuals
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

fn visual_refs_field(value: &Value) -> Vec<String> {
    let candidates: Vec<String> = match value {
        Value::Array(values) => values
            .iter()
            .filter_map(|value| visual_ref_field(Some(value)))
            .collect(),
        _ => visual_ref_field(Some(value)).into_iter().collect(),
    };

    let mut normalized = Vec::with_capacity(candidates.len().min(MAX_LOADING_LOGOS));
    for candidate in candidates {
        let candidate = candidate.trim();
        if candidate.is_empty() || normalized.iter().any(|existing| existing == candidate) {
            continue;
        }
        normalized.push(candidate.to_owned());
        if normalized.len() == MAX_LOADING_LOGOS {
            break;
        }
    }
    normalized
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
        let visuals = profile.visuals_for_phase(phase).clone();
        Self {
            assignment_id: format!("{}.{}", profile.manifest_id, phase.as_str()),
            phase,
            pipeline: format!("{}.boot", profile.manifest_id),
            selected: vec![profile.brand_id.clone()],
            source: profile.source.clone(),
            presenter: "engine.platform.boot_presenter".to_owned(),
            asset_mode: if visuals.image_layer_count() > 0 {
                "consumer-declared-asset-ref"
            } else {
                "no-core-visual-assets"
            }
            .to_owned(),
            display_name: profile.display_name.clone(),
            visuals,
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
