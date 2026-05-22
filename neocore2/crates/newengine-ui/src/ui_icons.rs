#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{AssetAccess, UiImageLoader};

/// Built-in UI icon catalog.
///
/// This is a *data contract* between UI code and assets. UI code depends on stable logical keys,
/// while the actual files are sourced via AssetManager.
///
/// Paths are semantic `.ytd@entry` selectors resolved by `engine.textures`, with
/// AssetManager/VFS used only as byte owner behind that gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinUiIcon {
    AppLogo,

    FileNew,
    FileOpen,
    FileSave,

    AssetManager,

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

            Self::FileNew => "file_new",
            Self::FileOpen => "file_open",
            Self::FileSave => "file_save",

            Self::AssetManager => "asset_manager",

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
            Self::AppLogo => "ui/icons/builtin_icons.ytd@app_logo",

            Self::FileNew => "ui/icons/builtin_icons.ytd@file_new",
            Self::FileOpen => "ui/icons/builtin_icons.ytd@file_open",
            Self::FileSave => "ui/icons/builtin_icons.ytd@file_save",

            Self::AssetManager => "ui/icons/builtin_icons.ytd@asset_manager",

            Self::Refresh => "ui/icons/builtin_icons.ytd@refresh",
            Self::Load => "ui/icons/builtin_icons.ytd@load",
            Self::Reset => "ui/icons/builtin_icons.ytd@reset",
            Self::Console => "ui/icons/builtin_icons.ytd@console",
            Self::Enable => "ui/icons/builtin_icons.ytd@enable",
            Self::Disable => "ui/icons/builtin_icons.ytd@disable",
            Self::Close => "ui/icons/builtin_icons.ytd@close",
            Self::Play => "ui/icons/builtin_icons.ytd@play",
            Self::Stop => "ui/icons/builtin_icons.ytd@stop",
            Self::GizmoTranslate => "ui/icons/builtin_icons.ytd@gizmo_translate",
            Self::GizmoRotate => "ui/icons/builtin_icons.ytd@gizmo_rotate",
            Self::GizmoScale => "ui/icons/builtin_icons.ytd@gizmo_scale",
            Self::LightDirectional => "ui/icons/builtin_icons.ytd@sun",
            Self::LightPoint => "ui/icons/builtin_icons.ytd@light",
        }
    }
}

/// Default editor-facing icon set.
///
/// This is intentionally conservative (small) to keep startup cheap.
pub const EDITOR_DEFAULT_ICONS: &[BuiltinUiIcon] = &[
    BuiltinUiIcon::AppLogo,
    BuiltinUiIcon::FileNew,
    BuiltinUiIcon::FileOpen,
    BuiltinUiIcon::FileSave,
    BuiltinUiIcon::AssetManager,

    BuiltinUiIcon::Refresh,
    BuiltinUiIcon::Load,
    BuiltinUiIcon::Reset,
    BuiltinUiIcon::Console,
    BuiltinUiIcon::Enable,
    BuiltinUiIcon::Disable,
    BuiltinUiIcon::Close,

    BuiltinUiIcon::GizmoTranslate,
    BuiltinUiIcon::GizmoRotate,
    BuiltinUiIcon::GizmoScale,
    BuiltinUiIcon::Play,
    BuiltinUiIcon::Stop,

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
