use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    PlayerCharacterPresentation, PlayerJointRotationWeight, PlayerModelAssignment,
    PlayerModelBinding,
};
use newengine_gameplay_fps_api::{FpsGameplayPolicySnapshot, FpsPlayableCharacterPolicy};

pub const CHARACTER_SELECT_ACTION_PREFIX: &str = "game.character.select.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayableCharacterSelection {
    pub variant_id: String,
}

#[inline]
pub fn playable_character_variants(world: &World) -> &[FpsPlayableCharacterPolicy] {
    world
        .resource::<FpsGameplayPolicySnapshot>()
        .map(|policy| policy.characters.as_slice())
        .unwrap_or(&[])
}

pub fn variant_by_id<'a>(world: &'a World, id: &str) -> Option<&'a FpsPlayableCharacterPolicy> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    playable_character_variants(world).iter().find(|variant| {
        variant.id.eq_ignore_ascii_case(id)
            || variant
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(id))
    })
}

pub fn variant_from_action<'a>(
    world: &'a World,
    action_id: &str,
) -> Option<&'a FpsPlayableCharacterPolicy> {
    let token = action_id.strip_prefix(CHARACTER_SELECT_ACTION_PREFIX)?;
    variant_by_id(world, token)
}

pub fn selected_variant<'a>(
    world: &'a World,
    player: EntityId,
) -> Option<&'a FpsPlayableCharacterPolicy> {
    if let Some(selection) = world.get::<PlayableCharacterSelection>(player) {
        if let Some(variant) = variant_by_id(world, &selection.variant_id) {
            return Some(variant);
        }
    }
    let binding = world.get::<PlayerModelBinding>(player)?;
    playable_character_variants(world).iter().find(|variant| {
        variant.runtime_ready
            && variant
                .runtime_model_ref
                .as_deref()
                .is_some_and(|source| source.eq_ignore_ascii_case(binding.source.trim()))
    })
}

pub fn assignment(variant: &FpsPlayableCharacterPolicy) -> Option<PlayerModelAssignment> {
    if !variant.runtime_ready {
        return None;
    }
    Some(PlayerModelAssignment {
        enabled: true,
        source: variant.runtime_model_ref.as_deref()?.to_owned(),
        properties_ref: variant.properties_ref.clone(),
        texture_dictionary: variant.texture_dictionary.clone(),
        skeleton_source: variant.skeleton_ref.clone(),
        idle_animation: variant.animations.idle.clone(),
        walk_animation: variant.animations.walk.clone(),
        run_animation: variant.animations.run.clone(),
        sprint_animation: variant.animations.sprint.clone(),
        crouch_idle_animation: variant.animations.crouch_idle.clone(),
        crouch_walk_animation: variant.animations.crouch_walk.clone(),
        jump_animation: variant.animations.jump.clone(),
        fall_animation: variant.animations.fall.clone(),
        presentation: PlayerCharacterPresentation {
            detached_head_follow: variant.presentation.detached_head_follow,
            eye_parent_follow: variant.presentation.eye_parent_follow,
            equipment_ready_animation: variant.presentation.equipment_ready_animation.clone(),
            equipment_aim_animation: variant.presentation.equipment_aim_animation.clone(),
            equipment_reload_animation: variant.presentation.equipment_reload_animation.clone(),
            equipment_ready_sample_phase: variant.presentation.equipment_ready_sample_phase,
            equipment_ready_rotation_weights: variant
                .presentation
                .equipment_ready_rotation_weights
                .iter()
                .map(|item| PlayerJointRotationWeight {
                    joint: item.joint.clone(),
                    weight: item.weight,
                })
                .collect(),
            equipment_aim_rotation_weights: variant
                .presentation
                .equipment_aim_rotation_weights
                .iter()
                .map(|item| PlayerJointRotationWeight {
                    joint: item.joint.clone(),
                    weight: item.weight,
                })
                .collect(),
            equipment_reload_rotation_weights: variant
                .presentation
                .equipment_reload_rotation_weights
                .iter()
                .map(|item| PlayerJointRotationWeight {
                    joint: item.joint.clone(),
                    weight: item.weight,
                })
                .collect(),
            equipment_arm_ik: variant.presentation.equipment_arm_ik,
        },
        target_height: variant.target_height,
        yaw_offset: variant.yaw_offset,
        hide_in_first_person: variant.hide_in_first_person,
        ..PlayerModelAssignment::default()
    })
}

#[inline]
pub fn availability_label(variant: &FpsPlayableCharacterPolicy) -> &'static str {
    if variant.runtime_ready {
        "Runtime ready"
    } else {
        "Authoring source"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_gameplay_fps_api::FpsPlayableCharacterAnimations;

    fn character(id: &str, alias: &str, model: &str) -> FpsPlayableCharacterPolicy {
        FpsPlayableCharacterPolicy {
            id: id.to_owned(),
            family: "test-family".to_owned(),
            display_name: id.to_owned(),
            aliases: vec![alias.to_owned()],
            runtime_ready: true,
            runtime_model_ref: Some(model.to_owned()),
            animations: FpsPlayableCharacterAnimations::default(),
            target_height: 1.75,
            hide_in_first_person: true,
            ..FpsPlayableCharacterPolicy::default()
        }
    }

    fn world_with_characters() -> World {
        let mut world = World::new();
        let mut policy = FpsGameplayPolicySnapshot::default();
        policy.characters = vec![
            character("test.alpha", "legacy_alpha", "models/test/alpha.ydd@alpha"),
            character("test.beta", "legacy_beta", "models/test/beta.ydd@beta"),
        ];
        world.insert_resource(policy);
        world
    }

    #[test]
    fn character_catalog_comes_from_world_policy() {
        let world = world_with_characters();
        assert_eq!(playable_character_variants(&world).len(), 2);
        assert_eq!(variant_by_id(&world, "test.beta").unwrap().id, "test.beta");
    }

    #[test]
    fn project_authored_aliases_drive_action_compatibility() {
        let world = world_with_characters();
        assert_eq!(
            variant_from_action(&world, "game.character.select.legacy_alpha")
                .unwrap()
                .id,
            "test.alpha"
        );
    }

    #[test]
    fn assignment_contains_only_project_descriptor_values() {
        let world = world_with_characters();
        let variant = variant_by_id(&world, "test.alpha").unwrap();
        let assignment = assignment(variant).unwrap();
        assert_eq!(assignment.source, "models/test/alpha.ydd@alpha");
        assert_eq!(assignment.target_height, 1.75);
    }
}
