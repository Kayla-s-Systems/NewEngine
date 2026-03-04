#![forbid(unsafe_op_in_unsafe_fn)]

pub mod draw;
pub mod hub;
pub mod input;
pub mod provider;
pub mod providers;
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

pub use asset::{wait_ready, AssetAccess, AssetService, AssetState, WaitReadyError};

#[cfg(feature = "assets")]
pub use asset::AssetServiceClient;

pub use previews::{UiPreviewDesc, UiPreviewHandle, UiPreviewKind, UiPreviewProvider};
pub use ui_icons::{BuiltinUiIcon, EDITOR_DEFAULT_ICONS};
pub use ui_images::UiImageLoader;

pub use markup::{UiMarkupDoc, UiState};

pub use hub::{UiContributor, UiDynFrame, UiHub, UiLayer, UiOrder};
