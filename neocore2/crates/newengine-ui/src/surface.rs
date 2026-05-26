#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

/// Canonical UI provider and surface identifiers.
///
/// These constants are intentionally owned by `newengine-ui` so runtime hosts,
/// plugins and native adapters do not duplicate string literals for the same
/// logical surfaces.
pub const UI_PROVIDER_NONE_ID: &str = "none";
pub const UI_FEATURE_STANDARD_MODAL_SYSTEM: &str = "standard-modal-system";
pub const UI_PROVIDER_NATIVE_FALLBACK_ID: &str = "newengine.ui.provider.native-fallback";
pub const UI_SURFACE_ENGINE_LOADING: &str = "engine.loading";
pub const UI_SURFACE_ENGINE_ERROR_MODAL: &str = "engine.error_modal";
pub const UI_SURFACE_RUNTIME_OVERLAY: &str = "runtime.overlay";
pub const UI_FEATURE_NATIVE_SAFE_STARTUP: &str = "native-safe-startup";
pub const UI_FEATURE_KSYSTEMS_ERROR_MODAL: &str = "ksystems-error-modal";
pub const UI_FEATURE_EXTERNAL_PLUGIN_PROVIDER: &str = "external-plugin-provider";
pub const UI_SHELL_KSYSTEMS_LOADING_ID: &str = "newengine.shell.ksystems.loading.v1";
pub const UI_SHELL_MINIMAL_FALLBACK_LOADING_ID: &str = "newengine.shell.minimal-fallback.loading.v1";
pub const UI_THEME_DARK_GOLD_MAGENTA: &str = "newengine.dark.gold-magenta";
pub const UI_STYLE_KSYSTEMS_INDUSTRIAL: &str = "ksystems-industrial";
pub const UI_ERROR_MODAL_KSYSTEMS_ID: &str = "engine.error_modal.ksystems.v1";

/// Stable identity for the UI provider selected by startup config/plugin discovery.
///
/// This is intentionally serializable and renderer-agnostic. Runtime systems can
/// project UI surfaces without knowing whether the final presentation is native,
/// a provider backend or intentionally disabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiProviderBinding {
    /// No user interface provider is active. This is a valid headless/minimal mode.
    None,
    /// Built-in native fallback adapter used before an external UI provider is ready.
    NativeFallback,
    /// External provider selected by service id.
    Plugin { service_id: String },
}

impl UiProviderBinding {
    #[inline]
    pub fn none() -> Self {
        Self::None
    }

    #[inline]
    pub fn native_fallback() -> Self {
        Self::NativeFallback
    }

    #[inline]
    pub fn plugin(service_id: impl Into<String>) -> Self {
        Self::Plugin {
            service_id: service_id.into(),
        }
    }

    #[inline]
    pub fn id(&self) -> &str {
        match self {
            Self::None => UI_PROVIDER_NONE_ID,
            Self::NativeFallback => UI_PROVIDER_NATIVE_FALLBACK_ID,
            Self::Plugin { service_id } => service_id.as_str(),
        }
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    #[inline]
    pub fn is_native_fallback(&self) -> bool {
        matches!(self, Self::NativeFallback)
    }

    #[inline]
    pub fn is_plugin(&self) -> bool {
        matches!(self, Self::Plugin { .. })
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        !self.is_none()
    }
}

impl Default for UiProviderBinding {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

/// Provider-owned UI capabilities exposed to the rest of the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiProviderManifest {
    pub provider: UiProviderBinding,
    pub version: u32,
    pub surfaces: Vec<String>,
    pub features: Vec<String>,
}

impl UiProviderManifest {
    #[inline]
    pub fn supports_surface(&self, surface_id: &str) -> bool {
        self.surfaces.iter().any(|surface| surface == surface_id)
    }


    #[inline]
    pub fn none() -> Self {
        Self {
            provider: UiProviderBinding::None,
            version: 1,
            surfaces: Vec::new(),
            features: Vec::new(),
        }
    }

    #[inline]
    pub fn native_fallback() -> Self {
        Self {
            provider: UiProviderBinding::NativeFallback,
            version: 1,
            surfaces: vec![
                UI_SURFACE_ENGINE_LOADING.to_owned(),
                UI_SURFACE_ENGINE_ERROR_MODAL.to_owned(),
            ],
            features: vec![
                UI_FEATURE_NATIVE_SAFE_STARTUP.to_owned(),
                UI_FEATURE_KSYSTEMS_ERROR_MODAL.to_owned(),
                UI_FEATURE_STANDARD_MODAL_SYSTEM.to_owned(),
            ],
        }
    }
}

/// Shell layout and animation policy for loading-style surfaces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiShellSpec {
    pub id: String,
    pub theme: String,
    pub style: String,
    pub animation: UiAnimationSpec,
    pub loading: UiLoadingShellSpec,
    #[serde(default)]
    pub subsystem_cards: UiSubsystemCardSpec,
    pub error_modal: UiErrorModalSpec,
}

impl UiShellSpec {
    #[inline]
    pub fn ksystems_loading() -> Self {
        Self {
            id: UI_SHELL_KSYSTEMS_LOADING_ID.to_owned(),
            theme: UI_THEME_DARK_GOLD_MAGENTA.to_owned(),
            style: UI_STYLE_KSYSTEMS_INDUSTRIAL.to_owned(),
            animation: UiAnimationSpec::default(),
            loading: UiLoadingShellSpec::default(),
            subsystem_cards: UiSubsystemCardSpec::default(),
            error_modal: UiErrorModalSpec::default(),
        }
    }

    pub fn minimal_fallback_loading() -> Self {
        let mut shell = Self::ksystems_loading();
        shell.id = UI_SHELL_MINIMAL_FALLBACK_LOADING_ID.to_owned();
        shell.style = "minimal-native-fallback".to_owned();
        shell.loading.footer_template = "{percent}%".to_owned();
        shell.loading.show_subsystems = false;
        shell.loading.max_subsystems = 0;
        shell.subsystem_cards.show_detail = false;
        shell.subsystem_cards.show_progress = false;
        shell.subsystem_cards.pulse_glow = false;
        shell
    }
}

impl Default for UiShellSpec {
    #[inline]
    fn default() -> Self {
        Self::ksystems_loading()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UiAnimationSpec {
    pub spinner_turns_per_sec: f32,
    pub pulse_hz: f32,
    pub status_hold_secs: f32,
    pub active_frame_interval_ms: u32,
    pub idle_frame_interval_ms: u32,
}

impl Default for UiAnimationSpec {
    #[inline]
    fn default() -> Self {
        Self {
            spinner_turns_per_sec: 0.58,
            pulse_hz: 0.96,
            status_hold_secs: 0.12,
            active_frame_interval_ms: 8,
            idle_frame_interval_ms: 16,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiLoadingShellSpec {
    pub title_template: String,
    pub footer_template: String,
    pub show_spinner: bool,
    pub show_subsystems: bool,
    pub max_subsystems: usize,
}

impl Default for UiLoadingShellSpec {
    #[inline]
    fn default() -> Self {
        Self {
            title_template: "{title}".to_owned(),
            footer_template: "{mode} // {percent}%".to_owned(),
            show_spinner: true,
            show_subsystems: true,
            max_subsystems: 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSubsystemCardSpec {
    pub id: String,
    pub height_px: i32,
    pub icon_y_px: i32,
    pub title_y_px: i32,
    pub state_y_px: i32,
    pub detail_y_px: i32,
    pub progress_bottom_px: i32,
    pub progress_height_px: i32,
    pub progress_segments: i32,
    pub progress_segment_gap_px: i32,
    pub pulse_glow: bool,
    pub glow_radius_px: i32,
    pub glow_strength_pct: u8,
    pub title_max_chars: usize,
    pub state_max_chars: usize,
    pub detail_max_chars: usize,
    pub show_detail: bool,
    pub show_progress: bool,
    pub uppercase_state: bool,
    pub palette_token_set: String,
    pub palette: UiSubsystemCardPaletteSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSubsystemCardPaletteSpec {
    pub success: String,
    pub active: String,
    pub warning: String,
    pub error: String,
    pub waiting: String,
    pub track: String,
    pub segment_empty: String,
    pub glow: String,
}

impl Default for UiSubsystemCardPaletteSpec {
    #[inline]
    fn default() -> Self {
        Self {
            success: "#46d682".to_owned(),
            active: "#ffcb48".to_owned(),
            warning: "#ff9256".to_owned(),
            error: "#ff485a".to_owned(),
            waiting: "#594b3f".to_owned(),
            track: "#0a0c12".to_owned(),
            segment_empty: "#181c24".to_owned(),
            glow: "#de23a7".to_owned(),
        }
    }
}

impl Default for UiSubsystemCardSpec {
    #[inline]
    fn default() -> Self {
        Self {
            id: "engine.loading.subsystem_card.ksystems.v1".to_owned(),
            height_px: 136,
            icon_y_px: 34,
            title_y_px: 70,
            state_y_px: 94,
            detail_y_px: 111,
            progress_bottom_px: 12,
            progress_height_px: 6,
            progress_segments: 18,
            progress_segment_gap_px: 3,
            pulse_glow: true,
            glow_radius_px: 5,
            glow_strength_pct: 38,
            title_max_chars: 18,
            state_max_chars: 12,
            detail_max_chars: 24,
            show_detail: true,
            show_progress: true,
            uppercase_state: true,
            palette_token_set: "newengine.dark.gold-magenta.subsystem-cards".to_owned(),
            palette: UiSubsystemCardPaletteSpec::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiErrorModalSpec {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub severity: String,
    pub primary_action: String,
    pub secondary_action: String,
    pub show_diagnostics_hint: bool,
}

impl Default for UiErrorModalSpec {
    #[inline]
    fn default() -> Self {
        Self {
            id: UI_ERROR_MODAL_KSYSTEMS_ID.to_owned(),
            title: "STARTUP FAILURE".to_owned(),
            subtitle: "NewEngine stopped before playable handoff.".to_owned(),
            severity: "fatal".to_owned(),
            primary_action: "OPEN LOGS".to_owned(),
            secondary_action: "EXIT".to_owned(),
            show_diagnostics_hint: true,
        }
    }
}

/// A complete UI surface projection. Every interface is identified by a
/// surface id and represented as data; there are no hardcoded surface-kind
/// branches in the public projection contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiSurfaceProjection<T> {
    pub version: u32,
    pub surface_id: String,
    pub provider: UiProviderBinding,
    pub shell: UiShellSpec,
    pub state: T,
}

impl<T> UiSurfaceProjection<T> {
    #[inline]
    pub fn new(surface_id: impl Into<String>, provider: UiProviderBinding, shell: UiShellSpec, state: T) -> Self {
        Self {
            version: 1,
            surface_id: surface_id.into(),
            provider,
            shell,
            state,
        }
    }

    #[inline]
    pub fn loading(provider: UiProviderBinding, shell: UiShellSpec, state: T) -> Self {
        Self::new(UI_SURFACE_ENGINE_LOADING, provider, shell, state)
    }

    #[inline]
    pub fn error_modal(provider: UiProviderBinding, shell: UiShellSpec, state: T) -> Self {
        Self::new(UI_SURFACE_ENGINE_ERROR_MODAL, provider, shell, state)
    }

    #[inline]
    pub fn surface_id(&self) -> &str {
        self.surface_id.as_str()
    }
}
