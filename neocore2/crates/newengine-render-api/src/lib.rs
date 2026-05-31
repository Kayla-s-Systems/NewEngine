#![forbid(unsafe_op_in_unsafe_fn)]

pub use newengine_ui_draw::{
    UiDrawCmd, UiDrawList, UiRect, UiTexId, UiTexture, UiTextureDelta, UiTexturePatch, UiVertex,
};

pub mod reserved_textures {
    pub use newengine_ui_draw::reserved::*;
}

mod bindings;
mod capabilities;
mod constants;
mod diagnostics;
mod events;
mod effects;
mod frame;
mod feature_registry;
mod material_graph;
mod shader_variants;
mod maturity;
mod ids;
mod pipeline;
mod protocol;
mod provider_bridge;
mod render_graph;
mod residency;
mod resources;
mod shader_cache;
mod shadows;
mod postfx;
mod uploads;

pub use bindings::*;
pub use capabilities::*;
pub use constants::*;
pub use diagnostics::*;
pub use events::*;
pub use effects::*;
pub use frame::*;
pub use feature_registry::*;
pub use material_graph::*;
pub use shader_variants::*;
pub use maturity::*;
pub use ids::*;
pub use pipeline::*;
pub use protocol::*;
pub use provider_bridge::*;
pub use render_graph::*;
pub use residency::*;
pub use resources::*;
pub use shader_cache::*;
pub use shadows::*;
pub use postfx::*;
pub use uploads::*;
