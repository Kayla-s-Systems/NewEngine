use super::*;

const SCRIPT_EDITOR_NODE_ID: &str = "editor.script_editor.code";
const SCRIPT_EDITOR_SLOT_ID: &str = "bottom.script_editor";
const SCRIPT_EDITOR_REQUESTER: &str = "engine.editor.script_editor";

impl ScreenProfileRuntimeState {
    pub(super) fn update_script_editor_interaction(
        &mut self,
        resources: &mut Resources,
        frame_index: u64,
    ) {
        if !editing_tools_available(resources) || !in_game_editor_active(resources) {
            return;
        }

        self.update_bottom_tab_activation(resources, frame_index);
        self.open_selected_typescript_document(resources);

        let mut editor_changed = false;
        let mut cursor_changed = false;
        let mut completion_dismissed = false;
        if let Some(dispatch) = resources.get::<UiEventDispatchFrame>() {
            for patch in &dispatch.state_patches {
                if patch.surface_id != UI_SURFACE_EDITOR_SHELL {
                    continue;
                }
                for change in &patch.changes {
                    match change.path.as_str() {
                        "nodes.editor.script_editor.code/value" => {
                            if let (Some(session), Some(value)) =
                                (self.script_editor.session.as_mut(), change.value.as_str())
                            {
                                if session.source_text() != value {
                                    session.set_source_text(value.to_owned());
                                    editor_changed = true;
                                }
                            }
                        }
                        "nodes.editor.script_editor.code/cursor_byte_offset" => {
                            if let (Some(session), Some(cursor)) =
                                (self.script_editor.session.as_mut(), change.value.as_u64())
                            {
                                if session.set_cursor_byte_offset(cursor as usize).is_ok() {
                                    cursor_changed = true;
                                }
                            }
                        }
                        "nodes.editor.script_editor.code/completion_selected_index" => {
                            if let (Some(session), Some(index)) =
                                (self.script_editor.session.as_mut(), change.value.as_u64())
                            {
                                let item_count = session.popup().items.len();
                                session.popup_mut().selected_index = if item_count == 0 {
                                    None
                                } else {
                                    Some((index as usize).min(item_count - 1))
                                };
                            }
                        }
                        "nodes.editor.script_editor.code/completion_visible" => {
                            if change.value.as_bool() == Some(false) {
                                if let Some(session) = self.script_editor.session.as_mut() {
                                    session.dismiss_completion();
                                }
                                completion_dismissed = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let save_clicked =
            clicked_dispatch_action(resources, "editor.script_editor.save").is_some();
        let save_shortcut = resources
            .get::<UiEventDispatchFrame>()
            .and_then(|dispatch| dispatch.focused_node.as_ref())
            .is_some_and(|hit| hit.node_id == SCRIPT_EDITOR_NODE_ID)
            && resources.get::<UiInputFrame>().is_some_and(|input| {
                let control_down = input.is_key_down(newengine_input_api::key_code::CONTROL_LEFT)
                    || input.is_key_down(newengine_input_api::key_code::CONTROL_RIGHT);
                control_down && input.is_key_pressed(newengine_input_api::key_code::KEY_S)
            });
        if save_clicked || save_shortcut {
            self.save_script_editor_document();
        }

        if editor_changed || cursor_changed {
            self.refresh_script_editor_tooling();
            if completion_dismissed {
                // Refresh signature help for the accepted text/caret, but preserve the
                // renderer's explicit Escape/accept dismissal across this frame.
                if let Some(session) = self.script_editor.session.as_mut() {
                    session.dismiss_completion();
                }
            }
        }
    }

    fn update_bottom_tab_activation(&mut self, resources: &Resources, frame_index: u64) {
        let Some(action_id) = clicked_dispatch_action(resources, "editor.bottom.activate.") else {
            return;
        };
        let slot = action_id.trim_start_matches("editor.bottom.activate.");
        if !self
            .descriptor
            .panels
            .iter()
            .any(|panel| panel.slot_id == slot)
        {
            return;
        }
        if self.hidden_panels.contains(slot) {
            self.hidden_panels.remove(slot);
        }
        self.active_bottom_panel = slot.to_owned();
        self.last_dock_click_frame = frame_index;
    }

    fn open_selected_typescript_document(&mut self, resources: &Resources) {
        let selection = resources
            .get::<EditorSelectionContext>()
            .cloned()
            .unwrap_or_else(EditorSelectionContext::none);
        if !matches!(
            selection.kind,
            EditorSelectionKind::Asset | EditorSelectionKind::AssetEntry
        ) {
            return;
        }
        let reference = selection.reference.trim();
        if reference.is_empty() || reference.contains('@') || !is_typescript_source_ref(reference) {
            return;
        }
        if self.script_editor.asset_ref == reference && self.script_editor.session.is_some() {
            return;
        }

        let client = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        let document = match client.inspect_document_json_v1(AssetDocumentRequest {
            asset_ref: selection.reference.clone(),
            requester: SCRIPT_EDITOR_REQUESTER.to_owned(),
            ..AssetDocumentRequest::default()
        }) {
            Ok(document) => document,
            Err(error) => {
                self.script_editor.status =
                    format!("TypeScript asset '{reference}' inspect failed: {error}");
                return;
            }
        };
        let Some(text) = document.text.as_ref() else {
            self.script_editor.status = format!(
                "Asset '{}' has no UTF-8 text projection",
                document.asset_ref
            );
            return;
        };
        if !text.language.eq_ignore_ascii_case("typescript") || text.truncated {
            self.script_editor.status = format!(
                "Asset '{}' is not an editable full TypeScript text document",
                document.asset_ref
            );
            return;
        }

        let mut session = ScriptCodeEditorSession::new(
            document.asset_ref.clone(),
            "typescript",
            text.content.clone(),
        );
        let _ = session.set_cursor_byte_offset(0);
        self.script_editor.asset_ref = document.asset_ref.clone();
        self.script_editor.title = if document.title.trim().is_empty() {
            document.asset_ref.clone()
        } else {
            document.title.clone()
        };
        self.script_editor.editable = text.editable && document.editable;
        self.script_editor.original_text = text.content.clone();
        self.script_editor.session = Some(session);
        self.script_editor.signature_help = None;
        self.script_editor.last_completion_revision = 0;
        self.script_editor.last_signature_revision = 0;
        self.script_editor.status = if self.script_editor.editable {
            format!(
                "Opened {} · authored TypeScript · Ctrl+S saves through engine.assets",
                document.asset_ref
            )
        } else {
            format!("Opened {} · read-only TypeScript", document.asset_ref)
        };
        self.active_bottom_panel = SCRIPT_EDITOR_SLOT_ID.to_owned();
        self.hidden_panels.remove(SCRIPT_EDITOR_SLOT_ID);

        let tooling = ScriptingToolingClient::new();
        match tooling.refresh_generated_northstar_catalog() {
            Ok(catalog) => {
                self.script_editor.tooling_catalog_revision = catalog.revision;
                self.script_editor.status = format!(
                    "{} · NorthStar API catalog functions={} revision={:016x}",
                    self.script_editor.status,
                    catalog.functions.len(),
                    catalog.revision
                );
            }
            Err(error) => {
                self.script_editor.status = format!(
                    "{} · API catalog unavailable: {}",
                    self.script_editor.status, error
                );
            }
        }
        self.refresh_script_editor_tooling();
    }

    fn refresh_script_editor_tooling(&mut self) {
        let Some(session) = self.script_editor.session.as_mut() else {
            return;
        };
        let revision = session.document_revision();
        let tooling = ScriptingToolingClient::new();
        match session.refresh_completion(&tooling, None, 128) {
            Ok(_) => self.script_editor.last_completion_revision = revision,
            Err(error) => {
                self.script_editor.status = format!("Completion unavailable: {error}");
            }
        }

        let signature_request = ScriptingSignatureHelpRequest {
            module_ref: session.module_ref().to_owned(),
            language_id: session.language_id().to_owned(),
            source_text: session.source_text().to_owned(),
            document_revision: revision,
            cursor_byte_offset: session.cursor_byte_offset(),
            ..ScriptingSignatureHelpRequest::default()
        };
        match tooling.signature_help(&signature_request) {
            Ok(response) => {
                self.script_editor.signature_help = Some(response);
                self.script_editor.last_signature_revision = revision;
            }
            Err(error) => {
                self.script_editor.signature_help = None;
                self.script_editor.status = format!("Signature help unavailable: {error}");
            }
        }
    }

    fn save_script_editor_document(&mut self) {
        if !self.script_editor.editable {
            self.script_editor.status = "Script document is read-only".to_owned();
            return;
        }
        let Some(session) = self.script_editor.session.as_ref() else {
            return;
        };
        if !self.script_editor.dirty() {
            self.script_editor.status = "Script document has no changes".to_owned();
            return;
        }
        let logical_path = self.script_editor.asset_ref.clone();
        let text = session.source_text().to_owned();
        let client = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        match client.package_write_text_json_v1(TextAssetWriteRequestV1 {
            logical_path: logical_path.clone(),
            text: text.clone(),
            expected_hash: String::new(),
            requested_capability: ASSETS_PACKAGE_WRITER_CAPABILITY_ID.to_owned(),
        }) {
            Ok(response) if response.ok && response.written => {
                self.script_editor.original_text = text;
                self.script_editor.status = format!(
                    "Saved {} · {} bytes through {}",
                    response.logical_path,
                    response.bytes_written,
                    ASSETS_PACKAGE_WRITER_CAPABILITY_ID
                );
            }
            Ok(response) => {
                self.script_editor.status = response
                    .diagnostics
                    .last()
                    .cloned()
                    .unwrap_or_else(|| format!("Text writer rejected '{logical_path}'"));
            }
            Err(error) => {
                self.script_editor.status = format!("Failed to save {logical_path}: {error}");
            }
        }
    }

    pub(super) fn append_script_editor_panel(
        &mut self,
        node: &mut UiSurfaceNode,
        layout: &EditorLayoutMetrics,
    ) {
        if !layout.bottom_visible {
            return;
        }
        configure_bottom_tabs(node, &self.active_bottom_panel);

        if let Some(panel) = self
            .descriptor
            .panels
            .iter()
            .find(|panel| panel.slot_id == SCRIPT_EDITOR_SLOT_ID)
        {
            let mut tab = panel_component(
                panel,
                true,
                self.active_bottom_panel == SCRIPT_EDITOR_SLOT_ID,
            );
            tab.action_id = Some(format!("editor.bottom.activate.{SCRIPT_EDITOR_SLOT_ID}"));
            tab.tone = if self.active_bottom_panel == SCRIPT_EDITOR_SLOT_ID {
                UiNodeTone::Accent
            } else {
                UiNodeTone::Normal
            };
            node.components
                .push(with_rect(tab, 856.0, layout.bottom_y, 204.0, 28.0));
        }

        if self.active_bottom_panel != SCRIPT_EDITOR_SLOT_ID {
            return;
        }
        node.components
            .retain(|component| component.id != "editor.bottom.placeholder");

        let content_x = 14.0;
        let content_w = (layout.screen_w - 28.0).max(320.0);
        let content_top = layout.bottom_y + 34.0;
        let content_bottom = (layout.screen_h - layout.status_h - 6.0).max(content_top + 80.0);
        let header_h = 28.0;

        if let Some(session) = self.script_editor.session.as_ref() {
            let dirty = self.script_editor.dirty();
            node.components.push(with_rect(
                UiComponentNode::row(
                    "editor.script_editor.header",
                    if dirty {
                        format!("{} *", self.script_editor.title)
                    } else {
                        self.script_editor.title.clone()
                    },
                )
                .with_value(self.script_editor.asset_ref.clone())
                .with_detail(self.script_editor.status.clone())
                .with_tone(if dirty {
                    UiNodeTone::Accent
                } else {
                    UiNodeTone::Normal
                })
                .tagged("script-editor")
                .tagged(if dirty { "dirty" } else { "clean" }),
                content_x,
                content_top,
                (content_w - 106.0).max(180.0),
                header_h,
            ));
            node.components.push(with_rect(
                lively_editor_action(
                    UiComponentNode::action(
                        "editor.script_editor.save",
                        if dirty { "Save *" } else { "Save" },
                        "editor.script_editor.save",
                    )
                    .with_tooltip("Save authored TypeScript through engine.assets package writer")
                    .tagged("script-editor")
                    .tagged("save"),
                ),
                content_x + content_w - 98.0,
                content_top,
                94.0,
                header_h,
            ));

            let signature_label = self
                .script_editor
                .signature_help
                .as_ref()
                .and_then(|help| help.signatures.get(help.active_signature))
                .map(|signature| {
                    format!(
                        "{}  [arg {}]",
                        signature.label,
                        self.script_editor
                            .signature_help
                            .as_ref()
                            .map(|help| help.active_parameter + 1)
                            .unwrap_or(1)
                    )
                });
            let signature_h = if signature_label.is_some() { 24.0 } else { 0.0 };
            let editor_y = content_top + header_h + 4.0;
            let editor_h = (content_bottom - editor_y - signature_h - 4.0).max(72.0);
            let mut code = UiComponentNode::row(SCRIPT_EDITOR_NODE_ID, "")
                .with_value(session.source_text().to_owned())
                .with_detail("TypeScript source")
                .with_tone(UiNodeTone::Normal)
                .tagged("script-editor")
                .tagged("code-editor")
                .tagged("typescript");
            code.component_id = UI_COMPONENT_CODE_EDITOR.to_owned();
            code.props.insert(
                "cursor_byte_offset".to_owned(),
                serde_json::json!(session.cursor_byte_offset()),
            );
            code.props
                .insert("desired_rows".to_owned(), serde_json::json!(20));
            code.props
                .insert("max_height_px".to_owned(), serde_json::json!(editor_h));
            code.props.insert(
                "completion_visible".to_owned(),
                serde_json::json!(session.popup().visible),
            );
            code.props.insert(
                "completion_selected_index".to_owned(),
                serde_json::json!(session.popup().selected_index.unwrap_or(0)),
            );
            code.props.insert(
                "completion_items".to_owned(),
                serde_json::to_value(&session.popup().items)
                    .unwrap_or_else(|_| serde_json::json!([])),
            );
            node.components
                .push(with_rect(code, content_x, editor_y, content_w, editor_h));
            if let Some(label) = signature_label {
                node.components.push(with_rect(
                    UiComponentNode::row("editor.script_editor.signature", label)
                        .with_detail("signature help · engine.scripting")
                        .with_tone(UiNodeTone::Accent)
                        .tagged("script-editor")
                        .tagged("signature-help"),
                    content_x,
                    editor_y + editor_h + 2.0,
                    content_w,
                    signature_h,
                ));
            }
        } else {
            node.components.push(with_rect(
                UiComponentNode::row("editor.script_editor.empty", "No TypeScript document open")
                    .with_value("Select an authored .ts file in Content Browser")
                    .with_detail(self.script_editor.status.clone())
                    .with_tone(UiNodeTone::Normal)
                    .tagged("script-editor")
                    .tagged("empty-state"),
                content_x,
                content_top,
                content_w,
                52.0,
            ));
        }
    }
}

fn configure_bottom_tabs(node: &mut UiSurfaceNode, active: &str) {
    for component in &mut node.components {
        if !matches!(
            component.id.as_str(),
            "bottom.asset_browser"
                | "bottom.import_queue"
                | "bottom.output_log"
                | "bottom.profiler_diagnostics"
        ) {
            continue;
        }
        component.action_id = Some(format!("editor.bottom.activate.{}", component.id));
        component.tone = if component.id == active {
            UiNodeTone::Accent
        } else {
            UiNodeTone::Normal
        };
        component.props.insert(
            "active".to_owned(),
            serde_json::json!(component.id == active),
        );
    }
}

fn is_typescript_source_ref(reference: &str) -> bool {
    reference
        .split(['?', '#'])
        .next()
        .unwrap_or(reference)
        .to_ascii_lowercase()
        .ends_with(".ts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_authored_typescript_sources_activate_script_editor() {
        assert!(is_typescript_source_ref("scripts/player.ts"));
        assert!(is_typescript_source_ref("Scripts/Player.TS"));
        assert!(!is_typescript_source_ref("scripts/player.ysc"));
        assert!(!is_typescript_source_ref("scripts/player.js"));
    }

    #[test]
    fn bottom_tab_configuration_uses_activation_not_visibility_toggle() {
        let descriptor = editing_overlay_descriptor();
        let mut surface = UiSurfaceNode::default();
        let panel = descriptor
            .panels
            .iter()
            .find(|panel| panel.slot_id == "bottom.asset_browser")
            .unwrap();
        surface.components.push(panel_component(panel, true, false));
        configure_bottom_tabs(&mut surface, "bottom.asset_browser");
        assert_eq!(
            surface.components[0].action_id.as_deref(),
            Some("editor.bottom.activate.bottom.asset_browser")
        );
        assert_eq!(surface.components[0].tone, UiNodeTone::Accent);
    }
}
