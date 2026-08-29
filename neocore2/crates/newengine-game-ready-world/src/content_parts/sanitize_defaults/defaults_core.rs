use newengine_game_data::default_game_data;

pub(in super::super) fn default_definition_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}
pub(in super::super) fn default_definition_apply_mode() -> String {
    "metadata_only".to_owned()
}
pub(in super::super) fn default_title() -> String {
    default_game_data().world.title.clone()
}
pub(in super::super) fn default_objective() -> String {
    default_game_data().world.objective.clone()
}
pub(in super::super) fn default_player_start() -> [f32; 3] {
    default_game_data().player.spawn
}
pub(in super::super) fn default_player_yaw() -> f32 {
    default_game_data().player.yaw
}
pub(in super::super) fn default_move_speed() -> f32 {
    default_game_data().player.move_speed
}
pub(in super::super) fn default_look_sens() -> f32 {
    default_game_data().player.look_sensitivity
}
pub(in super::super) fn default_player_model_enabled() -> bool {
    default_game_data().player.model.enabled
}
pub(in super::super) fn default_player_model_source() -> String {
    default_game_data().player.model.source.clone()
}
pub(in super::super) fn default_player_model_properties_ref() -> Option<String> {
    None
}
pub(in super::super) fn default_player_texture_dictionary() -> Option<String> {
    None
}
pub(in super::super) fn default_player_skeleton() -> Option<String> {
    None
}
pub(in super::super) fn default_player_model_height() -> f32 {
    default_game_data().player.model.target_height
}
pub(in super::super) fn default_player_model_eye_height_ratio() -> f32 {
    default_game_data().player.model.eye_height_ratio
}
pub(in super::super) fn default_player_model_offset() -> [f32; 3] {
    default_game_data().player.model.local_offset
}
pub(in super::super) fn default_player_model_yaw_offset() -> f32 {
    default_game_data().player.model.yaw_offset
}
pub(in super::super) fn default_player_model_hide_in_first_person() -> bool {
    default_game_data().player.model.hide_in_first_person
}
pub(in super::super) fn default_terrain_enabled() -> bool {
    default_game_data().world.terrain.enabled
}
