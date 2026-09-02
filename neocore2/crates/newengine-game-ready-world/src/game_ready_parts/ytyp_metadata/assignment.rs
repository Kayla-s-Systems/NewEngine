use super::*;

pub(in crate::ytyp_metadata) fn character_model_assignment_from_ytyp_metadata(
    metadata: &serde_json::Value,
    definition_ref: &str,
) -> Option<newengine_engine_runtime::gameplay::PlayerModelAssignment> {
    let player = value_path(metadata, &["player"])?;
    let source = value_path(player, &["model"])
        .or_else(|| value_path(player, &["source"]))
        .and_then(value_string)?;
    if source.trim().is_empty() {
        return None;
    }

    let mut assignment = newengine_engine_runtime::gameplay::PlayerModelAssignment::new(source);
    assignment.properties_ref = Some(definition_ref.trim().replace('\\', "/"));
    assignment.texture_dictionary = value_path(player, &["texture_dictionary"])
        .and_then(value_string)
        .filter(|value| !value.trim().is_empty());
    assignment.skeleton_source = value_path(player, &["metadata"])
        .or_else(|| value_path(player, &["skeleton"]))
        .and_then(value_string)
        .filter(|value| !value.trim().is_empty());
    assignment.target_height = value_path(player, &["target_height"])
        .and_then(value_f32)
        .filter(|value| value.is_finite())
        .unwrap_or(assignment.target_height)
        .clamp(0.25, 3.0);
    assignment.eye_height_ratio = value_path(player, &["eye_height_ratio"])
        .and_then(value_f32)
        .filter(|value| value.is_finite())
        .unwrap_or(assignment.eye_height_ratio)
        .clamp(0.55, 0.98);
    assignment.yaw_offset = value_path(player, &["yaw_offset"])
        .and_then(value_f32)
        .filter(|value| value.is_finite())
        .unwrap_or(0.0);
    assignment.hide_in_first_person = false;

    let mut slots = std::collections::BTreeMap::<String, String>::new();
    for (attribute, semantic) in [
        ("idle_animation", "locomotion.idle"),
        ("walk_animation", "locomotion.walk"),
        ("run_animation", "locomotion.run"),
        ("sprint_animation", "locomotion.sprint"),
        ("crouch_idle_animation", "locomotion.crouch_idle"),
        ("crouch_walk_animation", "locomotion.crouch_walk"),
        ("jump_animation", "locomotion.jump"),
        ("fall_animation", "locomotion.fall"),
        ("unarmed_ready_animation", "unarmed.ready"),
        ("unarmed_attack_animation", "unarmed.attack"),
        ("turn_45_left_animation", "turn.left.45"),
        ("turn_45_right_animation", "turn.right.45"),
        ("turn_90_left_animation", "turn.left.90"),
        ("turn_90_right_animation", "turn.right.90"),
        ("turn_135_left_animation", "turn.left.135"),
        ("turn_135_right_animation", "turn.right.135"),
        ("turn_180_left_animation", "turn.left.180"),
        ("turn_180_right_animation", "turn.right.180"),
    ] {
        if let Some(reference) = value_path(player, &[attribute])
            .and_then(value_string)
            .filter(|value| !value.trim().is_empty())
        {
            slots.insert(semantic.to_owned(), reference);
        }
    }
    assignment.animation_slots = slots;
    assignment.idle_animation = assignment
        .animation_for_slot("locomotion.idle")
        .map(str::to_owned);
    assignment.walk_animation = assignment
        .animation_for_slot("locomotion.walk")
        .map(str::to_owned);
    assignment.run_animation = assignment
        .animation_for_slot("locomotion.run")
        .map(str::to_owned);
    assignment.sprint_animation = assignment
        .animation_for_slot("locomotion.sprint")
        .map(str::to_owned);
    assignment.crouch_idle_animation = assignment
        .animation_for_slot("locomotion.crouch_idle")
        .map(str::to_owned);
    assignment.crouch_walk_animation = assignment
        .animation_for_slot("locomotion.crouch_walk")
        .map(str::to_owned);
    assignment.jump_animation = assignment
        .animation_for_slot("locomotion.jump")
        .map(str::to_owned);
    assignment.fall_animation = assignment
        .animation_for_slot("locomotion.fall")
        .map(str::to_owned);

    if let Some(value) = value_path(player, &["helper_pose_copies"]) {
        assignment.presentation.helper_pose_copies =
            player_joint_copy_rules(value).unwrap_or_default();
    }
    if let Some(value) = value_path(player, &["animation_event_bindings"]) {
        assignment.presentation.animation_event_bindings =
            animation_event_bindings(value).unwrap_or_default();
    }
    assignment.presentation.turn_45_left_animation = assignment
        .animation_for_slot("turn.left.45")
        .map(str::to_owned);
    assignment.presentation.turn_45_right_animation = assignment
        .animation_for_slot("turn.right.45")
        .map(str::to_owned);
    assignment.presentation.turn_90_left_animation = assignment
        .animation_for_slot("turn.left.90")
        .map(str::to_owned);
    assignment.presentation.turn_90_right_animation = assignment
        .animation_for_slot("turn.right.90")
        .map(str::to_owned);
    assignment.presentation.turn_135_left_animation = assignment
        .animation_for_slot("turn.left.135")
        .map(str::to_owned);
    assignment.presentation.turn_135_right_animation = assignment
        .animation_for_slot("turn.right.135")
        .map(str::to_owned);
    assignment.presentation.turn_180_left_animation = assignment
        .animation_for_slot("turn.left.180")
        .map(str::to_owned);
    assignment.presentation.turn_180_right_animation = assignment
        .animation_for_slot("turn.right.180")
        .map(str::to_owned);
    assignment.presentation.unarmed_ready_animation = assignment
        .animation_for_slot("unarmed.ready")
        .map(str::to_owned);
    assignment.presentation.unarmed_attack_animation = assignment
        .animation_for_slot("unarmed.attack")
        .map(str::to_owned);

    assignment
        .animation_for_slot("locomotion.idle")
        .is_some()
        .then_some(assignment)
}
