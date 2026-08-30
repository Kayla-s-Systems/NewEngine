#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime decoding/sampling for canonical NorthStar YCD clip bodies and generic
//! linear-blend-skinning palette construction.

use newengine_math::{Mat4, Quat, Vec3};
use newengine_model_skeleton_api::ModelSkeletonMetadata;

// Animation runtime facade split into clip sampling, decoding, palette construction, and binary helpers.
include!("runtime/common.rs");
include!("runtime/clip.rs");
include!("runtime/events.rs");
include!("runtime/compiled.rs");
include!("runtime/decode.rs");
include!("runtime/store.rs");
include!("runtime/graph.rs");
include!("runtime/graph_compile.rs");
include!("runtime/graph_state.rs");
include!("runtime/graph_asset.rs");
include!("runtime/graph_store.rs");
include!("runtime/intent.rs");
include!("runtime/graph_eval.rs");
include!("runtime/graph_tests.rs");
include!("runtime/palette.rs");
include!("runtime/binary.rs");
include!("runtime/tests.rs");
