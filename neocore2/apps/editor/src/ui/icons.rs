#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_assets::AssetServiceClient;
use newengine_core::plugins::default_host_api;
use newengine_platform_winit::egui;
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
        ui_icons::tex_id(&self.loader, icon)
    }

    #[inline]
    pub fn icon_button(&self, ui: &mut egui::Ui, icon: BuiltinUiIcon, label: &str) -> egui::Response {
        ui_icons::icon_button(ui, self.tex_id(icon), label)
    }

    /// Pumps icon loading and uploads newly ready textures into egui.
    pub fn pump(&mut self, ctx: &egui::Context) {
        self.loader.pump(ctx, &self.assets);
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
