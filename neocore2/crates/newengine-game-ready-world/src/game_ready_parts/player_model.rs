use super::*;

use newengine_model_domain_api::ModelSkinBinding;

#[path = "player_model_animation.rs"]
mod animation;
#[path = "player_model_assets.rs"]
mod assets;
#[path = "player_model_binding.rs"]
mod binding;
#[path = "player_model_validation.rs"]
mod validation;

pub(crate) use animation::tick_player_skin_animation;
pub(crate) use binding::{spawn_game_ready_player_model, tick_player_model_assignments};

#[derive(Clone, Debug)]
pub(super) struct PlayerRuntimeModelPart {
    primitive_id: PrimitiveId,
    material_id: MaterialId,
    material_slot: String,
    color: [f32; 4],
    skin: Option<ModelSkinBinding>,
}
