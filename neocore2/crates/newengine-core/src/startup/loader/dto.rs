#[derive(Deserialize)]
struct RootJson {
    window: Option<WindowJson>,
    engine: Option<EngineJson>,
    startup_settings: Option<crate::startup_window::StartupLaunchSettings>,
    plugins: Option<newengine_math::collections_prelude::NeHashMap<String, serde_json::Value>>,

    #[serde(flatten)]
    extra: newengine_math::collections_prelude::NeHashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct WindowJson {
    title: Option<String>,

    size: Option<[u32; 2]>,
    width: Option<u32>,
    height: Option<u32>,

    placement: Option<WindowPlacementJson>,

    /// Logical path inside assets, e.g. "textures/ui/icons/builtin_icons.ytd@app_logo"
    icon: Option<String>,
}

#[derive(Deserialize)]
struct WindowPlacementJson {
    #[serde(rename = "type")]
    kind: Option<String>,
    offset: Option<[i32; 2]>,
}

#[derive(Deserialize)]
struct EngineJson {
    modules_dir: Option<String>,
    cache_files: Option<String>,
    #[serde(rename = "CACHE_FILES")]
    cache_files_upper: Option<String>,
    config: Option<String>,
    #[serde(rename = "CONFIG")]
    config_upper: Option<String>,

    /// Unknown keys are preserved to produce deterministic diagnostics.
    ///
    /// Engine-side asset settings are intentionally NOT supported anymore:
    /// assets must be configured via the AssetManager plugin (`plugins.newengine.assets`).
    #[serde(flatten)]
    extra: newengine_math::collections_prelude::NeHashMap<String, serde_json::Value>,
}
