#![forbid(unsafe_op_in_unsafe_fn)]

//! Standard profile-owned render feature pack.
//!
//! This crate is not a renderer backend and does not depend on a concrete
//! runtime controller. It implements provider traits from
//! `newengine-render-feature-api`; the active profile composes these providers
//! into whatever runtime owns the render feature registries.

mod constants;
mod draw;
mod lighting;
mod pack;

pub use constants::*;
pub use pack::StandardRenderFeaturePack;

pub const STANDARD_RENDER_FEATURE_RUNTIME_UNIT_SPEC: newengine_service_api::EngineRuntimeUnitSpec =
    newengine_service_api::EngineRuntimeUnitSpec::new(
        "newengine.render-feature.standard",
        1,
        newengine_service_api::EngineRuntimeUnitKind::Provider,
        &[newengine_service_api::runtime_unit_capability::RENDER_FEATURE],
        &["render.backend"],
        &[
            "engine.runtime-unit",
            "render-feature",
            "standard",
            "first-party",
        ],
    );

#[cfg(test)]
mod tests;

#[cfg(test)]
mod runtime_unit_tests {
    use super::*;

    #[test]
    fn runtime_unit_advertises_render_feature_capability() {
        assert!(STANDARD_RENDER_FEATURE_RUNTIME_UNIT_SPEC
            .provides
            .contains(&newengine_service_api::runtime_unit_capability::RENDER_FEATURE));
    }
}
