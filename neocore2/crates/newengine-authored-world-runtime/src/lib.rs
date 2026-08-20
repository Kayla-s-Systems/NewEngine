#![forbid(unsafe_op_in_unsafe_fn)]

mod bootstrap;
mod loader;
mod materialize;
mod module;

pub use bootstrap::AuthoredMapSceneBootstrapProvider;
pub use module::AuthoredWorldBootstrapModule;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthoredWorldBootstrapStats {
    pub cells: usize,
    pub placements: usize,
    pub model_actors: usize,
    pub definition_markers: usize,
}
