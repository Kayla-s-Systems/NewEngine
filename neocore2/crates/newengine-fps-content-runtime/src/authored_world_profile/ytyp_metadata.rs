#[path = "ytyp_metadata/player.rs"]
mod player;
use player::apply_player_model_from_ytyp;
#[cfg(test)]
use player::player_joint_rotation_weights;

include!("ytyp_metadata/camera.rs");

include!("ytyp_metadata/values_materials.rs");
include!("ytyp_metadata/player_runtime.rs");
include!("ytyp_metadata/world_constants.rs");
include!("ytyp_metadata/definition.rs");
include!("ytyp_metadata/audio.rs");
include!("ytyp_metadata/render.rs");
include!("ytyp_metadata/apply.rs");

#[cfg(test)]
#[path = "ytyp_metadata/tests.rs"]
mod tests;
