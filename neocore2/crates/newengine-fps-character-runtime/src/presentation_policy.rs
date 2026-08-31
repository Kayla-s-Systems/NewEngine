use newengine_ecs::World;
use newengine_engine_runtime::gameplay::{
    PlayerBraidSecondaryMotionRig, PlayerCharacterPresentation, PlayerEyeParentFollowRule,
    PlayerJointChannels, PlayerJointCopyRule, PlayerJointRotationWeight, PlayerModelAssignment,
    PlayerPaletteFollowRule, PlayerWeaponArmIkRigDefinition,
};
use newengine_gameplay_fps_api::{FpsGameplayPolicySnapshot, FpsPlayableCharacterPolicy};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayableCharacterSelection {
    pub variant_id: String,
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
pub fn reconcile_existing_player_assignments_with_policy(
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

    let animation = |slot: &str, legacy: &Option<String>| {
        variant
            .animations
            .slots
            .get(slot)
            .cloned()
            .or_else(|| legacy.clone())
    };
    let presentation_animation = |slot: &str, legacy: &Option<String>| {
        variant
            .presentation
            .animation_slots
            .get(slot)
            .cloned()
            .or_else(|| legacy.clone())
    };

    Some(PlayerModelAssignment {
        enabled: true,
        source: variant.runtime_model_ref.as_deref()?.to_owned(),
        properties_ref: variant.properties_ref.clone(),
        texture_dictionary: variant.texture_dictionary.clone(),
        skeleton_source: variant.skeleton_ref.clone(),
        animation_slots: variant.animations.slots.clone(),
        idle_animation: animation("idle", &variant.animations.idle),
        walk_animation: animation("walk", &variant.animations.walk),
        run_animation: animation("run", &variant.animations.run),
        sprint_animation: animation("sprint", &variant.animations.sprint),
        crouch_idle_animation: animation("crouch_idle", &variant.animations.crouch_idle),
        crouch_walk_animation: animation("crouch_walk", &variant.animations.crouch_walk),
        jump_animation: animation("jump", &variant.animations.jump),
        fall_animation: animation("fall", &variant.animations.fall),
        presentation: PlayerCharacterPresentation {
            animation_slots: variant.presentation.animation_slots.clone(),
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
            skin_sidecar: None,
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
            equipment_ready_animation: presentation_animation(
                "equipment.ready",
                &variant.presentation.equipment_ready_animation,
            ),
            equipment_aim_animation: presentation_animation(
                "equipment.aim",
                &variant.presentation.equipment_aim_animation,
            ),
            equipment_reload_animation: presentation_animation(
                "equipment.reload",
                &variant.presentation.equipment_reload_animation,
            ),
            unarmed_ready_animation: presentation_animation(
                "unarmed.ready",
                &variant.presentation.unarmed_ready_animation,
            ),
            unarmed_attack_animation: presentation_animation(
                "unarmed.attack",
                &variant.presentation.unarmed_attack_animation,
            ),
            turn_45_left_animation: presentation_animation(
                "turn.left.45",
                &variant.presentation.turn_45_left_animation,
            ),
            turn_45_right_animation: presentation_animation(
                "turn.right.45",
                &variant.presentation.turn_45_right_animation,
            ),
            turn_90_left_animation: presentation_animation(
                "turn.left.90",
                &variant.presentation.turn_90_left_animation,
            ),
            turn_90_right_animation: presentation_animation(
                "turn.right.90",
                &variant.presentation.turn_90_right_animation,
            ),
            turn_135_left_animation: presentation_animation(
                "turn.left.135",
                &variant.presentation.turn_135_left_animation,
            ),
            turn_135_right_animation: presentation_animation(
                "turn.right.135",
                &variant.presentation.turn_135_right_animation,
            ),
            turn_180_left_animation: presentation_animation(
                "turn.left.180",
                &variant.presentation.turn_180_left_animation,
            ),
            turn_180_right_animation: presentation_animation(
                "turn.right.180",
                &variant.presentation.turn_180_right_animation,
            ),
            noclip_animation: presentation_animation(
                "movement.noclip",
                &variant.presentation.noclip_animation,
            ),
            fall_low_animation: presentation_animation(
                "fall.low",
                &variant.presentation.fall_low_animation,
            ),
            fall_medium_animation: presentation_animation(
                "fall.medium",
                &variant.presentation.fall_medium_animation,
            ),
            fall_high_animation: presentation_animation(
                "fall.high",
                &variant.presentation.fall_high_animation,
            ),
            landing_soft_animation: presentation_animation(
                "landing.soft",
                &variant.presentation.landing_soft_animation,
            ),
            landing_medium_animation: presentation_animation(
                "landing.medium",
                &variant.presentation.landing_medium_animation,
            ),
            landing_hard_animation: presentation_animation(
                "landing.hard",
                &variant.presentation.landing_hard_animation,
            ),
            landing_hard_run_animation: presentation_animation(
                "landing.hard_run",
                &variant.presentation.landing_hard_run_animation,
            ),
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
