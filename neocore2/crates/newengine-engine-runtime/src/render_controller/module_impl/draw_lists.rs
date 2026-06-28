#![forbid(unsafe_op_in_unsafe_fn)]

#[path = "draw_lists_parts/extraction.rs"]
mod extraction;
#[path = "draw_lists_parts/registry.rs"]
mod registry;

pub(crate) use self::extraction::{DrawListBuildCtx, RuntimeDrawListSet};
pub(crate) use self::registry::{
    ExternalRenderDrawListProviderDesc, RenderDrawListProviderRegistry,
};
