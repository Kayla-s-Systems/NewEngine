use super::*;

impl ScreenProfileRuntimeState {
    pub(super) fn prepare_toast_surface(
        &mut self,
        resources: &Resources,
        frame_index: u64,
        profile_changed: bool,
    ) -> bool {
        let Some(stack) = resources.get::<UiToastStack>() else {
            self.last_toast_surface_version = None;
            return self.hide_profile_surface(UI_SURFACE_SYSTEM_NOTIFICATIONS, profile_changed);
        };
        if self.descriptor.profile == UiScreenProfile::Headless || stack.notifications.is_empty() {
            self.last_toast_surface_version = None;
            return self.hide_profile_surface(UI_SURFACE_SYSTEM_NOTIFICATIONS, profile_changed);
        }

        let [screen_w, screen_h] = resources
            .get::<WindowInitSize>()
            .map(|size| [size.width.max(1), size.height.max(1)])
            .unwrap_or(DEFAULT_EDITOR_SURFACE_SIZE_PX);
        let extent = [screen_w, screen_h];
        if !profile_changed
            && self.last_toast_surface_version == Some(stack.version)
            && self.last_toast_surface_extent == extent
            && self
                .published_surfaces
                .contains(UI_SURFACE_SYSTEM_NOTIFICATIONS)
        {
            return false;
        }

        let layout = editor_layout_metrics(resources, &self.hidden_panels);
        let toast_w = 360.0_f32.min((layout.screen_w * 0.34).max(260.0));
        let toast_count = stack.notifications.len().min(4);
        let row_h = 52.0;
        let gap = 8.0;
        let total_h =
            toast_count as f32 * row_h + toast_count.saturating_sub(1) as f32 * gap + 16.0;
        let top_margin = if self.descriptor.profile == UiScreenProfile::Editor {
            layout.menu_h + layout.toolbar_h + 12.0
        } else {
            16.0
        };

        let theme_id = if self.descriptor.profile == UiScreenProfile::Editor {
            UI_THEME_NORTHSTAR_EDITOR
        } else {
            UI_THEME_NORTHSTAR_DEFAULT
        };
        let mut style = UiSurfaceStyle {
            anchor: UiSurfaceAnchor::TopRight,
            min_size_px: [toast_w + 16.0, total_h],
            max_size_px: [toast_w + 16.0, total_h],
            margin_px: [12.0, top_margin],
            padding_px: [8.0, 8.0, 8.0, 8.0],
            panel_rgba: [0, 0, 0, 0],
            panel_header_rgba: [0, 0, 0, 0],
            border_rgba: [0, 0, 0, 0],
            border_px: 0.0,
            backdrop_rgba: [0, 0, 0, 0],
            shadow_alpha: 0,
            ..UiSurfaceStyle::default()
        };
        style.theme_id = theme_id.to_owned();

        let mut node =
            UiSurfaceNode::new(UI_SURFACE_SYSTEM_NOTIFICATIONS, ENGINE_UI_NOTIFY_SERVICE_ID)
                .with_theme(theme_id)
                .with_style(style);
        node.z_order = 50_000;
        node.component_id = UI_COMPONENT_PANEL.to_owned();
        node.style_tags = vec![
            "system".to_owned(),
            "toast-stack".to_owned(),
            "notification".to_owned(),
        ];
        node.metrics
            .insert("frame_index".to_owned(), serde_json::json!(frame_index));
        self.append_toast_components(resources, &mut node, &layout);

        publish_screen_node_tree_request(&UiNodeTreeRequest::from_surface_node(
            &node,
            UiNodeRequestSourceKind::Generated,
        ));
        self.published_surfaces
            .insert(UI_SURFACE_SYSTEM_NOTIFICATIONS.to_owned());
        self.last_toast_surface_version = Some(stack.version);
        self.last_toast_surface_extent = extent;
        true
    }
}
