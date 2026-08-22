// Provider-neutral Game GUI layer stack contracts.
// Included flat from lib.rs to preserve the public newengine-ui-api surface.

/// Logical primary game viewport identity used when a screen profile does not
/// provide an explicit viewport surface.
///
/// This is intentionally a logical surface id, not a render-graph resource or GPU
/// handle. Physical resource mapping stays inside the renderer/compiler so future
/// transient allocation and aliasing can be driven by authoritative lifetime data.
pub const UI_GAME_VIEWPORT_SURFACE_PRIMARY: &str = "engine.render.viewport.primary";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiGameLayerKind {
    #[default]
    Hud,
    Overlay,
    Menu,
    Modal,
}

impl UiGameLayerKind {
    #[inline]
    pub const fn default_z_order(self) -> i32 {
        match self {
            Self::Hud => 100,
            Self::Overlay => 300,
            Self::Menu => 600,
            Self::Modal => 1_000,
        }
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hud => "hud",
            Self::Overlay => "overlay",
            Self::Menu => "menu",
            Self::Modal => "modal",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiGameInputMode {
    /// Viewport/game owns interaction. UI remains visible but does not gate gameplay.
    #[default]
    GameOnly,
    /// UI and gameplay can both receive input. Suitable for interactive overlays.
    GameAndUi,
    /// UI owns interaction; gameplay movement/camera navigation are gated.
    UiOnly,
}

impl UiGameInputMode {
    #[inline]
    pub const fn blocks_gameplay(self) -> bool {
        matches!(self, Self::UiOnly)
    }

    #[inline]
    pub const fn requests_ui_focus(self) -> bool {
        !matches!(self, Self::GameOnly)
    }
}

/// Authored Game GUI layer attached to the logical game viewport.
///
/// The descriptor deliberately carries only stable UI identities and policy. It
/// never exposes physical textures, transient render targets, descriptor sets or
/// backend-owned objects across the UI/runtime boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiGameLayerDescriptor {
    pub id: String,
    pub kind: UiGameLayerKind,
    pub document_ref: String,
    pub surface_id: String,
    /// Zero selects the engine default for the layer kind. Authored `.neui`
    /// should currently declare the same effective z-order; provider-side override
    /// is intentionally a later protocol extension.
    pub z_order: i32,
    pub visible: bool,
    pub input_mode: UiGameInputMode,
}

impl Default for UiGameLayerDescriptor {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: UiGameLayerKind::Hud,
            document_ref: String::new(),
            surface_id: String::new(),
            z_order: 0,
            visible: true,
            input_mode: UiGameInputMode::GameOnly,
        }
    }
}

impl UiGameLayerDescriptor {
    pub fn new(
        id: impl Into<String>,
        kind: UiGameLayerKind,
        document_ref: impl Into<String>,
        surface_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            document_ref: document_ref.into(),
            surface_id: surface_id.into(),
            ..Self::default()
        }
    }

    pub fn hud(
        id: impl Into<String>,
        document_ref: impl Into<String>,
        surface_id: impl Into<String>,
    ) -> Self {
        Self::new(id, UiGameLayerKind::Hud, document_ref, surface_id)
    }

    pub fn overlay(
        id: impl Into<String>,
        document_ref: impl Into<String>,
        surface_id: impl Into<String>,
    ) -> Self {
        Self::new(id, UiGameLayerKind::Overlay, document_ref, surface_id)
    }

    pub fn menu(
        id: impl Into<String>,
        document_ref: impl Into<String>,
        surface_id: impl Into<String>,
    ) -> Self {
        Self::new(id, UiGameLayerKind::Menu, document_ref, surface_id)
            .with_input_mode(UiGameInputMode::UiOnly)
    }

    pub fn modal(
        id: impl Into<String>,
        document_ref: impl Into<String>,
        surface_id: impl Into<String>,
    ) -> Self {
        Self::new(id, UiGameLayerKind::Modal, document_ref, surface_id)
            .with_input_mode(UiGameInputMode::UiOnly)
    }

    #[inline]
    pub const fn effective_z_order(&self) -> i32 {
        if self.z_order == 0 {
            self.kind.default_z_order()
        } else {
            self.z_order
        }
    }

    #[inline]
    pub fn with_z_order(mut self, z_order: i32) -> Self {
        self.z_order = z_order;
        self
    }

    #[inline]
    pub fn with_input_mode(mut self, input_mode: UiGameInputMode) -> Self {
        self.input_mode = input_mode;
        self
    }

    #[inline]
    pub fn initially_hidden(mut self) -> Self {
        self.visible = false;
        self
    }
}

/// Declarative Game GUI composition.
///
/// This is deliberately small: projects configure authored layers, while the
/// active screen profile owns the logical viewport identity and runtime-host owns
/// mounting/focus lifecycle.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiGameGuiConfig {
    pub enabled: bool,
    pub layers: Vec<UiGameLayerDescriptor>,
}

impl UiGameGuiConfig {
    #[inline]
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    #[inline]
    pub fn with_layer(mut self, layer: UiGameLayerDescriptor) -> Self {
        self.layers.push(layer);
        self
    }

    pub fn simple_hud(document_ref: impl Into<String>, surface_id: impl Into<String>) -> Self {
        Self::enabled().with_layer(UiGameLayerDescriptor::hud("hud", document_ref, surface_id))
    }

    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if !self.enabled {
            return errors;
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut surfaces = std::collections::BTreeSet::new();
        for (index, layer) in self.layers.iter().enumerate() {
            let id = layer.id.trim();
            if id.is_empty() {
                errors.push(format!("game_gui.layers[{index}] has an empty id"));
            } else if !ids.insert(id.to_owned()) {
                errors.push(format!("duplicate game_gui layer id '{id}'"));
            }
            if layer.document_ref.trim().is_empty() {
                errors.push(format!("game_gui layer '{id}' has an empty document_ref"));
            }
            let surface = layer.surface_id.trim();
            if surface.is_empty() {
                errors.push(format!("game_gui layer '{id}' has an empty surface_id"));
            } else if !surfaces.insert(surface.to_owned()) {
                errors.push(format!("duplicate game_gui surface_id '{surface}'"));
            }
        }
        errors
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.validation_errors().is_empty()
    }

    pub fn resolved_layers(&self) -> Vec<UiGameLayerDescriptor> {
        let mut layers = self.layers.clone();
        for layer in &mut layers {
            if layer.z_order == 0 {
                layer.z_order = layer.kind.default_z_order();
            }
        }
        layers.sort_by_key(|layer| layer.z_order);
        layers
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiGameLayerCommandKind {
    Show,
    Hide,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiGameLayerCommand {
    pub layer_id: String,
    pub kind: UiGameLayerCommandKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiGameLayerCommandQueue {
    pub commands: Vec<UiGameLayerCommand>,
}

impl UiGameLayerCommandQueue {
    #[inline]
    pub fn push(&mut self, layer_id: impl Into<String>, kind: UiGameLayerCommandKind) {
        self.commands.push(UiGameLayerCommand {
            layer_id: layer_id.into(),
            kind,
        });
    }

    #[inline]
    pub fn show(&mut self, layer_id: impl Into<String>) {
        self.push(layer_id, UiGameLayerCommandKind::Show);
    }

    #[inline]
    pub fn hide(&mut self, layer_id: impl Into<String>) {
        self.push(layer_id, UiGameLayerCommandKind::Hide);
    }

    #[inline]
    pub fn toggle(&mut self, layer_id: impl Into<String>) {
        self.push(layer_id, UiGameLayerCommandKind::Toggle);
    }
}

/// Resolved runtime view of the Game GUI stack.
///
/// `viewport_surface_id` is a logical render/UI rendezvous point. It is stable
/// across backend changes and can later be mapped to a transient physical resource
/// after RenderGraph lifetime analysis determines the exact live resource history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiGameLayerStackState {
    pub version: u32,
    pub frame_index: u64,
    pub enabled: bool,
    pub viewport_surface_id: String,
    pub layers: Vec<UiGameLayerDescriptor>,
    pub active_input_mode: UiGameInputMode,
    pub top_visible_layer_id: Option<String>,
    pub input_owner_surface_id: Option<String>,
    pub top_modal_surface_id: Option<String>,
}

impl Default for UiGameLayerStackState {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            enabled: false,
            viewport_surface_id: UI_GAME_VIEWPORT_SURFACE_PRIMARY.to_owned(),
            layers: Vec::new(),
            active_input_mode: UiGameInputMode::GameOnly,
            top_visible_layer_id: None,
            input_owner_surface_id: None,
            top_modal_surface_id: None,
        }
    }
}

impl UiGameLayerStackState {
    pub fn from_config(config: &UiGameGuiConfig, frame_index: u64) -> Self {
        Self::from_config_for_viewport(config, UI_GAME_VIEWPORT_SURFACE_PRIMARY, frame_index)
    }

    pub fn from_config_for_viewport(
        config: &UiGameGuiConfig,
        viewport_surface_id: impl Into<String>,
        frame_index: u64,
    ) -> Self {
        let viewport_surface_id = viewport_surface_id.into();
        let viewport_surface_id = if viewport_surface_id.trim().is_empty() {
            UI_GAME_VIEWPORT_SURFACE_PRIMARY.to_owned()
        } else {
            viewport_surface_id
        };

        if !config.enabled || !config.is_valid() {
            return Self {
                frame_index,
                viewport_surface_id,
                ..Self::default()
            };
        }

        let layers = config.resolved_layers();
        let top_visible_layer_id = layers
            .iter()
            .rev()
            .find(|layer| layer.visible)
            .map(|layer| layer.id.clone());
        let top_input_layer = layers
            .iter()
            .rev()
            .find(|layer| layer.visible && layer.input_mode.requests_ui_focus());
        let top_input = top_input_layer
            .map(|layer| layer.input_mode)
            .unwrap_or(UiGameInputMode::GameOnly);
        let input_owner_surface_id = top_input_layer.map(|layer| layer.surface_id.clone());
        let top_modal_surface_id = layers
            .iter()
            .rev()
            .find(|layer| layer.visible && layer.kind == UiGameLayerKind::Modal)
            .map(|layer| layer.surface_id.clone());

        Self {
            version: 1,
            frame_index,
            enabled: true,
            viewport_surface_id,
            layers,
            active_input_mode: top_input,
            top_visible_layer_id,
            input_owner_surface_id,
            top_modal_surface_id,
        }
    }

    /// Resolve this authored/runtime stack into the generic engine presentation-layer plan.
    /// Runtime-host consumes the plan; it no longer needs Game-HUD-specific surface filtering.
    pub fn composition_plan(&self, invalidation_revision: u64) -> UiLayerCompositionPlan {
        if !self.enabled {
            return UiLayerCompositionPlan::disabled(
                UiLayerDomain::GameViewport,
                self.viewport_surface_id.clone(),
                self.frame_index,
            );
        }
        UiLayerCompositionPlan {
            version: 1,
            frame_index: self.frame_index,
            domain: UiLayerDomain::GameViewport,
            target_surface_id: self.viewport_surface_id.clone(),
            surface_ids: self
                .layers
                .iter()
                .filter(|layer| layer.visible)
                .map(|layer| layer.surface_id.clone())
                .filter(|surface_id| !surface_id.trim().is_empty())
                .collect(),
            invalidation_revision,
            input_owner_surface_id: self.input_owner_surface_id.clone(),
            modal_surface_id: self.top_modal_surface_id.clone(),
        }
    }
}

#[cfg(test)]
mod game_gui_tests {
    use super::*;

    fn layer(
        id: &str,
        kind: UiGameLayerKind,
        input_mode: UiGameInputMode,
    ) -> UiGameLayerDescriptor {
        UiGameLayerDescriptor {
            id: id.to_owned(),
            kind,
            document_ref: format!("ui/game/{id}.neui@surface"),
            surface_id: format!("game.{id}"),
            input_mode,
            ..UiGameLayerDescriptor::default()
        }
    }

    #[test]
    fn layer_stack_resolves_ue_like_order_and_top_input_owner() {
        let config = UiGameGuiConfig {
            enabled: true,
            layers: vec![
                layer("pause", UiGameLayerKind::Menu, UiGameInputMode::UiOnly),
                layer("hud", UiGameLayerKind::Hud, UiGameInputMode::GameOnly),
                layer(
                    "overlay",
                    UiGameLayerKind::Overlay,
                    UiGameInputMode::GameAndUi,
                ),
            ],
        };
        let state = UiGameLayerStackState::from_config(&config, 7);
        assert_eq!(state.layers[0].id, "hud");
        assert_eq!(state.layers[1].id, "overlay");
        assert_eq!(state.layers[2].id, "pause");
        assert_eq!(state.top_visible_layer_id.as_deref(), Some("pause"));
        assert_eq!(state.active_input_mode, UiGameInputMode::UiOnly);
        assert_eq!(state.input_owner_surface_id.as_deref(), Some("game.pause"));
    }

    #[test]
    fn simple_hud_builder_is_enabled_and_non_blocking() {
        let config = UiGameGuiConfig::simple_hud("ui/game/hud.neui@surface", "game.hud");
        assert!(config.enabled);
        assert!(config.is_valid());
        assert_eq!(config.layers.len(), 1);
        assert_eq!(config.layers[0].kind, UiGameLayerKind::Hud);
        assert_eq!(config.layers[0].input_mode, UiGameInputMode::GameOnly);
    }

    #[test]
    fn menu_builder_defaults_to_ui_only_and_can_start_hidden() {
        let menu = UiGameLayerDescriptor::menu("pause", "ui/game/pause.neui@surface", "game.pause")
            .initially_hidden();
        assert_eq!(menu.kind, UiGameLayerKind::Menu);
        assert_eq!(menu.input_mode, UiGameInputMode::UiOnly);
        assert!(!menu.visible);
    }

    #[test]
    fn layer_stack_keeps_logical_viewport_binding() {
        let config = UiGameGuiConfig::simple_hud("ui/game/hud.neui@surface", "game.hud");
        let state = UiGameLayerStackState::from_config_for_viewport(
            &config,
            "engine.render.viewport.player0",
            9,
        );
        assert_eq!(state.viewport_surface_id, "engine.render.viewport.player0");
    }

    #[test]
    fn layer_command_queue_is_provider_neutral() {
        let mut queue = UiGameLayerCommandQueue::default();
        queue.show("pause");
        queue.toggle("map");
        queue.hide("confirm");
        assert_eq!(queue.commands.len(), 3);
        assert_eq!(queue.commands[0].kind, UiGameLayerCommandKind::Show);
        assert_eq!(queue.commands[1].kind, UiGameLayerCommandKind::Toggle);
        assert_eq!(queue.commands[2].kind, UiGameLayerCommandKind::Hide);
    }

    #[test]
    fn game_stack_builds_engine_layer_composition_plan() {
        let config = UiGameGuiConfig {
            enabled: true,
            layers: vec![
                layer("hud", UiGameLayerKind::Hud, UiGameInputMode::GameOnly),
                layer("pause", UiGameLayerKind::Menu, UiGameInputMode::UiOnly),
            ],
        };
        let state = UiGameLayerStackState::from_config_for_viewport(
            &config,
            "engine.render.viewport.player0",
            42,
        );
        let plan = state.composition_plan(9);
        assert_eq!(plan.domain, UiLayerDomain::GameViewport);
        assert_eq!(plan.target_surface_id, "engine.render.viewport.player0");
        assert_eq!(plan.surface_ids, vec!["game.hud", "game.pause"]);
        assert_eq!(plan.invalidation_revision, 9);
        assert_eq!(plan.input_owner_surface_id.as_deref(), Some("game.pause"));
    }

    #[test]
    fn duplicate_surface_ids_are_rejected() {
        let a = layer("a", UiGameLayerKind::Hud, UiGameInputMode::GameOnly);
        let mut b = layer("b", UiGameLayerKind::Overlay, UiGameInputMode::GameOnly);
        b.surface_id = a.surface_id.clone();
        let config = UiGameGuiConfig {
            enabled: true,
            layers: vec![a, b],
        };
        assert!(config
            .validation_errors()
            .iter()
            .any(|error| error.contains("duplicate game_gui surface_id")));
    }
}
