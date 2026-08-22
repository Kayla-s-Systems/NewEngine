use super::*;

impl ScreenProfileRuntimeState {
    pub(super) fn append_right_edit_window(
        &mut self,
        resources: &Resources,
        node: &mut UiSurfaceNode,
        layout: &EditorLayoutMetrics,
    ) {
        let selection = resources
            .get::<EditorSelectionContext>()
            .cloned()
            .unwrap_or_else(EditorSelectionContext::none);
        self.refresh_right_edit_cache(&selection);
        let first_right_component = node.components.len();
        node.components.push(
            UiComponentNode::row("right_edit_window.header", "Right Edit Window")
                .with_value(selection.kind.as_str())
                .with_detail(if selection.reference.is_empty() {
                    "no active editor selection".to_owned()
                } else {
                    selection.reference.clone()
                })
                .with_tone(if selection.kind == EditorSelectionKind::None {
                    UiNodeTone::Normal
                } else {
                    UiNodeTone::Accent
                })
                .tagged("right")
                .tagged("edit-window")
                .tagged("selection-context"),
        );

        match selection.kind {
            EditorSelectionKind::None => {
                node.components.push(
                    UiComponentNode::row("right_edit_window.empty", "No selection")
                        .with_detail(
                            "viewport/outliner/content browser can publish EditorSelectionContext",
                        )
                        .with_tone(UiNodeTone::Normal)
                        .tagged("right")
                        .tagged("edit-window"),
                );
            }
            EditorSelectionKind::Entity => {
                node.components.push(
                    UiComponentNode::row("right_edit_window.entity.route", "Entity Component Editor")
                        .with_value(format!("{} + engine.entity", ENGINE_SCHEMA_SERVICE_ID))
                        .with_detail("component properties must come from schema.describe_properties_v1; native EntityId must not cross this boundary")
                        .with_tone(UiNodeTone::Accent)
                        .tagged("right")
                        .tagged("entity")
                        .tagged("opaque-handles"),
                );
            }
            EditorSelectionKind::Asset
            | EditorSelectionKind::AssetEntry
            | EditorSelectionKind::Material => {
                self.push_asset_document_components(node, &selection);
            }
            EditorSelectionKind::World => {
                node.components.push(
                    UiComponentNode::row("right_edit_window.world.route", "World Settings Editor")
                        .with_value(format!("{} + engine.world", ENGINE_SCHEMA_SERVICE_ID))
                        .with_detail("settings editor consumes schema properties and emits transaction DTO patches")
                        .with_tone(UiNodeTone::Accent)
                        .tagged("right")
                        .tagged("world"),
                );
            }
        }

        let right_x = layout.screen_w - layout.right_w + 10.0;
        let right_w = (layout.right_w - 28.0).max(160.0);
        let mut y = layout.viewport_y + 84.0;
        for component in node.components.iter_mut().skip(first_right_component) {
            let h = if component.state_tags.iter().any(|tag| tag == "asset-field") {
                24.0
            } else {
                34.0
            };
            set_rect(component, right_x, y, right_w, h);
            y += h + 5.0;
            if y > layout.bottom_y - 12.0 {
                break;
            }
        }
    }

    pub(super) fn refresh_right_edit_cache(&mut self, selection: &EditorSelectionContext) {
        let key = format!("{}:{}", selection.kind.as_str(), selection.reference);
        if self.last_right_edit_selection_key == key {
            return;
        }
        self.last_right_edit_selection_key = key;
        self.cached_right_edit_document = None;
        self.cached_right_edit_error = None;

        if !matches!(
            selection.kind,
            EditorSelectionKind::Asset
                | EditorSelectionKind::AssetEntry
                | EditorSelectionKind::Material
        ) {
            return;
        }
        if selection.reference.trim().is_empty() {
            self.cached_right_edit_error = Some("empty asset selection reference".to_owned());
            return;
        }

        let client = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        match client.inspect_document_json_v1(AssetDocumentRequest {
            asset_ref: selection.reference.clone(),
            requester: RIGHT_EDIT_WINDOW_OWNER.to_owned(),
            ..AssetDocumentRequest::default()
        }) {
            Ok(document) => self.cached_right_edit_document = Some(document),
            Err(error) => self.cached_right_edit_error = Some(error),
        }
    }

    pub(super) fn push_asset_document_components(
        &self,
        node: &mut UiSurfaceNode,
        selection: &EditorSelectionContext,
    ) {
        node.components.push(
            UiComponentNode::row("right_edit_window.asset.route", "Asset Document Editor")
                .with_value("engine.assets.inspect")
                .with_detail(format!(
                    "source={} semantic_gateway={}",
                    selection.source_surface, selection.semantic_gateway
                ))
                .with_tone(UiNodeTone::Accent)
                .tagged("right")
                .tagged("asset-document"),
        );

        if let Some(error) = self.cached_right_edit_error.as_ref() {
            node.components.push(
                UiComponentNode::row("right_edit_window.asset.error", "AssetDocument unavailable")
                    .with_value(error.clone())
                    .with_tone(UiNodeTone::Danger)
                    .tagged("right")
                    .tagged("diagnostic"),
            );
            return;
        }

        let Some(document) = self.cached_right_edit_document.as_ref() else {
            node.components.push(
                UiComponentNode::row("right_edit_window.asset.pending", "No AssetDocument DTO")
                    .with_detail("provider route missing or selection was not an asset")
                    .with_tone(UiNodeTone::Normal)
                    .tagged("right")
                    .tagged("asset-document"),
            );
            return;
        };

        node.components.push(
            UiComponentNode::row("right_edit_window.asset.header", document.title.clone())
                .with_value(document.document_kind.clone())
                .with_detail(format!(
                    "schema_editable={} can_apply_patch={} writer={}",
                    document.editable_fields_available,
                    document.can_apply_patch,
                    if document.writer_capability.is_empty() {
                        "missing"
                    } else {
                        document.writer_capability.as_str()
                    }
                ))
                .with_tone(if document.can_apply_patch {
                    UiNodeTone::Accent
                } else {
                    UiNodeTone::Normal
                })
                .tagged("right")
                .tagged("asset-document"),
        );
        node.components.push(
            UiComponentNode::row("right_edit_window.asset.contract", "Contract")
                .with_value(document.inspect_contract.clone())
                .with_detail(format!(
                    "edit_contract={} write_owner={}",
                    if document.edit_contract.is_empty() {
                        "none"
                    } else {
                        document.edit_contract.as_str()
                    },
                    document.write_owner
                ))
                .with_tone(UiNodeTone::Normal)
                .tagged("right")
                .tagged("asset-document"),
        );
        if let Some(schema_type) = document.schema_type.as_ref() {
            node.components.push(
                UiComponentNode::row("right_edit_window.asset.schema", "Schema Type")
                    .with_value(schema_type.type_id.clone())
                    .with_detail(format!(
                        "route={} contract={} properties={}",
                        ENGINE_SCHEMA_SERVICE_ID,
                        document.schema_contract,
                        schema_type.properties.len()
                    ))
                    .with_tone(UiNodeTone::Accent)
                    .tagged("right")
                    .tagged("asset-document")
                    .tagged("schema"),
            );
        }

        for section in document.sections.iter().take(3) {
            node.components.push(
                UiComponentNode::row(
                    format!(
                        "right_edit_window.asset.section.{}",
                        component_id_fragment(&section.id)
                    ),
                    section.title.clone(),
                )
                .with_value(format!("{} fields", section.fields.len()))
                .with_tone(UiNodeTone::Accent)
                .tagged("right")
                .tagged("asset-section"),
            );
            for field in section.fields.iter().take(4) {
                node.components.push(
                    UiComponentNode::row(
                        format!(
                            "right_edit_window.asset.field.{}.{}",
                            component_id_fragment(&section.id),
                            component_id_fragment(&field.id)
                        ),
                        field.label.clone(),
                    )
                    .with_value(asset_document_value_label(&field.value))
                    .with_detail(asset_document_field_detail(field))
                    .with_tone(if field.editable {
                        UiNodeTone::Accent
                    } else {
                        UiNodeTone::Normal
                    })
                    .tagged("right")
                    .tagged("asset-field")
                    .tagged("schema-property"),
                );
            }
        }
    }
}
