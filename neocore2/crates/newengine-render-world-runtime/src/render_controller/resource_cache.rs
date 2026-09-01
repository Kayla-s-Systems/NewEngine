#![forbid(unsafe_op_in_unsafe_fn)]

#[path = "resource_cache/lifetimes.rs"]
mod lifetimes;
#[path = "resource_cache/material_textures.rs"]
mod material_textures;
#[path = "resource_cache/per_draw.rs"]
mod per_draw;

pub(in crate::render_controller) use material_textures::MaterialTextureReadyState;
