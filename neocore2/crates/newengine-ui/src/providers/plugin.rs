#![forbid(unsafe_op_in_unsafe_fn)]

use crate::provider::{UiBuildFn, UiFrameDesc, UiFrameOutput, UiProvider, UiProviderKind};
use crate::schema::{
    UiActionDeclaration, UiActionRoute, UiLayoutDeclaration, UiProviderCatalog, UiThemeDeclaration, UI_ACTION_CLOSE_MODAL, UI_ACTION_OPEN_LOGS,
    UI_ACTION_TOGGLE_PRIMARY_UI, UI_ACTION_QUIT, UI_ACTION_RESUME_GAME, UI_ACTION_START_GAME,
    UI_ACTION_TOGGLE_DEBUG_OVERLAY, UI_SURFACE_DEBUG_OVERLAY, UI_SURFACE_GAME_HUD,
};
use crate::surface::{
    UiProviderManifest, UI_FEATURE_EXTERNAL_PLUGIN_PROVIDER, UI_SURFACE_ENGINE_ERROR_MODAL,
    UI_SURFACE_ENGINE_LOADING, UI_SURFACE_RUNTIME_OVERLAY,
};
use std::any::Any;

/// External UI provider placeholder.
///
/// The concrete drawing implementation is supplied by a runtime plugin service.
/// This object keeps the engine side provider selection replaceable while still
/// allowing the runtime to degrade to empty draw output until a concrete bridge
/// is bound.
pub struct PluginUiProvider {
    service_id: String,
}

impl PluginUiProvider {
    #[inline]
    pub fn new(service_id: impl Into<String>) -> Self {
        Self {
            service_id: service_id.into(),
        }
    }
}

fn action(id: &str, label: &str, target: &str) -> UiActionDeclaration {
    UiActionDeclaration {
        id: id.to_owned(),
        label: label.to_owned(),
        route: UiActionRoute {
            target: target.to_owned(),
            event: id.to_owned(),
            payload_schema: "application/json".to_owned(),
        },
        enabled_when: None,
        visible_when: None,
    }
}

impl UiProvider for PluginUiProvider {
    #[inline]
    fn kind(&self) -> UiProviderKind {
        UiProviderKind::Plugin {
            service_id: self.service_id.clone(),
        }
    }

    #[inline]
    fn manifest(&self) -> UiProviderManifest {
        UiProviderManifest {
            provider: self.binding(),
            version: 1,
            surfaces: vec![
                UI_SURFACE_ENGINE_LOADING.to_owned(),
                UI_SURFACE_ENGINE_ERROR_MODAL.to_owned(),
                UI_SURFACE_RUNTIME_OVERLAY.to_owned(),
                UI_SURFACE_GAME_HUD.to_owned(),
                UI_SURFACE_DEBUG_OVERLAY.to_owned(),
            ],
            features: vec![UI_FEATURE_EXTERNAL_PLUGIN_PROVIDER.to_owned()],
        }
    }

    #[inline]
    fn catalog(&self) -> UiProviderCatalog {
        let manifest = self.manifest();
        let mut catalog = UiProviderCatalog::from_manifest(manifest);
        catalog.layouts = vec![
            UiLayoutDeclaration { id: "engine.loading.ksystems".to_owned(), surface_id: UI_SURFACE_ENGINE_LOADING.to_owned(), document: "assets/ui/engine/loading.neui@surface".to_owned(), hot_reload: true, fallback_document: None },
            UiLayoutDeclaration { id: "engine.loading.subsystem_card.ksystems".to_owned(), surface_id: UI_SURFACE_ENGINE_LOADING.to_owned(), document: "assets/ui/engine/loading.neui@layout.subsystem_cards".to_owned(), hot_reload: true, fallback_document: None },
            UiLayoutDeclaration { id: "engine.error_modal.ksystems".to_owned(), surface_id: UI_SURFACE_ENGINE_ERROR_MODAL.to_owned(), document: "assets/ui/engine/error_modal.neui@surface".to_owned(), hot_reload: true, fallback_document: None },
            UiLayoutDeclaration { id: "game.hud.open_world".to_owned(), surface_id: UI_SURFACE_GAME_HUD.to_owned(), document: "assets/ui/game/hud.neui@surface".to_owned(), hot_reload: true, fallback_document: None },
        ];
        catalog.actions = vec![
            action(UI_ACTION_START_GAME, "Start", "GameCommand"),
            action(UI_ACTION_RESUME_GAME, "Resume", "GameCommand"),
            action(UI_ACTION_TOGGLE_PRIMARY_UI, "Primary UI", "GameCommand"),
            action(UI_ACTION_OPEN_LOGS, "Open Logs", "SystemCommand"),
            action(UI_ACTION_CLOSE_MODAL, "Close", "UiCommand"),
            action(UI_ACTION_TOGGLE_DEBUG_OVERLAY, "Debug", "RuntimeCommand"),
            action(UI_ACTION_QUIT, "Quit", "SystemCommand"),
        ];
        catalog.themes = vec![UiThemeDeclaration {
            id: "newengine.dark.gold-magenta".to_owned(),
            display_name: "NewEngine Dark / Gold-Magenta".to_owned(),
            token_document: "assets/ui/themes/north_star_dark.neui@theme".to_owned(),
        }];
        catalog
    }

    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    #[inline]
    fn run_frame(
        &mut self,
        _window: &dyn Any,
        frame: UiFrameDesc,
        build: &mut dyn UiBuildFn,
    ) -> UiFrameOutput {
        build.begin_frame(&frame);
        let mut ctx = ();
        build.build(&mut ctx);
        UiFrameOutput::empty()
    }
}
