#![forbid(unsafe_op_in_unsafe_fn)]

pub mod draw;
pub mod texture;

pub mod provider;
pub mod providers;

pub mod markup;

pub use provider::{
    UiBuildFn, UiFrameDesc, UiFrameOutput, UiProvider, UiProviderKind, UiProviderOptions,
};
pub use providers::create_provider;

pub use markup::{UiMarkupDoc, UiState};
