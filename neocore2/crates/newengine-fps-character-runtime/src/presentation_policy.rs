use newengine_ecs::World;
use newengine_engine_runtime::gameplay::{
    PlayerCharacterPresentation, PlayerJointRotationWeight, PlayerModelAssignment,
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
            unarmed_ready_animation: variant.presentation.unarmed_ready_animation.clone(),
            unarmed_attack_animation: variant.presentation.unarmed_attack_animation.clone(),
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
