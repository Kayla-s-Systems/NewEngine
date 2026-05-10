#![forbid(unsafe_op_in_unsafe_fn)]

use egui;
use newengine_assets::AssetServiceClient;
use newengine_plugin_host::default_host_api;
use newengine_ui::ui_icons;
use newengine_ui::{BuiltinUiIcon, UiImageLoader, EDITOR_DEFAULT_ICONS};

/// Deterministic, editor-local icon loader.
///
/// Delegates to `newengine_ui::UiImageLoader` to avoid per-app duplicated ad-hoc loaders.
///
/// Design goals:
/// - No background threads.
/// - Non-blocking: polling via `AssetAccess::pump()` and `AssetAccess::state()`.
/// - Stable egui texture handles.
pub struct EditorIconLoader {
    assets: AssetServiceClient,
    loader: UiImageLoader,
}

impl Default for EditorIconLoader {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl EditorIconLoader {
    #[inline]
    pub fn new() -> Self {
        let assets = AssetServiceClient::new(default_host_api());
        let mut loader = UiImageLoader::new();

        // Default editor icon set (shared base implementation).
        ui_icons::request_builtin_icons(&mut loader, &assets, EDITOR_DEFAULT_ICONS);

        Self { assets, loader }
    }

    /// Return egui `TextureId::User(u64)` if the icon is available.
    ///
    /// Note: `u64 == 0` is a valid texture id in egui.
    #[inline]
    pub fn tex_id(&self, icon: BuiltinUiIcon) -> Option<egui::TextureId> {
        let id = self.loader.tex_id_u64(icon.key())?;
        Some(egui::TextureId::User(id))
    }

    #[inline]
    pub fn icon_button(
        &self,
        ui: &mut egui::Ui,
        icon: BuiltinUiIcon,
        label: &str,
    ) -> egui::Response {
        let min = egui::vec2(0.0, 28.0);
        match self.tex_id(icon) {
            Some(tid) => {
                let st = egui::load::SizedTexture::new(tid, egui::vec2(16.0, 16.0));
                ui.add(egui::Button::image_and_text(st, label).min_size(min))
            }
            None => ui.add(egui::Button::new(label).min_size(min)),
        }
    }


    pub fn pump_into_state(
        &mut self,
        ctx: &egui::Context,
        state: &mut newengine_ui::markup::UiState,
    ) {
        self.loader.pump_into_state(ctx, &self.assets, state);
    }

    /// Schedule/override an icon path (data-driven).
    ///
    /// Useful for branding/skins without touching UI code.
    #[inline]
    #[allow(dead_code)]
    pub fn set_icon_path(&mut self, icon: BuiltinUiIcon, path: impl Into<String>) {
        self.loader.request(&self.assets, icon.key(), path);
    }
}
