use super::*;

const GAME_GUI_CAPTURE_OWNER: &str = "screen_profile.game_gui";
const LEGACY_GAME_GUI_SURFACE_ID: &str = "game.hud";

impl ScreenProfileRuntimeState {
    fn resolved_game_gui_config(&self) -> Option<UiGameGuiConfig> {
        resolve_game_gui_config(
            self.descriptor.profile,
            self.config.game_gui.as_ref(),
            self.active_presentation_state().is_some(),
            self.descriptor.game_ui_document_ref.as_deref(),
            self.descriptor.game_ui_root_surface_id.as_deref(),
        )
    }

    pub(super) fn game_gui_stack_state(&self, frame_index: u64) -> UiGameLayerStackState {
        let viewport_surface_id = self.descriptor.viewport_surface_id.clone();
        if self.descriptor.profile != UiScreenProfile::Game {
            return UiGameLayerStackState {
                frame_index,
                viewport_surface_id,
                ..UiGameLayerStackState::default()
            };
        }
        self.resolved_game_gui_config()
            .as_ref()
            .map(|config| {
                UiGameLayerStackState::from_config_for_viewport(
                    config,
                    viewport_surface_id.clone(),
                    frame_index,
                )
            })
            .unwrap_or_else(|| UiGameLayerStackState {
                frame_index,
                viewport_surface_id,
                ..UiGameLayerStackState::default()
            })
    }

    pub(super) fn has_active_game_gui(&self) -> bool {
        self.descriptor.profile == UiScreenProfile::Game
            && self
                .resolved_game_gui_config()
                .as_ref()
                .is_some_and(|config| config.enabled && config.is_valid())
    }

    pub(super) fn prepare_game_gui(
        &mut self,
        resources: &mut Resources,
        frame_index: u64,
        profile_changed: bool,
    ) -> bool {
        let resolved_config = self.resolved_game_gui_config();
        let active = self.descriptor.profile == UiScreenProfile::Game
            && resolved_config
                .as_ref()
                .is_some_and(|config| config.enabled && config.is_valid());
        if !active {
            let was_enabled = resources
                .get::<UiGameLayerStackState>()
                .is_some_and(|state| state.enabled);
            let mut changed = false;
            if was_enabled || profile_changed {
                for surface_id in self.mounted_game_gui_layers.values() {
                    changed |= crate::platform_runtime::ui_gateway_frame::set_surface_visible(
                        surface_id, false,
                    );
                }
            }
            remove_input_capture_contribution(resources, GAME_GUI_CAPTURE_OWNER, None);
            resources.insert(UiGameLayerCommandQueue::default());
            resources.insert(UiGameLayerStackState {
                frame_index,
                viewport_surface_id: self.descriptor.viewport_surface_id.clone(),
                ..UiGameLayerStackState::default()
            });
            return changed;
        }

        if profile_changed {
            self.failed_game_gui_layers.clear();
        }

        let config = resolved_config.expect("active game gui config disappeared");
        let commands_changed = self.apply_game_gui_commands(resources, &config);
        let mut state = UiGameLayerStackState::from_config_for_viewport(
            &config,
            self.descriptor.viewport_surface_id.clone(),
            frame_index,
        );
        for layer in &mut state.layers {
            if let Some(visible) = self.game_gui_visibility_overrides.get(&layer.id) {
                layer.visible = *visible;
            }
        }
        let mut refresh = commands_changed;

        for layer in &mut state.layers {
            let layer_id = layer.id.clone();
            let configured_surface = layer.surface_id.clone();
            let actual_surface = if let Some(surface) = self.mounted_game_gui_layers.get(&layer_id)
            {
                Some(surface.clone())
            } else if self.failed_game_gui_layers.contains(&layer_id) {
                None
            } else {
                match self.mount_authored_ui_document(layer.document_ref.as_str()) {
                    Ok(surface_id) => {
                        if surface_id != configured_surface {
                            newengine_ulog_api::ulog::warn!(
                                "game gui: layer surface differs from config layer='{}' configured='{}' authored='{}'; authored surface becomes runtime identity",
                                layer_id,
                                configured_surface,
                                surface_id,
                            );
                        }
                        self.mounted_game_gui_layers
                            .insert(layer_id.clone(), surface_id.clone());
                        self.published_surfaces.insert(surface_id.clone());
                        refresh = true;
                        Some(surface_id)
                    }
                    Err(error) => {
                        self.failed_game_gui_layers.insert(layer_id.clone());
                        layer.visible = false;
                        newengine_ulog_api::ulog::warn!(
                            "game gui: authored layer mount failed layer='{}' kind='{}' ref='{}' err='{}'",
                            layer_id,
                            layer.kind.as_str(),
                            layer.document_ref,
                            error,
                        );
                        None
                    }
                }
            };

            if let Some(surface_id) = actual_surface {
                layer.surface_id = surface_id.clone();
                if profile_changed || refresh {
                    refresh |= crate::platform_runtime::ui_gateway_frame::set_surface_visible(
                        &surface_id,
                        layer.visible,
                    );
                }
            }
        }

        // Re-resolve z-order/input ownership after authored documents have supplied
        // their actual runtime surface identities. Keep the screen profile's logical
        // viewport binding intact; no physical render resource escapes this boundary.
        let mut resolved_config = config.clone();
        resolved_config.layers = state.layers;
        state = UiGameLayerStackState::from_config_for_viewport(
            &resolved_config,
            self.descriptor.viewport_surface_id.clone(),
            frame_index,
        );
        self.publish_game_gui_input_capture(resources, &state);
        resources.insert(state);
        refresh
    }

    fn apply_game_gui_commands(
        &mut self,
        resources: &mut Resources,
        config: &UiGameGuiConfig,
    ) -> bool {
        let mut queue = resources
            .remove::<UiGameLayerCommandQueue>()
            .unwrap_or_default();
        let mut changed = false;
        for command in queue.commands.drain(..) {
            let layer_id = command.layer_id.trim();
            let Some(base_layer) = config.layers.iter().find(|layer| layer.id == layer_id) else {
                newengine_ulog_api::ulog::warn!(
                    "game gui: ignored command for unknown layer='{}' kind={:?}",
                    layer_id,
                    command.kind,
                );
                continue;
            };
            let current = self
                .game_gui_visibility_overrides
                .get(layer_id)
                .copied()
                .unwrap_or(base_layer.visible);
            let next = match command.kind {
                UiGameLayerCommandKind::Show => true,
                UiGameLayerCommandKind::Hide => false,
                UiGameLayerCommandKind::Toggle => !current,
            };
            if next != current {
                self.game_gui_visibility_overrides
                    .insert(layer_id.to_owned(), next);
                changed = true;
            }
        }
        resources.insert(queue);
        changed
    }

    fn publish_game_gui_input_capture(
        &self,
        resources: &mut Resources,
        state: &UiGameLayerStackState,
    ) {
        if let Some(capture) = game_gui_input_capture(state) {
            set_input_capture_contribution(resources, GAME_GUI_CAPTURE_OWNER, capture);
        } else {
            remove_input_capture_contribution(resources, GAME_GUI_CAPTURE_OWNER, None);
        }
    }
}

/// Converts the existing single-document game UI configuration into the new
/// viewport-layer contract. This keeps the first integration step deliberately
/// small: existing projects automatically become a one-layer HUD stack, while an
/// explicit `game_gui` block remains authoritative when present.
fn resolve_game_gui_config(
    profile: UiScreenProfile,
    explicit: Option<&UiGameGuiConfig>,
    presentation_flow_active: bool,
    game_ui_document_ref: Option<&str>,
    game_ui_root_surface_id: Option<&str>,
) -> Option<UiGameGuiConfig> {
    if profile != UiScreenProfile::Game {
        return None;
    }
    if let Some(config) = explicit {
        return Some(config.clone());
    }
    // The legacy single-document adapter is a fallback only. A valid presentation
    // flow already owns the authored screen state and may itself mount the gameplay
    // HUD; adapting the same document here would mount it a second time.
    if presentation_flow_active {
        return None;
    }

    let document_ref = game_ui_document_ref
        .map(str::trim)
        .filter(|it| !it.is_empty())?;
    let surface_id = game_ui_root_surface_id
        .map(str::trim)
        .filter(|it| !it.is_empty())
        .unwrap_or(LEGACY_GAME_GUI_SURFACE_ID);
    Some(UiGameGuiConfig::simple_hud(document_ref, surface_id))
}

fn game_gui_input_capture(state: &UiGameLayerStackState) -> Option<UiInputCaptureState> {
    let layer = state
        .layers
        .iter()
        .rev()
        .find(|layer| layer.visible && layer.input_mode.requests_ui_focus())?;
    let mut capture = UiInputCaptureState::none();
    capture.gameplay_movement_gated = layer.input_mode.blocks_gameplay();
    // Non-modal UI may own keyboard/gameplay movement without stealing the possessed camera.
    // Camera look is gated only by a true modal layer.
    capture.camera_navigation_gated =
        layer.kind == UiGameLayerKind::Modal && layer.input_mode.blocks_gameplay();
    capture.modal = layer.kind == UiGameLayerKind::Modal;
    capture.draw_refresh_requested = true;
    capture.reason = format!(
        "game_gui.layer:{}:{}:{:?}",
        layer.id,
        layer.kind.as_str(),
        layer.input_mode,
    );
    capture.surfaces = vec![layer.surface_id.clone()];
    Some(capture)
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_ui_api::{UiGameInputMode, UiGameLayerDescriptor};

    fn state(kind: UiGameLayerKind, input_mode: UiGameInputMode) -> UiGameLayerStackState {
        UiGameLayerStackState::from_config_for_viewport(
            &UiGameGuiConfig {
                enabled: true,
                layers: vec![UiGameLayerDescriptor {
                    id: "test".to_owned(),
                    kind,
                    document_ref: "ui/game/test.neui@surface".to_owned(),
                    surface_id: "game.test".to_owned(),
                    visible: true,
                    input_mode,
                    ..UiGameLayerDescriptor::default()
                }],
            },
            "engine.render.viewport.test",
            1,
        )
    }

    #[test]
    fn game_only_layer_does_not_take_ui_focus() {
        assert!(
            game_gui_input_capture(&state(UiGameLayerKind::Hud, UiGameInputMode::GameOnly))
                .is_none()
        );
    }

    #[test]
    fn ui_only_modal_gates_gameplay_and_marks_modal_capture() {
        let capture =
            game_gui_input_capture(&state(UiGameLayerKind::Modal, UiGameInputMode::UiOnly))
                .expect("ui-only modal should own input");
        assert!(capture.gameplay_movement_gated);
        assert!(capture.camera_navigation_gated);
        assert!(capture.modal);
        assert_eq!(capture.surfaces, vec!["game.test"]);
    }

    #[test]
    fn non_modal_ui_only_overlay_preserves_camera_look() {
        let capture =
            game_gui_input_capture(&state(UiGameLayerKind::Overlay, UiGameInputMode::UiOnly))
                .expect("ui-only overlay should own UI focus");
        assert!(capture.gameplay_movement_gated);
        assert!(!capture.camera_navigation_gated);
        assert!(!capture.modal);
    }

    #[test]
    fn game_and_ui_overlay_keeps_gameplay_unblocked() {
        let capture =
            game_gui_input_capture(&state(UiGameLayerKind::Overlay, UiGameInputMode::GameAndUi))
                .expect("game-and-ui overlay should participate in UI focus");
        assert!(!capture.gameplay_movement_gated);
        assert!(!capture.camera_navigation_gated);
        assert!(!capture.modal);
    }

    #[test]
    fn game_gui_state_keeps_screen_profile_viewport_identity() {
        let state = state(UiGameLayerKind::Hud, UiGameInputMode::GameOnly);
        assert_eq!(state.viewport_surface_id, "engine.render.viewport.test");
    }

    #[test]
    fn legacy_single_document_becomes_simple_hud_layer() {
        let config = resolve_game_gui_config(
            UiScreenProfile::Game,
            None,
            false,
            Some("ui/game/game_hud.neui@surface"),
            Some("game.hud"),
        )
        .expect("legacy game document should resolve");
        assert!(config.enabled);
        assert!(config.is_valid());
        assert_eq!(config.layers.len(), 1);
        assert_eq!(config.layers[0].id, "hud");
        assert_eq!(config.layers[0].kind, UiGameLayerKind::Hud);
        assert_eq!(config.layers[0].surface_id, "game.hud");
        assert_eq!(config.layers[0].input_mode, UiGameInputMode::GameOnly);
    }

    #[test]
    fn explicit_game_gui_config_wins_over_legacy_document_and_presentation_flow() {
        let explicit = UiGameGuiConfig {
            enabled: true,
            layers: vec![UiGameLayerDescriptor::menu(
                "pause",
                "ui/game/pause.neui@surface",
                "game.pause",
            )],
        };
        let resolved = resolve_game_gui_config(
            UiScreenProfile::Game,
            Some(&explicit),
            true,
            Some("ui/game/legacy.neui@surface"),
            Some("game.legacy"),
        )
        .expect("explicit game gui should resolve");
        assert_eq!(resolved, explicit);
    }

    #[test]
    fn legacy_adapter_yields_to_active_presentation_flow() {
        assert!(resolve_game_gui_config(
            UiScreenProfile::Game,
            None,
            true,
            Some("ui/game/game_hud.neui@surface"),
            Some("game.hud"),
        )
        .is_none());
    }

    #[test]
    fn legacy_adapter_is_game_profile_only() {
        assert!(resolve_game_gui_config(
            UiScreenProfile::Editor,
            None,
            false,
            Some("ui/game/game_hud.neui@surface"),
            Some("game.hud"),
        )
        .is_none());
    }
}
