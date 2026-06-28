use super::*;

pub(super) fn editor_layout_metrics(
    resources: &Resources,
    hidden_panels: &BTreeSet<String>,
) -> EditorLayoutMetrics {
    let [w_u32, h_u32] = resources
        .get::<WindowInitSize>()
        .map(|size| [size.width.max(1), size.height.max(1)])
        .unwrap_or(DEFAULT_EDITOR_SURFACE_SIZE_PX);
    let w = w_u32 as f32;
    let h = h_u32 as f32;
    let menu_h = 28.0;
    let toolbar_h = 34.0;
    let status_h = 20.0;
    let gap = 5.0;
    let bottom_visible = !hidden_panels.contains("bottom.asset_browser");
    let left_visible = !hidden_panels.contains("left.scene_tree");
    let right_visible = !hidden_panels.contains("right.inspector");
    let bottom_h = if bottom_visible {
        (h * 0.24).clamp(176.0, 235.0)
    } else {
        0.0
    };
    let left_w = if left_visible {
        (w * 0.15).clamp(210.0, 268.0)
    } else {
        0.0
    };
    let right_w = if right_visible {
        (w * 0.21).clamp(286.0, 362.0)
    } else {
        0.0
    };
    let viewport_x = if left_visible { left_w + gap } else { gap };
    let viewport_y = menu_h + toolbar_h + gap;
    let viewport_w = (w
        - left_w
        - right_w
        - gap
            * if left_visible && right_visible {
                2.0
            } else {
                1.0
            })
    .max(360.0);
    let viewport_h = (h
        - menu_h
        - toolbar_h
        - status_h
        - bottom_h
        - gap * if bottom_visible { 3.0 } else { 2.0 })
    .max(180.0);
    let bottom_y = h - bottom_h - status_h - gap;
    let hovered_dock_slot = hovered_dock_slot_from_dispatch(resources);
    let hovered_runtime_mode = hovered_runtime_mode_from_dispatch(resources);
    let hovered_menu_id = hovered_menu_id_from_dispatch(resources);
    EditorLayoutMetrics {
        screen_w: w,
        screen_h: h,
        menu_h,
        toolbar_h,
        status_h,
        bottom_h,
        left_w,
        right_w,
        gap,
        viewport_x,
        viewport_y,
        viewport_w,
        viewport_h,
        bottom_y,
        left_visible,
        right_visible,
        bottom_visible,
        hovered_dock_slot,
        hovered_runtime_mode,
        hovered_menu_id,
    }
}

pub(super) fn clicked_dispatch_action(resources: &Resources, prefix: &str) -> Option<String> {
    resources
        .get::<UiEventDispatchFrame>()?
        .actions
        .iter()
        .find(|action| {
            action.trigger == UiNodeEventTrigger::Click && action.action_id.starts_with(prefix)
        })
        .map(|action| action.action_id.clone())
}

pub(super) fn hovered_action_id(resources: &Resources) -> Option<&str> {
    resources
        .get::<UiEventDispatchFrame>()?
        .hovered_node
        .as_ref()?
        .action_id
        .as_deref()
}

pub(super) fn hovered_dock_slot_from_dispatch(resources: &Resources) -> Option<&'static str> {
    let action_id = hovered_action_id(resources)?;
    let slot_id = action_id.strip_prefix("editor.dock.toggle.")?;
    [
        "left.scene_tree",
        "right.inspector",
        "bottom.asset_browser",
        "bottom.import_queue",
        "bottom.output_log",
        "bottom.profiler_diagnostics",
    ]
    .into_iter()
    .find(|slot| *slot == slot_id)
}

pub(super) fn hovered_runtime_mode_from_dispatch(
    resources: &Resources,
) -> Option<UiEditorRuntimeMode> {
    let action_id = hovered_action_id(resources)?;
    EDITOR_CHROME
        .runtime_actions
        .iter()
        .find(|action| action.action_id == action_id)
        .map(|action| action.mode)
}

pub(super) fn hovered_menu_id_from_dispatch(resources: &Resources) -> Option<&'static str> {
    let action_id = hovered_action_id(resources)?;
    let menu_id = action_id.strip_prefix("editor.menu.")?;
    EDITOR_CHROME
        .menu
        .iter()
        .find(|menu| menu.id == menu_id)
        .map(|menu| menu.id)
}

pub(super) fn dock_state(
    slot_id: &str,
    visible: bool,
    disabled: bool,
    hovered: bool,
) -> UiDockPanelRuntimeState {
    UiDockPanelRuntimeState {
        slot_id: slot_id.to_owned(),
        visible,
        collapsed: !visible,
        detachable: true,
        resizable: true,
        active: visible && !disabled,
        hovered,
        disabled,
    }
}

pub(super) fn dock_slot_label(slot: &str) -> &'static str {
    match slot {
        "left.scene_tree" => "Scene",
        "right.inspector" => "Inspector",
        "bottom.asset_browser" => "Assets",
        "bottom.import_queue" => "Import",
        "bottom.output_log" => "Log",
        "bottom.profiler_diagnostics" => "Diagnostics",
        _ => "Panel",
    }
}

pub(super) fn set_input_capture_contribution(
    resources: &mut Resources,
    owner: &str,
    capture: UiInputCaptureState,
) {
    let mut manager = resources
        .remove::<UiInputCaptureStateManager>()
        .unwrap_or_default();
    manager.add_capture(owner.to_owned(), capture);
    let resolved = manager.resolve_final_capture();
    resources.insert(manager);
    resources.insert(resolved);
}

pub(super) fn remove_input_capture_contribution(
    resources: &mut Resources,
    owner: &str,
    refresh_surface: Option<&str>,
) {
    let mut manager = resources
        .remove::<UiInputCaptureStateManager>()
        .unwrap_or_default();
    manager.remove_capture(owner);
    let mut resolved = manager.resolve_final_capture();
    if let Some(surface) = refresh_surface {
        resolved.draw_refresh_requested = true;
        if !resolved.surfaces.iter().any(|it| it == surface) {
            resolved.surfaces.push(surface.to_owned());
        }
    }
    resources.insert(manager);
    resources.insert(resolved);
}

pub(super) fn component_id_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
}

pub(super) fn asset_document_field_detail(
    field: &newengine_assets_api::AssetDocumentField,
) -> String {
    let Some(property) = field.schema_property.as_ref() else {
        return field.value_kind.clone();
    };
    let edit_policy = if property.editable {
        "editable"
    } else {
        "readonly"
    };
    let pointer = if property.json_pointer.trim().is_empty() {
        field.source_pointer.as_str()
    } else {
        property.json_pointer.as_str()
    };
    format!(
        "{} · kind={} · {}",
        edit_policy,
        property.value_kind.as_str(),
        if pointer.is_empty() {
            "schema pointer pending"
        } else {
            pointer
        }
    )
}

pub(super) fn asset_document_value_label(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => {
            let mut out = v.chars().take(48).collect::<String>();
            if v.chars().count() > 48 {
                out.push('…');
            }
            out
        }
        Value::Array(items) => format!("[{} items]", items.len()),
        Value::Object(map) => format!("{{{} keys}}", map.len()),
    }
}

pub(super) fn publish_screen_surface_node(node: &UiSurfaceNode) {
    crate::platform_runtime::ui_gateway_frame::publish_surface_node(node);
}

pub(super) fn publish_screen_node_tree_request(request: &UiNodeTreeRequest) {
    crate::platform_runtime::ui_gateway_frame::publish_node_tree_request(request);
}

pub(super) fn load_screen_profile_config() -> ScreenProfileConfig {
    let raw = newengine_plugin_host::get_plugin_overrides_with_env("engine.runtime");
    let value = raw
        .get("ui")
        .and_then(|ui| ui.get("screen_profile").or_else(|| ui.get("screen")))
        .or_else(|| raw.get("screen_profile"));

    let Some(value) = value else {
        return ScreenProfileConfig::default();
    };

    match parse_config_value(value) {
        Ok(config) => config,
        Err(err) => {
            newengine_ulog_api::ulog::warn!(
                "screen profile: invalid engine.runtime.ui.screen_profile config; using Editor profile err='{}' raw={}",
                err,
                compact_json(value),
            );
            ScreenProfileConfig::default()
        }
    }
}

pub(super) fn parse_config_value(value: &Value) -> Result<ScreenProfileConfig, String> {
    match value {
        Value::String(profile) => UiScreenProfile::parse(profile)
            .map(|profile| ScreenProfileConfig {
                profile,
                ..ScreenProfileConfig::default()
            })
            .ok_or_else(|| format!("unknown screen profile '{profile}'")),
        Value::Object(_) => {
            serde_json::from_value::<ScreenProfileConfig>(value.clone()).map_err(|e| e.to_string())
        }
        other => Err(format!("expected string or object, got {other}")),
    }
}

pub(super) fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unprintable-json>".to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditorScreen {
    pub(super) descriptor: UiScreenProfileDescriptor,
}
