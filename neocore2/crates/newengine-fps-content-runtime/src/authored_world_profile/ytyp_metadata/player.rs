include!("player/secondary_motion.rs");

use super::*;

#[path = "player/parsing.rs"]
mod parsing;
use parsing::{animation_event_bindings, authored_joint_list, player_joint_copy_rules};
pub use parsing::{
    equipment_animation_slot_from_attribute, equipment_ready_sample_phase_family_from_attribute,
    player_joint_rotation_weights,
};

include!("player/hydration_identity.rs");
include!("player/hydration_animation.rs");
include!("player/hydration_runtime.rs");
include!("player/hydration_view.rs");

pub fn apply_player_model_from_ytyp(
    profile: &mut AuthoredWorldProfile,
    metadata: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let player_node = value_path(metadata, &["player"]);
    let Some(model) = player_node
        .and_then(|player| player.get("model"))
        .filter(|model| model.is_object())
        .or_else(|| {
            player_node.filter(|player| {
                player.get("source").is_some()
                    || player.get("texture_dictionary").is_some()
                    || player.get("metadata").is_some()
                    || player
                        .get("model")
                        .and_then(|value| value.as_str())
                        .is_some()
            })
        })
        .or_else(|| value_path(metadata, &["model"]))
        .or_else(|| player_node.and_then(|player| player.get("model")))
    else {
        return 0;
    };
    let mut applied = 0usize;
    applied += apply_player_model_identity_and_rig(profile, model, definition_ref);
    applied += apply_player_animation_metadata(profile, model);
    applied += apply_player_runtime_tuning(profile, player_node.unwrap_or(model));
    applied += apply_player_model_view_metadata(profile, model);
    if applied > 0 {
        newengine_ulog_api::ulog::info!(
            "fps-content ytyp metadata: player model descriptor source='{}' properties_ref={:?} policy='.ytyp connects model source to material bindings'",
            profile.player.model.source,
            profile.player.model.properties_ref
        );
    }
    applied
}

#[path = "assignment.rs"]
mod assignment;
pub use assignment::character_model_assignment_from_ytyp_metadata;
