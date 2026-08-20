#![forbid(unsafe_op_in_unsafe_fn)]

//! Scene-domain lighting components and settings.
//!
//! This crate deliberately contains only renderer-neutral ECS/resource data:
//! light color/intensity, transform-facing parameters and declarative shadow
//! preferences. It is not a lighting backend, does not own GPU buffers, and does
//! not choose tiled/deferred/clustered execution. Native light-list construction
//! belongs to the active `engine.render` provider.

mod lights;
mod shadow;

pub use lights::{AmbientLight, DirectionalLight, PointLight, SpotLight};
pub use shadow::{
    LocalShadowSettings, ShadowFilter, ShadowMethod, ShadowPcssSettings, ShadowSettings,
};
