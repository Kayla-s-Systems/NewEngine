use newengine_ui_api::UiActionDispatch;

use super::super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InspectorScrollTarget {
    Browser,
    PreviewEntries,
}

impl AssetInspectorRuntimeModule {
    pub(in crate::runtime) fn handle_generic_scroll_action(
        &mut self,
        action: &UiActionDispatch,
    ) -> bool {
        let Some(target) = scroll_target(action) else {
            return false;
        };
        if action.action_id == UI_SCROLL_WHEEL_ACTION
            && action.trigger == UiNodeEventTrigger::ValueChanged
        {
            let Some(wheel_y) = payload_array_f32(&action.payload, "wheel", 1) else {
                return false;
            };
            return self.scroll_window_by_wheel(target, wheel_y);
        }
        if action.action_id == UI_SCROLLBAR_DRAG_ACTION {
            if matches!(
                action.trigger,
                UiNodeEventTrigger::Press
                    | UiNodeEventTrigger::DragStart
                    | UiNodeEventTrigger::DragMove
            ) {
                let Some(local_y) = payload_array_f32(&action.payload, "local_pos", 1) else {
                    return false;
                };
                let Some(track_h) = payload_array_f32(&action.payload, "global_rect", 3)
                    .filter(|height| *height > 0.0)
                else {
                    return false;
                };
                return self.scroll_window_to_ratio(target, local_y / track_h);
            }
            return matches!(
                action.trigger,
                UiNodeEventTrigger::Release | UiNodeEventTrigger::DragEnd
            );
        }
        false
    }

    fn scroll_window_by_wheel(&mut self, target: InspectorScrollTarget, wheel_y: f32) -> bool {
        if !wheel_y.is_finite() || wheel_y.abs() <= f32::EPSILON {
            return false;
        }
        let normalized = if wheel_y.abs() > 10.0 {
            wheel_y / 120.0
        } else {
            wheel_y
        };
        let steps = normalized.abs().ceil().max(1.0) as usize;
        match target {
            InspectorScrollTarget::Browser => {
                let max_start = self.entries.len().saturating_sub(ENTRY_ROWS);
                let next = if normalized > 0.0 {
                    self.browser_window_start.saturating_sub(steps)
                } else {
                    self.browser_window_start
                        .saturating_add(steps)
                        .min(max_start)
                };
                if next == self.browser_window_start {
                    return false;
                }
                self.browser_window_start = next;
            }
            InspectorScrollTarget::PreviewEntries => {
                let max_start = self
                    .preview_entries
                    .len()
                    .saturating_sub(PREVIEW_ENTRY_ROWS);
                let next = if normalized > 0.0 {
                    self.preview_entries_window_start.saturating_sub(steps)
                } else {
                    self.preview_entries_window_start
                        .saturating_add(steps)
                        .min(max_start)
                };
                if next == self.preview_entries_window_start {
                    return false;
                }
                self.preview_entries_window_start = next;
            }
        }
        self.dirty = true;
        true
    }

    fn scroll_window_to_ratio(&mut self, target: InspectorScrollTarget, ratio: f32) -> bool {
        let ratio = ratio.clamp(0.0, 1.0);
        match target {
            InspectorScrollTarget::Browser => {
                let max_start = self.entries.len().saturating_sub(ENTRY_ROWS);
                let next = (max_start as f32 * ratio).round() as usize;
                if next == self.browser_window_start {
                    return false;
                }
                self.browser_window_start = next;
            }
            InspectorScrollTarget::PreviewEntries => {
                let max_start = self
                    .preview_entries
                    .len()
                    .saturating_sub(PREVIEW_ENTRY_ROWS);
                let next = (max_start as f32 * ratio).round() as usize;
                if next == self.preview_entries_window_start {
                    return false;
                }
                self.preview_entries_window_start = next;
            }
        }
        self.dirty = true;
        true
    }
}

fn scroll_target(action: &UiActionDispatch) -> Option<InspectorScrollTarget> {
    if action_node_or_path_contains(action, PREVIEW_ENTRIES_SCROLL_NODE_ID) {
        Some(InspectorScrollTarget::PreviewEntries)
    } else if action_node_or_path_contains(action, BROWSER_SCROLL_NODE_ID) {
        Some(InspectorScrollTarget::Browser)
    } else {
        None
    }
}

fn action_node_or_path_contains(action: &UiActionDispatch, node_id: &str) -> bool {
    action.node_id == node_id
        || action.node_id.starts_with(&format!("{node_id}."))
        || action
            .payload
            .get("z_path")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|path| {
                path.iter().any(|value| {
                    value.as_str().is_some_and(|candidate| {
                        candidate == node_id || candidate.starts_with(&format!("{node_id}."))
                    })
                })
            })
}

fn payload_array_f32(payload: &serde_json::Value, key: &str, index: usize) -> Option<f32> {
    let value = payload.get(key)?.as_array()?.get(index)?;
    let value = value.as_f64()? as f32;
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_for(node_id: &str, action_id: &str, trigger: UiNodeEventTrigger) -> UiActionDispatch {
        UiActionDispatch {
            surface_id: ASSET_INSPECTOR_SURFACE_ID.to_owned(),
            node_id: node_id.to_owned(),
            action_id: action_id.to_owned(),
            trigger,
            payload: serde_json::json!({
                "z_path": [ASSET_INSPECTOR_SURFACE_ID, node_id],
                "wheel": [0.0, -2.0],
                "local_pos": [2.0, 75.0],
                "global_rect": [0.0, 0.0, 10.0, 100.0]
            }),
            ..UiActionDispatch::default()
        }
    }

    #[test]
    fn wheel_scrolls_browser_window_without_changing_selection() {
        let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
            newengine_engine_runtime::ViewportBridge::new(),
        )));
        let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
        runtime.entries = (0..30)
            .map(|index| InspectorEntry {
                name: format!("asset-{index}"),
                ..InspectorEntry::default()
            })
            .collect();
        runtime.selected_index = Some(0);
        let action = action_for(
            "asset.inspector.browser.scroll",
            UI_SCROLL_WHEEL_ACTION,
            UiNodeEventTrigger::ValueChanged,
        );
        assert!(runtime.handle_generic_scroll_action(&action));
        assert_eq!(runtime.browser_window_start, 2);
        assert_eq!(runtime.selected_index, Some(0));
    }

    #[test]
    fn scrollbar_drag_maps_to_virtualized_preview_entry_window() {
        let preview_api = Arc::new(AssetPreviewApi::new(Arc::new(
            newengine_engine_runtime::ViewportBridge::new(),
        )));
        let mut runtime = AssetInspectorRuntimeModule::new(preview_api);
        runtime.preview_entries = (0..25)
            .map(|index| InspectorEntry {
                name: format!("entry-{index}"),
                ..InspectorEntry::default()
            })
            .collect();
        let action = action_for(
            "asset.inspector.entries.scroll.scrollbar",
            UI_SCROLLBAR_DRAG_ACTION,
            UiNodeEventTrigger::DragMove,
        );
        assert!(runtime.handle_generic_scroll_action(&action));
        assert_eq!(runtime.preview_entries_window_start, 15);
    }
}
