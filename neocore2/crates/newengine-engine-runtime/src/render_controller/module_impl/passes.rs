#![forbid(unsafe_op_in_unsafe_fn)]

pub(super) mod mesh_visibility {
    include!("passes_parts/mesh_visibility.rs");
}

include!("passes_parts/mesh_passes.rs");
