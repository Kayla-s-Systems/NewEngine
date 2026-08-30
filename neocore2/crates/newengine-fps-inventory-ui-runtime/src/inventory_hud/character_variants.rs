use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    PlayerBraidSecondaryMotionRig, PlayerCharacterPresentation, PlayerEyeParentFollowRule,
    PlayerJointChannels, PlayerJointCopyRule, PlayerJointRotationWeight, PlayerModelAssignment,
    PlayerModelBinding, PlayerPaletteFollowRule, PlayerWeaponArmIkRigDefinition,
};
use newengine_gameplay_fps_api::{
    FpsCharacterMenuPolicySnapshot, FpsGameplayPolicySnapshot, FpsPlayableCharacterPolicy,
};

pub const CHARACTER_SELECT_ACTION_PREFIX: &str = "game.character.select.";

pub use newengine_fps_character_runtime::PlayableCharacterSelection;

#[inline]
pub fn playable_character_variants(world: &World) -> &[FpsPlayableCharacterPolicy] {
    if let Some(project) = world.resource::<FpsGameplayPolicySnapshot>() {
        if !project.characters.is_empty() {
            return project.characters.as_slice();
        }
    }
    world
        .resource::<FpsCharacterMenuPolicySnapshot>()
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

fn assignment_payload_matches(
    current: &PlayerModelAssignment,
    desired: &PlayerModelAssignment,
) -> bool {
    let mut current = current.clone();
    let mut desired = desired.clone();
    current.revision = 0;
    desired.revision = 0;
    current == desired
}

/// The game-ready scene may bootstrap the default avatar before the FPS Lua policy is installed.
/// Reconcile that provisional assignment with the project-authored character descriptor once the
/// policy becomes available, even when the YDD path itself did not change. Presentation data
/// (native equipment clips, rig policies, offsets) is part of the assignment contract and must
/// therefore advance the assignment revision.
pub(crate) fn reconcile_existing_player_assignments_with_policy(
    world: &mut World,
    policy: &FpsGameplayPolicySnapshot,
) -> usize {
    let players = world
        .query::<PlayerModelAssignment>()
        .map(|(entity, assignment)| (entity, assignment.clone()))
        .collect::<Vec<_>>();
    let mut reconciled = 0usize;

    for (player, current) in players {
        let selected_id = world
            .get::<PlayableCharacterSelection>(player)
            .map(|selection| selection.variant_id.as_str());
        let variant = selected_id
            .and_then(|id| {
                policy.characters.iter().find(|variant| {
                    variant.id.eq_ignore_ascii_case(id)
                        || variant
                            .aliases
                            .iter()
                            .any(|alias| alias.eq_ignore_ascii_case(id))
                })
            })
            .or_else(|| {
                policy.characters.iter().find(|variant| {
                    variant.runtime_ready
                        && variant.runtime_model_ref.as_deref().is_some_and(|source| {
                            source.trim().eq_ignore_ascii_case(current.source.trim())
                        })
                })
            });
        let Some(variant) = variant else {
            continue;
        };
        let Some(desired) = assignment(variant) else {
            continue;
        };
        if assignment_payload_matches(&current, &desired) {
            continue;
        }
        match newengine_engine_runtime::gameplay::set_player_model_assignment(
            world, player, desired,
        ) {
            Ok(revision) => {
                reconciled += 1;
                newengine_ulog_api::ulog::info!(
                    "fps character assignment reconciled after policy install player={} variant='{}' revision={} source='{}' ready_clip='{}' aim_clip='{}'",
                    player.stable_u64(),
                    variant.id,
                    revision,
                    variant.runtime_model_ref.as_deref().unwrap_or(""),
                    variant
                        .presentation
                        .equipment_ready_animation
                        .as_deref()
                        .unwrap_or("none"),
                    variant
                        .presentation
                        .equipment_aim_animation
                        .as_deref()
                        .unwrap_or("none"),
                );
            }
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "fps character assignment reconciliation failed player={} variant='{}': {}",
                    player.stable_u64(),
                    variant.id,
                    error,
                );
            }
        }
    }
    reconciled
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
            detached_head_follow_rule: variant.presentation.detached_head_follow_rule.as_ref().map(
                |rule| PlayerPaletteFollowRule {
                    driver_joint: rule.driver_joint.clone(),
                    follower_roots: rule.follower_roots.clone(),
                    include_descendants: rule.include_descendants,
                },
            ),
            eye_parent_follow: variant.presentation.eye_parent_follow,
            eye_parent_follow_rule: variant.presentation.eye_parent_follow_rule.as_ref().map(
                |rule| PlayerEyeParentFollowRule {
                    left_joint: rule.left_joint.clone(),
                    right_joint: rule.right_joint.clone(),
                    parent_joint: rule.parent_joint.clone(),
                    preserve_bind_local: rule.preserve_bind_local,
                },
            ),
            helper_pose_copies: variant
                .presentation
                .helper_pose_copies
                .iter()
                .map(|rule| PlayerJointCopyRule {
                    source_joint: rule.source_joint.clone(),
                    target_joint: rule.target_joint.clone(),
                    channels: PlayerJointChannels {
                        translation: rule.channels.translation,
                        rotation: rule.channels.rotation,
                        scale: rule.channels.scale,
                    },
                })
                .collect(),
            braid_secondary_motion: variant.presentation.braid_secondary_motion.as_ref().map(
                |rig| PlayerBraidSecondaryMotionRig {
                    chain_joints: rig.chain_joints.clone(),
                    head_joint: rig.head_joint.clone(),
                    head_base_joint: rig.head_base_joint.clone(),
                    upper_back_joint: rig.upper_back_joint.clone(),
                    middle_back_joint: rig.middle_back_joint.clone(),
                    lower_back_joint: rig.lower_back_joint.clone(),
                    left_shoulder_joint: rig.left_shoulder_joint.clone(),
                    right_shoulder_joint: rig.right_shoulder_joint.clone(),
                },
            ),
            equipment_ready_animation: variant.presentation.equipment_ready_animation.clone(),
            equipment_aim_animation: variant.presentation.equipment_aim_animation.clone(),
            equipment_reload_animation: variant.presentation.equipment_reload_animation.clone(),
            unarmed_ready_animation: variant.presentation.unarmed_ready_animation.clone(),
            unarmed_attack_animation: variant.presentation.unarmed_attack_animation.clone(),
            turn_45_left_animation: variant.presentation.turn_45_left_animation.clone(),
            turn_45_right_animation: variant.presentation.turn_45_right_animation.clone(),
            turn_90_left_animation: variant.presentation.turn_90_left_animation.clone(),
            turn_90_right_animation: variant.presentation.turn_90_right_animation.clone(),
            turn_135_left_animation: variant.presentation.turn_135_left_animation.clone(),
            turn_135_right_animation: variant.presentation.turn_135_right_animation.clone(),
            turn_180_left_animation: variant.presentation.turn_180_left_animation.clone(),
            turn_180_right_animation: variant.presentation.turn_180_right_animation.clone(),
            noclip_animation: variant.presentation.noclip_animation.clone(),
            fall_low_animation: variant.presentation.fall_low_animation.clone(),
            fall_medium_animation: variant.presentation.fall_medium_animation.clone(),
            fall_high_animation: variant.presentation.fall_high_animation.clone(),
            landing_soft_animation: variant.presentation.landing_soft_animation.clone(),
            landing_medium_animation: variant.presentation.landing_medium_animation.clone(),
            landing_hard_animation: variant.presentation.landing_hard_animation.clone(),
            landing_hard_run_animation: variant.presentation.landing_hard_run_animation.clone(),
            fall_medium_min_distance: variant.presentation.fall_medium_min_distance,
            fall_high_min_distance: variant.presentation.fall_high_min_distance,
            equipment_ready_sample_phase: variant.presentation.equipment_ready_sample_phase,
            equipment_ready_rotation_weights: variant
                .presentation
                .equipment_ready_rotation_weights
                .iter()
                .map(|item| PlayerJointRotationWeight {
                    joint: item.joint.clone(),
                    weight: item.weight,
                    channels: PlayerJointChannels {
                        translation: item.channels.translation,
                        rotation: item.channels.rotation,
                        scale: item.channels.scale,
                    },
                })
                .collect(),
            equipment_aim_rotation_weights: variant
                .presentation
                .equipment_aim_rotation_weights
                .iter()
                .map(|item| PlayerJointRotationWeight {
                    joint: item.joint.clone(),
                    weight: item.weight,
                    channels: PlayerJointChannels {
                        translation: item.channels.translation,
                        rotation: item.channels.rotation,
                        scale: item.channels.scale,
                    },
                })
                .collect(),
            equipment_reload_rotation_weights: variant
                .presentation
                .equipment_reload_rotation_weights
                .iter()
                .map(|item| PlayerJointRotationWeight {
                    joint: item.joint.clone(),
                    weight: item.weight,
                    channels: PlayerJointChannels {
                        translation: item.channels.translation,
                        rotation: item.channels.rotation,
                        scale: item.channels.scale,
                    },
                })
                .collect(),
            equipment_arm_ik: variant.presentation.equipment_arm_ik,
            equipment_arm_ik_rig: variant
                .presentation
                .equipment_arm_ik_rig
                .as_ref()
                .map(|rig| PlayerWeaponArmIkRigDefinition {
                    chest: rig.chest.clone(),
                    right_shoulder: rig.right_shoulder.clone(),
                    right_elbow: rig.right_elbow.clone(),
                    right_wrist: rig.right_wrist.clone(),
                    right_palm: rig.right_palm.clone(),
                    right_prop_attachment: rig.right_prop_attachment.clone(),
                    left_shoulder: rig.left_shoulder.clone(),
                    left_elbow: rig.left_elbow.clone(),
                    left_wrist: rig.left_wrist.clone(),
                    left_palm: rig.left_palm.clone(),
                    left_prop_attachment: rig.left_prop_attachment.clone(),
                }),
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
    fn policy_reconciliation_revisions_same_model_when_presentation_changes() {
        let mut world = World::new();
        let player = newengine_engine_runtime::gameplay::spawn_default_player(
            &mut world,
            None,
            "policy-reconcile-test",
            newengine_math::Vec3::ZERO,
        );
        let mut variant = character(
            "ellie",
            "ellie_latest",
            "models/characters/ellie/ellie.ydd@ellie",
        );
        variant.presentation.equipment_ready_animation = Some(
            "animations/characters/ellie/movement-combat.ycd@ellie-idle-fb-hr-aim-guns-rifle-ref"
                .to_owned(),
        );
        variant.presentation.noclip_animation = Some(
            "animations/characters/ellie/DA_Sit_CrossLegged.ycd@DA_Sit_CrossLegged".to_owned(),
        );
        let current = PlayerModelAssignment {
            revision: 4,
            enabled: true,
            source: "models/characters/ellie/ellie.ydd@ellie".to_owned(),
            target_height: variant.target_height,
            hide_in_first_person: variant.hide_in_first_person,
            ..PlayerModelAssignment::default()
        };
        let _ = world.insert(player, current);
        let mut policy = FpsGameplayPolicySnapshot::default();
        policy.characters = vec![variant];

        assert_eq!(
            reconcile_existing_player_assignments_with_policy(&mut world, &policy),
            1
        );
        let assignment = world
            .get::<PlayerModelAssignment>(player)
            .expect("assignment");
        assert_eq!(assignment.revision, 5);
        assert!(assignment
            .presentation
            .equipment_ready_animation
            .as_deref()
            .is_some_and(|value| value.contains("ellie-idle-fb-hr-aim-guns-rifle-ref")));
        assert_eq!(
            assignment.presentation.noclip_animation.as_deref(),
            Some("animations/characters/ellie/DA_Sit_CrossLegged.ycd@DA_Sit_CrossLegged")
        );
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

    #[test]
    fn shared_menu_catalog_fills_empty_project_character_catalog() {
        let mut world = World::new();
        world.insert_resource(FpsGameplayPolicySnapshot::default());
        world.insert_resource(FpsCharacterMenuPolicySnapshot {
            characters: vec![
                character(
                    "shared.ellie",
                    "ellie",
                    "models/characters/ellie/ellie.ydd@ellie",
                ),
                character(
                    "shared.abby",
                    "abby",
                    "models/characters/abby/abby.ydd@abby",
                ),
            ],
            ..FpsCharacterMenuPolicySnapshot::default()
        });
        assert_eq!(playable_character_variants(&world).len(), 2);
        assert_eq!(variant_by_id(&world, "abby").unwrap().id, "shared.abby");
    }
}
