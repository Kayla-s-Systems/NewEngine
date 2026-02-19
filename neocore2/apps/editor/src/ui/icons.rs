#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_assets::AssetServiceClient;
use newengine_core::plugins::default_host_api;
use newengine_platform_winit::egui;
use newengine_ui::UiImageLoader;

/// Editor-local icon kinds.
///
/// This is intentionally small and stable: UI code should depend on these logical kinds,
/// while the actual paths remain data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorIconKind {
    DirectionalLight,
    PointLight,
}

impl EditorIconKind {
    #[inline]
    pub const fn key(self) -> &'static str {
        match self {
            Self::DirectionalLight => "sun",
            Self::PointLight => "light",
        }
    }

    #[inline]
    pub const fn default_path(self) -> &'static str {
        match self {
            Self::DirectionalLight => "ui/icons/sun.png",
            Self::PointLight => "ui/icons/light.png",
        }
    }
}

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

        // Default editor icon set.
        for kind in [EditorIconKind::DirectionalLight, EditorIconKind::PointLight] {
            loader.request(&assets, kind.key(), kind.default_path());
        }

        Self { assets, loader }
    }

    /// Return egui `TextureId::User(u64)` if the icon is available.
    ///
    /// Note: `u64 == 0` is a valid texture id in egui.
    #[inline]
    pub fn tex_id(&self, kind: EditorIconKind) -> Option<egui::TextureId> {
        let id = self.loader.tex_id_u64(kind.key())?;
        Some(egui::TextureId::User(id))
    }

    /// Pumps icon loading and uploads newly ready textures into egui.
    pub fn pump(&mut self, ctx: &egui::Context) {
        self.loader.pump(ctx, &self.assets);
    }

    /// Schedule/override an icon path (data-driven).
    ///
    /// Useful for branding/skins without touching UI code.
    #[inline]
    pub fn set_icon_path(&mut self, kind: EditorIconKind, path: impl Into<String>) {
        self.loader.request(&self.assets, kind.key(), path);
    }
}
