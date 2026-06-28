use super::*;

/// Profile-owned UI projection over `engine.assets`.
///
/// This module does not register a service and does not extend the backend API.
/// If `engine.ui` is unavailable, it only emits a warning and skips drawing.
pub struct AssetsCatalogUiRuntimeModule {
    pub(crate) state: AssetsCatalogRuntimeState,
    pub(crate) open: bool,
    pub(crate) current_path: String,
    pub(crate) selected_index: usize,
    pub(crate) last_refresh_frame: u64,
    pub(crate) last_toggle_frame: u64,
    pub(crate) last_published_open: bool,
    pub(crate) last_published_visible: bool,
    pub(crate) last_pointer_frame: u64,
    pub(crate) input_registered: bool,
    pub(crate) cached_snapshot: Option<AssetsCatalogSnapshot>,
    pub(crate) cached_node: Option<UiSurfaceNode>,
    pub(crate) view_mode: CatalogViewMode,
    pub(crate) search_query: String,
    pub(crate) collapsed_paths: BTreeSet<String>,
    pub(crate) hovered_entry_index: Option<usize>,
    pub(crate) focus_scope: CatalogFocusScope,
    pub(crate) cached_document_actions: Vec<AssetDocumentAction>,
    pub(crate) cached_document_action_ref: String,
    pub(crate) cached_document_action_error: Option<String>,
    pub(crate) last_action_result: Option<AssetPatchResult>,
    pub(crate) context_menu_open: bool,
    pub(crate) main_scrollbar_dragging: bool,
}

impl Default for AssetsCatalogUiRuntimeModule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Send + 'static> Module<E> for AssetsCatalogUiRuntimeModule {
    fn id(&self) -> &'static str {
        "app.asset_browser.ui_node"
    }

    fn startup_requires(&self) -> &'static [EngineReadinessKey] {
        const REQUIRES: &[EngineReadinessKey] = &[EngineReadinessKey::EnginePluginsReady];
        REQUIRES
    }

    fn start(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        self.input_registered = ensure_assets_catalog_input_registration();
        if !self.input_registered {
            newengine_ulog_api::ulog::warn!(
                "asset browser UI: semantic input listener registration incomplete; will continue through engine.input snapshot but F1 may be unavailable"
            );
        }
        Ok(())
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let frame_index = ctx.frame().map(|frame| frame.frame_index).unwrap_or(0);

        let input = ctx
            .resources()
            .get::<UiInputFrame>()
            .cloned()
            .unwrap_or_default();
        let actions = resolve_actions(&input);
        let surface_size_px = ctx
            .resources()
            .get::<WindowInitSize>()
            .map(|size| [size.width.max(1), size.height.max(1)])
            .unwrap_or(DEFAULT_SURFACE_SIZE_PX);
        let toggled = action_frame_contains(&actions, engine_action::ASSET_CATALOG_UI_TOGGLE);
        let editor_profile_active = is_editor_screen_profile(ctx.resources());

        if toggled && self.last_toggle_frame != frame_index {
            self.last_toggle_frame = frame_index;
            if editor_profile_active {
                newengine_ulog_api::ulog::info!(
                    "asset browser UI: toggle consumed by editor dock surface; profile='editor' visible=true modal=false"
                );
            } else {
                self.open = !self.open;
                self.cached_node = None;
                if self.open && self.cached_snapshot.is_none() {
                    self.current_path.clear();
                    self.selected_index = 0;
                }
                newengine_ulog_api::ulog::info!(
                    "asset browser UI: visibility changed open={}",
                    self.open
                );
            }
        }

        let docked_browser_visible = ctx
            .resources()
            .get::<UiDockLayoutState>()
            .map(|layout| layout.panel_visible("bottom.content_browser"))
            .unwrap_or(true);
        let visible = (editor_profile_active && docked_browser_visible) || self.open;
        if visible {
            let stale = frame_index.saturating_sub(self.last_refresh_frame) >= 30;
            if stale || self.cached_node.is_none() || self.last_toggle_frame == frame_index {
                self.refresh_cache(frame_index, surface_size_px);
            }
            let dispatch_frame = ctx.resources().get::<UiEventDispatchFrame>().cloned();
            self.handle_text_input(&input);
            self.handle_ui_dispatch_frame(
                dispatch_frame.as_ref(),
                &input,
                surface_size_px,
                frame_index,
            );
            self.handle_navigation_input(&actions, frame_index, surface_size_px);
            if self.cached_node.is_none() {
                self.refresh_cache(frame_index, surface_size_px);
            }
            self.publish_selected_asset_context(ctx.resources_mut());
            if let Some(node) = self.cached_node.clone() {
                self.publish_surface(node);
            }
            if editor_profile_active {
                // In Editor profile the Content Browser is a docked editor panel.
                // The global screen profile capture already gates gameplay input;
                // the browser must not become a second modal owner and fight the editor shell.
                remove_input_capture_contribution(
                    ctx.resources_mut(),
                    ASSETS_CATALOG_UI_OWNER,
                    None,
                );
            } else {
                set_input_capture_contribution(
                    ctx.resources_mut(),
                    ASSETS_CATALOG_UI_OWNER,
                    UiInputCaptureState::modal(
                        ASSETS_CATALOG_SURFACE_ID,
                        "asset browser UI modal capture",
                    ),
                );
            }
        } else if self.last_published_visible || self.last_toggle_frame == frame_index {
            self.publish_surface(UiSurfaceNode::hidden(
                ASSETS_CATALOG_SURFACE_ID,
                ASSETS_CATALOG_UI_OWNER,
            ));
            remove_input_capture_contribution(
                ctx.resources_mut(),
                ASSETS_CATALOG_UI_OWNER,
                Some(ASSETS_CATALOG_SURFACE_ID),
            );
        } else {
            remove_input_capture_contribution(ctx.resources_mut(), ASSETS_CATALOG_UI_OWNER, None);
        }

        self.last_published_open = self.open;
        self.last_published_visible = visible;
        Ok(())
    }
}
