use super::super::*;
use super::selection::is_preview_image_node;

impl AssetInspectorRuntimeModule {
    pub(in crate::runtime) fn handle_preview_camera_input(
        &mut self,
        dispatch: &UiEventDispatchFrame,
        input: Option<&UiInputFrame>,
    ) {
        let scene_3d = self
            .preview_snapshot
            .as_ref()
            .is_some_and(|preview| preview.kind == AssetPreviewKind::Scene3d);
        let provider_captured = scene_3d
            && dispatch.capture_state.active
            && dispatch.capture_state.owner_surface_id == ASSET_INSPECTOR_SURFACE_ID
            && is_preview_image_node(&dispatch.capture_state.owner_node_id);
        let hovered = scene_3d
            && dispatch
                .hovered_node
                .as_ref()
                .is_some_and(|hit| is_preview_image_node(&hit.node_id));

        let Some(input) = input else {
            self.preview_middle_pan_active = false;
            self.update_preview_camera_capture_state(provider_captured, false);
            return;
        };

        let middle_down = input.is_mouse_down(PREVIEW_PAN_MOUSE_BUTTON);
        let middle_released = input.is_mouse_released(PREVIEW_PAN_MOUSE_BUTTON);
        if scene_3d
            && !self.preview_middle_pan_active
            && middle_down
            && (hovered || provider_captured)
        {
            self.preview_middle_pan_active = true;
        }
        if self.preview_middle_pan_active && (!scene_3d || middle_released || !middle_down) {
            self.preview_middle_pan_active = false;
        }

        self.update_preview_camera_capture_state(provider_captured, self.preview_middle_pan_active);

        if self.preview_middle_pan_active && input.mouse_delta != (0.0, 0.0) {
            let _ = self
                .preview_api
                .pan_camera(input.mouse_delta.0, input.mouse_delta.1);
        } else if provider_captured && input.mouse_delta != (0.0, 0.0) {
            let _ = self
                .preview_api
                .orbit_camera(input.mouse_delta.0, input.mouse_delta.1);
        }

        if hovered && input.mouse_wheel.1.abs() > f32::EPSILON {
            if let Some(view) = self.preview_api.zoom_camera(input.mouse_wheel.1) {
                self.status = format!(
                    "3D preview | zoom distance {:.2} | LMB orbit | MMB pan",
                    view.distance
                );
                self.dirty = true;
            }
        }
    }

    fn update_preview_camera_capture_state(
        &mut self,
        provider_captured: bool,
        middle_pan_active: bool,
    ) {
        let active = provider_captured || middle_pan_active;
        let status = if middle_pan_active {
            "3D preview | MMB camera pan active | wheel zoom"
        } else if provider_captured {
            "3D preview | mouse captured | LMB drag to orbit"
        } else {
            "3D preview | LMB orbit | MMB pan | wheel zoom"
        };
        if active != self.preview_pointer_captured
            || status != self.status
                && (provider_captured || middle_pan_active || self.preview_pointer_captured)
        {
            self.preview_pointer_captured = active;
            self.status = status.to_owned();
            self.dirty = true;
        }
    }
}
