//! Built-in materials.
//!
//! Builtins must remain small and deterministic; they are meant to bootstrap the editor
//! and provide predictable defaults.

mod unlit;

use crate::core::MaterialRegistry;

/// Register all built-in materials into the registry.
#[inline]
pub fn register_all(reg: &MaterialRegistry) {
    unlit::register(reg);
}
