#![forbid(unsafe_op_in_unsafe_fn)]

pub mod draw;
pub mod texture;
pub mod hub;
pub mod input;
pub mod provider;
pub mod providers;

pub mod ui_images;
pub mod ui_icons;
pub mod previews;

pub mod markup;

pub use input::UiInputFrame;
pub use provider::{
    UiBuildFn, UiFrameDesc, UiFrameOutput, UiProvider, UiProviderKind, UiProviderOptions,
};
pub use providers::create_provider;

pub use newengine_assets::{AssetAccess, AssetService, AssetServiceClient, AssetState, WaitReadyError};

pub use previews::{UiPreviewDesc, UiPreviewHandle, UiPreviewKind, UiPreviewProvider};
pub use ui_icons::{BuiltinUiIcon, EDITOR_DEFAULT_ICONS};
pub use ui_images::UiImageLoader;

pub use markup::{UiMarkupDoc, UiState};

pub use hub::{UiContributor, UiDynFrame, UiHub, UiLayer, UiOrder};
