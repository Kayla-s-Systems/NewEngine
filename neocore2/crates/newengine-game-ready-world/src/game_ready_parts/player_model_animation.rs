use super::*;

use newengine_animation_runtime::{
    apply_animation_intent_to_graph_instance, global_animation_clip_store,
    global_compiled_animation_graph_store, AnimationClip, AnimationClipBinding,
    AnimationClipReference, AnimationEventCursor, AnimationEventOccurrence,
    AnimationGraphEvaluation, AnimationGraphInstance, AnimationSkeletonRuntime,
    CompiledAnimationGraph, JointLocalPose,
};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient, ASSET_LIST_FILE_BODY_OUTPUT};
use newengine_math::{Mat4, Quat, Vec3};
use newengine_model_skeleton_api::ModelSkeletonMetadata;

// Player-model animation stays a flat implementation namespace; responsibilities live in focused fragments.
include!("player_model_animation/braid.rs");
include!("player_model_animation/binding.rs");
include!("player_model_animation/locomotion_graph.rs");
include!("player_model_animation/weapon_ik.rs");
include!("player_model_animation/eyes.rs");
include!("player_model_animation/detached_head.rs");
include!("player_model_animation/clip_loading.rs");
include!("player_model_animation/weapon_frames.rs");
include!("player_model_animation/runtime_tick.rs");
include!("player_model_animation/tests.rs");
