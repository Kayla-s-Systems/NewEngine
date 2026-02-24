#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{AssetAccess, UiImageLoader};

/// Built-in UI icon catalog.
///
/// This is a *data contract* between UI code and assets. UI code depends on stable logical keys,
/// while the actual files are sourced via AssetManager.
///
/// Paths are relative to the AssetManager virtual root (e.g. `assets/ui/icons/...`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinUiIcon {
    AppLogo,
    Refresh,
    Load,
    Reset,
    Console,

    Enable,
    Disable,
    Close,

    Play,
    Stop,

    GizmoTranslate,
    GizmoRotate,
    GizmoScale,

    LightDirectional,
    LightPoint,
}

impl BuiltinUiIcon {
    /// Stable icon key used for `$tex.<key>` variables.
    #[inline]
    pub const fn key(self) -> &'static str {
        match self {
            Self::AppLogo => "app_logo",
            Self::Refresh => "refresh",
            Self::Load => "load",
            Self::Reset => "reset",
            Self::Console => "console",
            Self::Enable => "enable",
            Self::Disable => "disable",
            Self::Close => "close",
            Self::Play => "play",
            Self::Stop => "stop",
            Self::GizmoTranslate => "gizmo_translate",
            Self::GizmoRotate => "gizmo_rotate",
            Self::GizmoScale => "gizmo_scale",
            Self::LightDirectional => "light_dir",
            Self::LightPoint => "light_point",
        }
    }

    /// Default asset path for the icon.
    #[inline]
    pub const fn default_path(self) -> &'static str {
        match self {
            Self::AppLogo => "ui/icons/app_logo.png",
            Self::Refresh => "ui/icons/refresh.png",
            Self::Load => "ui/icons/load.png",
            Self::Reset => "ui/icons/reset.png",
            Self::Console => "ui/icons/console.png",
            Self::Enable => "ui/icons/enable.svg",
            Self::Disable => "ui/icons/disable.svg",
            Self::Close => "ui/icons/close.svg",
            Self::Play => "ui/icons/play.svg",
            Self::Stop => "ui/icons/stop.svg",
            Self::GizmoTranslate => "ui/icons/gizmo_translate.png",
            Self::GizmoRotate => "ui/icons/gizmo_rotate.png",
            Self::GizmoScale => "ui/icons/gizmo_scale.png",
            Self::LightDirectional => "ui/icons/sun.png",
            Self::LightPoint => "ui/icons/light.png",
        }
    }
}

/// Default editor-facing icon set.
///
/// This is intentionally conservative (small) to keep startup cheap.
pub const EDITOR_DEFAULT_ICONS: &[BuiltinUiIcon] = &[
    BuiltinUiIcon::AppLogo,
    BuiltinUiIcon::Refresh,
    BuiltinUiIcon::Load,
    BuiltinUiIcon::Reset,
    BuiltinUiIcon::Console,
    BuiltinUiIcon::GizmoTranslate,
    BuiltinUiIcon::GizmoRotate,
    BuiltinUiIcon::GizmoScale,
    BuiltinUiIcon::LightDirectional,
    BuiltinUiIcon::LightPoint,
];

/// Registers built-in icon paths in the image loader.
#[inline]
pub fn request_builtin_icons(
    loader: &mut UiImageLoader,
    assets: &dyn AssetAccess,
    icons: &[BuiltinUiIcon],
) {
    for icon in icons {
        loader.request(assets, icon.key(), icon.default_path());
    }
}

/// Registers a single built-in icon.
#[inline]
pub fn request_builtin_icon(
    loader: &mut UiImageLoader,
    assets: &dyn AssetAccess,
    icon: BuiltinUiIcon,
) {
    loader.request(assets, icon.key(), icon.default_path());
}

#[cfg(feature = "egui")]
mod egui_ext {
    use super::*;
    use egui_winit::egui;

    /// Returns egui `TextureId::User(u64)` when the icon texture is ready.
    #[inline]
    pub fn tex_id(loader: &UiImageLoader, icon: BuiltinUiIcon) -> Option<egui::TextureId> {
        let id = loader.tex_id_u64(icon.key())?;
        Some(egui::TextureId::User(id))
    }

    /// A standardized icon+label button.
    ///
    /// - If the icon is not ready, falls back to a text-only button.
    /// - Keeps sizing consistent across the editor.
    pub fn icon_button(
        ui: &mut egui::Ui,
        tid: Option<egui::TextureId>,
        label: &str,
    ) -> egui::Response {
        let min = egui::vec2(0.0, 28.0);

        match tid {
            Some(tid) => {
                let st = egui::load::SizedTexture::new(tid, egui::vec2(16.0, 16.0));
                ui.add(egui::Button::image_and_text(st, label).min_size(min))
            }
            None => ui.add(egui::Button::new(label).min_size(min)),
        }
    }
}

#[cfg(feature = "egui")]
pub use egui_ext::{icon_button, tex_id};
