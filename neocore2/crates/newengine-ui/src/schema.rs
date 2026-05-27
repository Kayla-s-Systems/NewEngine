#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ui_api::UiDocumentSourceKind;
use serde::{Deserialize, Serialize};

use crate::surface::{
    UiProviderBinding, UiProviderManifest, UI_SURFACE_ENGINE_ERROR_MODAL,
    UI_SURFACE_ENGINE_LOADING, UI_SURFACE_RUNTIME_OVERLAY,
};

/// Additional canonical surface identifiers used by full-runtime UI providers.
pub const UI_SURFACE_MAIN_MENU: &str = "engine.main_menu";
pub const UI_SURFACE_PRIMARY: &str = "engine.ui.primary";
pub const UI_SURFACE_GAME_HUD: &str = "game.hud";
pub const UI_SURFACE_DEBUG_OVERLAY: &str = "runtime.debug_overlay";

/// Canonical action identifiers. Providers bind widgets to these ids; runtime
/// systems decide what the commands do through command/event routers.
pub const UI_ACTION_QUIT: &str = "engine.quit";
pub const UI_ACTION_OPEN_LOGS: &str = "engine.logs.open";
pub const UI_ACTION_RETRY_STARTUP: &str = "engine.startup.retry";
pub const UI_ACTION_START_GAME: &str = "game.start";
pub const UI_ACTION_RESUME_GAME: &str = "game.resume";
pub const UI_ACTION_TOGGLE_PRIMARY_UI: &str = "engine.ui.primary.toggle";
pub const UI_ACTION_CLOSE_MODAL: &str = "ui.modal.close";
pub const UI_ACTION_TOGGLE_DEBUG_OVERLAY: &str = "runtime.debug.toggle";

/// Provider-owned catalog describing every surface, layout, action and theme it
/// can render. This is the high-level replacement point for the whole UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiProviderCatalog {
    pub version: u32,
    pub provider: UiProviderBinding,
    pub surfaces: Vec<UiSurfaceDeclaration>,
    pub layouts: Vec<UiLayoutDeclaration>,
    pub actions: Vec<UiActionDeclaration>,
    pub themes: Vec<UiThemeDeclaration>,
}

impl UiProviderCatalog {
    #[inline]
    pub fn empty(provider: UiProviderBinding) -> Self {
        Self {
            version: 1,
            provider,
            surfaces: Vec::new(),
            layouts: Vec::new(),
            actions: Vec::new(),
            themes: Vec::new(),
        }
    }

    #[inline]
    pub fn from_manifest(manifest: UiProviderManifest) -> Self {
        let surfaces = manifest
            .surfaces
            .iter()
            .map(|id| UiSurfaceDeclaration {
                id: id.clone(),
                component_id: default_component_for_id(id).to_owned(),
                state_contract: state_contract_for_id(id).to_owned(),
                layout_id: default_layout_for_id(id).to_owned(),
                z_order: z_order_for_id(id),
                modal: id == UI_SURFACE_ENGINE_ERROR_MODAL,
                consumes: default_consumes_for_id(id),
            })
            .collect();

        Self {
            version: manifest.version,
            provider: manifest.provider,
            surfaces,
            layouts: Vec::new(),
            actions: Vec::new(),
            themes: Vec::new(),
        }
    }

    #[inline]
    pub fn supports_surface(&self, surface_id: &str) -> bool {
        self.surfaces.iter().any(|surface| surface.id == surface_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSurfaceDeclaration {
    pub id: String,
    pub component_id: String,
    pub state_contract: String,
    pub layout_id: String,
    pub z_order: i32,
    pub modal: bool,
    pub consumes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiLayoutDeclaration {
    pub id: String,
    pub surface_id: String,
    pub document: String,
    #[serde(default)]
    pub style_document: Option<String>,
    #[serde(default)]
    pub document_source: UiDocumentSourceKind,
    pub hot_reload: bool,
    pub fallback_document: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiActionDeclaration {
    pub id: String,
    pub label: String,
    pub route: UiActionRoute,
    pub enabled_when: Option<String>,
    pub visible_when: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiActionRoute {
    pub target: String,
    pub event: String,
    pub payload_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiThemeDeclaration {
    pub id: String,
    pub display_name: String,
    pub token_document: String,
}

/// Full declarative layout document. This is intentionally provider-neutral:
/// any provider implementation can map
/// the same tree into its own renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiDeclarativeLayout {
    pub version: u32,
    pub id: String,
    pub surface_id: String,
    pub theme: String,
    pub root: UiNodeSpec,
    pub data_sources: Vec<UiDataSourceBinding>,
    pub actions: Vec<UiActionBinding>,
}

impl UiDeclarativeLayout {
    #[inline]
    pub fn new(id: impl Into<String>, surface_id: impl Into<String>, theme: impl Into<String>, root: UiNodeSpec) -> Self {
        Self {
            version: 1,
            id: id.into(),
            surface_id: surface_id.into(),
            theme: theme.into(),
            root,
            data_sources: Vec::new(),
            actions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiNodeSpec {
    pub id: String,
    pub component_id: String,
    pub role: String,
    pub style: String,
    pub layout: UiLayoutBoxSpec,
    pub text: Option<String>,
    pub icon: Option<String>,
    pub image: Option<String>,
    pub bindings: Vec<UiDataBinding>,
    pub actions: Vec<UiActionBindingRef>,
    pub children: Vec<UiNodeSpec>,
}

impl UiNodeSpec {
    #[inline]
    pub fn new(id: impl Into<String>, component_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            component_id: component_id.into(),
            role: String::new(),
            style: String::new(),
            layout: UiLayoutBoxSpec::default(),
            text: None,
            icon: None,
            image: None,
            bindings: Vec::new(),
            actions: Vec::new(),
            children: Vec::new(),
        }
    }

    #[inline]
    pub fn with_child(mut self, child: UiNodeSpec) -> Self {
        self.children.push(child);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UiLayoutBoxSpec {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub anchor: UiAnchor,
}

impl Default for UiLayoutBoxSpec {
    #[inline]
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            anchor: UiAnchor::Fill,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiAnchor {
    Fill,
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiDataSourceBinding {
    pub id: String,
    pub source: String,
    pub contract: String,
    pub update: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiDataBinding {
    pub property: String,
    pub source: String,
    pub fallback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiActionBinding {
    pub id: String,
    pub action: String,
    pub trigger: String,
    pub when: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiActionBindingRef {
    pub trigger: String,
    pub action: String,
}

fn default_component_for_id(_id: &str) -> &'static str {
    // Public UI catalog entries are all the same node foundation. Providers may
    // attach templates through component ids, not through hardcoded surface kinds.
    "surface"
}

fn state_contract_for_id(_id: &str) -> &'static str {
    "UiSurfaceNode"
}

fn default_layout_for_id(id: &str) -> &'static str {
    match id {
        UI_SURFACE_ENGINE_LOADING => "assets/ui/engine/loading.neui@surface",
        UI_SURFACE_ENGINE_ERROR_MODAL => "assets/ui/engine/error_modal.neui@surface",
        UI_SURFACE_RUNTIME_OVERLAY => "assets/ui/runtime/overlay.neui@surface",
        UI_SURFACE_GAME_HUD => "assets/ui/game/hud.neui@surface",
        UI_SURFACE_DEBUG_OVERLAY => "assets/ui/runtime/debug_overlay.neui@surface",
        _ => "",
    }
}

fn z_order_for_id(id: &str) -> i32 {
    match id {
        UI_SURFACE_ENGINE_LOADING => 900,
        UI_SURFACE_ENGINE_ERROR_MODAL => 1000,
        UI_SURFACE_DEBUG_OVERLAY => 850,
        UI_SURFACE_RUNTIME_OVERLAY => 700,
        UI_SURFACE_GAME_HUD => 500,
        _ => 100,
    }
}

fn default_consumes_for_id(_id: &str) -> Vec<String> {
    vec!["UiSurfaceNode".to_owned()]
}
