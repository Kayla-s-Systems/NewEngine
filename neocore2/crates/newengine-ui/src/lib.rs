#![forbid(unsafe_op_in_unsafe_fn)]

pub mod draw;
pub mod hub;
pub mod input;
pub mod provider;
pub mod providers;
pub mod schema;
pub mod screen_profile;
pub mod surface;
pub mod texture;

pub mod asset;

pub mod previews;
pub mod ui_icons;
pub mod ui_images;

pub mod markup;

pub use input::UiInputFrame;
pub use provider::{
    UiBuildFn, UiFrameDesc, UiFrameOutput, UiProvider, UiProviderKind, UiProviderOptions,
};
pub use providers::create_provider;
pub use schema::{
    UiActionBinding, UiActionBindingRef, UiActionDeclaration, UiActionRoute, UiAnchor,
    UiDataBinding, UiDataSourceBinding, UiDeclarativeLayout, UiLayoutBoxSpec, UiLayoutDeclaration,
    UiNodeSpec, UiProviderCatalog, UiSurfaceDeclaration, UiThemeDeclaration, UI_ACTION_CLOSE_MODAL,
    UI_ACTION_OPEN_LOGS, UI_ACTION_QUIT, UI_ACTION_RESUME_GAME, UI_ACTION_RETRY_STARTUP,
    UI_ACTION_START_GAME, UI_ACTION_TOGGLE_DEBUG_OVERLAY, UI_ACTION_TOGGLE_PRIMARY_UI,
    UI_SURFACE_DEBUG_OVERLAY, UI_SURFACE_GAME_HUD, UI_SURFACE_MAIN_MENU, UI_SURFACE_PRIMARY,
};
pub use screen_profile::{
    editing_overlay_descriptor, game_screen_descriptor, headless_screen_descriptor,
    screen_profile_descriptor, EditorScreen, GameScreen,
};

pub use surface::{
    UiAnimationSpec, UiErrorModalSpec, UiLoadingShellSpec, UiProviderBinding, UiProviderManifest,
    UiShellSpec, UiSubsystemCardPaletteSpec, UiSubsystemCardSpec, UiSurfaceProjection,
    UI_ERROR_MODAL_KSYSTEMS_ID, UI_FEATURE_ENGINE_UI_ONLY_STARTUP,
    UI_FEATURE_EXTERNAL_PLUGIN_PROVIDER, UI_FEATURE_KSYSTEMS_ERROR_MODAL, UI_PROVIDER_NONE_ID,
    UI_SHELL_KSYSTEMS_LOADING_ID, UI_STYLE_KSYSTEMS_INDUSTRIAL, UI_SURFACE_ENGINE_ERROR_MODAL,
    UI_SURFACE_ENGINE_LOADING, UI_SURFACE_RUNTIME_OVERLAY, UI_THEME_DARK_GOLD_MAGENTA,
};

pub use asset::{wait_ready, AssetAccess, AssetService, AssetState, WaitReadyError};

#[cfg(feature = "assets")]
pub use asset::AssetServiceClient;

pub use previews::{UiPreviewDesc, UiPreviewHandle, UiPreviewKind, UiPreviewProvider};
pub use ui_icons::{BuiltinUiIcon, EDITOR_DEFAULT_ICONS};
pub use ui_images::UiImageLoader;

pub use markup::{UiMarkupDoc, UiState};

pub use hub::{UiContributor, UiDynFrame, UiHub, UiLayer, UiOrder};
