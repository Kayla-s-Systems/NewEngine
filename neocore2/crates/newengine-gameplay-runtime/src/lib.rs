#![forbid(unsafe_op_in_unsafe_fn)]

mod config;
mod service;
mod state;

pub use service::register_gameplay_foundation_gateways_best_effort;
pub use state::GameplayFoundationState;

pub(crate) const OWNER: &str = "newengine-gameplay-runtime.engine-core-baseline-provider";
pub(crate) const TAGS_PROVIDER_ROUTE: &str = "engine.tags.foundation";
pub(crate) const TASKS_PROVIDER_ROUTE: &str = "engine.tasks.foundation";
pub(crate) const ANIMATION_PROVIDER_ROUTE: &str = "engine.animation.foundation";
pub(crate) const NAVIGATION_PROVIDER_ROUTE: &str = "engine.navigation.foundation";
pub(crate) const AI_PROVIDER_ROUTE: &str = "engine.ai.foundation";
