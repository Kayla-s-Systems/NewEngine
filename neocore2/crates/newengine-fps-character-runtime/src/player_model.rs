use super::*;

use newengine_model_domain_api::ModelSkinBinding;

#[path = "player_model_animation.rs"]
mod animation;
#[path = "player_model_assets.rs"]
mod assets;
#[path = "player_model_binding.rs"]
mod binding;
#[path = "player_model_sidecar.rs"]
mod sidecar;
#[path = "player_model_validation.rs"]
mod validation;

pub(crate) use animation::{
    player_has_authored_equipment_pose, player_left_hand_weapon_frame,
    player_resolved_weapon_ready_root, player_rifle_ready_body_frames,
    player_rifle_view_forward_model, player_right_hand_prop_frame,
    publish_player_first_person_camera_anchors, tick_player_skin_animation,
};
pub(crate) use binding::tick_player_model_grounding;
pub use binding::{spawn_authored_player_model, tick_player_model_assignments};
pub(crate) use sidecar::tick_player_skin_sidecars;

#[derive(Clone, Debug)]
pub(super) struct PlayerRuntimeModelPart {
    source_mesh_name: String,
    primitive_id: PrimitiveId,
    first_person_primitive_id: Option<PrimitiveId>,
    material_id: MaterialId,
    material_slot: String,
    color: [f32; 4],
    skin: Option<ModelSkinBinding>,
}
