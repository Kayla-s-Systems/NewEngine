#![forbid(unsafe_op_in_unsafe_fn)]

//! Compatibility-only composition facade.
//!
//! New code should depend on the single-purpose provider crate it actually needs.

pub use newengine_ai_runtime::{
    register_ai_gateway_best_effort, PROVIDER_ROUTE as AI_PROVIDER_ROUTE,
};
pub use newengine_animation_foundation_runtime::{
    register_animation_gateway_best_effort, PROVIDER_ROUTE as ANIMATION_PROVIDER_ROUTE,
};
pub use newengine_navigation_runtime::{
    register_navigation_gateway_best_effort, PROVIDER_ROUTE as NAVIGATION_PROVIDER_ROUTE,
};
pub use newengine_tags_runtime::{
    register_tags_gateway_best_effort, PROVIDER_ROUTE as TAGS_PROVIDER_ROUTE,
};
pub use newengine_tasks_runtime::{
    register_tasks_gateway_best_effort, PROVIDER_ROUTE as TASKS_PROVIDER_ROUTE,
};

/// Legacy composition helper retained only for compatibility. Runtime-unit composition
/// uses the five leaf factories directly so selecting AI does not imply tags/tasks/nav/animation.
#[deprecated(note = "compose single-purpose gameplay provider factories instead")]
pub fn register_gameplay_foundation_gateways_best_effort() -> bool {
    let tags = register_tags_gateway_best_effort();
    let tasks = register_tasks_gateway_best_effort();
    let animation = register_animation_gateway_best_effort();
    let navigation = register_navigation_gateway_best_effort();
    let ai = register_ai_gateway_best_effort();
    tags && tasks && animation && navigation && ai
}
