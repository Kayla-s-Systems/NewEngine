#![forbid(unsafe_op_in_unsafe_fn)]

mod debug_lines;
mod lit;
mod primitives;
mod shader_assets;
mod types;

pub(super) use debug_lines::ensure_debug_line_pipeline;
pub(super) use lit::ensure_lit_pipeline;
pub(super) use types::LIT_UBO_SIZE;
pub(super) use primitives::{ensure_primitive_gpu, upload_primitive_mesh};
pub(super) use types::{DebugLineGpu, LitPipeline, PrimitiveGpu};
