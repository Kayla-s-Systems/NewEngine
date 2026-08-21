use super::*;

use newengine_animation_runtime::{
    build_skin_palette, build_skin_palette_from_local_pose, decode_ycd_body, AnimationClip,
    JointLocalPose,
};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient, ASSET_LIST_FILE_BODY_OUTPUT};
use newengine_math::{Mat4, Quat, Vec3};
use newengine_model_skeleton_api::ModelSkeletonMetadata;

#[derive(Clone, Debug)]
struct PlayerAnimationRuntimeClip {
    clip_ref: String,
    clip: AnimationClip,
}

#[derive(Clone, Debug)]
pub(super) struct PlayerAnimationRuntimeBinding {
    clips: [Option<PlayerAnimationRuntimeClip>; 8],
    active_state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    active_slot: usize,
    skeleton: ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    time_seconds: f32,
    /// Pose currently visible on the character. This is preserved when a new
    /// locomotion state interrupts an in-flight cross-fade.
    current_locals: Vec<JointLocalPose>,
    sampled_target_locals: Vec<JointLocalPose>,
    transition_from_locals: Vec<JointLocalPose>,
    palette_scratch: Vec<Mat4>,
}

#[inline]
const fn locomotion_slot(
    state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
) -> usize {
    use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;
    match state {
        L::Idle => 0,
        L::Walk => 1,
        L::Run => 2,
        L::Sprint => 3,
        L::CrouchIdle => 4,
        L::CrouchWalk => 5,
        L::Jump => 6,
        L::Fall => 7,
    }
}

impl PlayerAnimationRuntimeBinding {
    pub(super) fn initial_palette(&self) -> Vec<Mat4> {
        self.palette_scratch.clone()
    }

    pub(super) fn clip_refs_csv(&self) -> String {
        self.clips
            .iter()
            .filter_map(|clip| clip.as_ref().map(|clip| clip.clip_ref.as_str()))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn resolve_slot(
        &self,
        state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    ) -> usize {
        use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;
        let candidates: &[usize] = match state {
            L::Idle => &[0],
            L::Walk => &[1, 0],
            L::Run => &[2, 1, 0],
            L::Sprint => &[3, 2, 1, 0],
            L::CrouchIdle => &[4, 0],
            L::CrouchWalk => &[5, 1, 0],
            L::Jump => &[6, 2, 0],
            L::Fall => &[7, 6, 2, 0],
        };
        candidates
            .iter()
            .copied()
            .find(|slot| self.clips[*slot].is_some())
            .unwrap_or(0)
    }
}

fn blend_local_poses(
    from: &[JointLocalPose],
    to: &[JointLocalPose],
    alpha: f32,
    out: &mut Vec<JointLocalPose>,
) -> Result<(), String> {
    if from.len() != to.len() {
        return Err(format!(
            "animation transition pose count mismatch from={} to={}",
            from.len(),
            to.len()
        ));
    }
    let alpha = if alpha.is_finite() {
        alpha.clamp(0.0, 1.0)
    } else {
        1.0
    };
    out.clear();
    out.reserve(to.len());
    for (a, b) in from.iter().zip(to.iter()) {
        let translation = Vec3::new(a.translation[0], a.translation[1], a.translation[2]).lerp(
            Vec3::new(b.translation[0], b.translation[1], b.translation[2]),
            alpha,
        );
        let qa = Quat::from_xyzw(a.rotation[0], a.rotation[1], a.rotation[2], a.rotation[3])
            .normalize_or_identity();
        let mut qb = Quat::from_xyzw(b.rotation[0], b.rotation[1], b.rotation[2], b.rotation[3])
            .normalize_or_identity();
        if qa.dot(qb) < 0.0 {
            qb = Quat::from_xyzw(-qb.x, -qb.y, -qb.z, -qb.w);
        }
        let q = qa.slerp(qb, alpha).normalize_or_identity();
        out.push(JointLocalPose {
            translation: [translation.x, translation.y, translation.z],
            rotation: [q.x, q.y, q.z, q.w],
        });
    }
    Ok(())
}

fn split_animation_ref(reference: &str) -> Result<(String, Option<String>), String> {
    let normalized = reference.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err("empty animation reference".to_owned());
    }
    let (path, selector) = normalized
        .rsplit_once('@')
        .map(|(path, selector)| {
            let selector = selector.trim();
            (
                path.to_owned(),
                (!selector.is_empty()).then(|| selector.to_owned()),
            )
        })
        .unwrap_or_else(|| (normalized.clone(), None));
    if !path.to_ascii_lowercase().ends_with(".ycd") {
        return Err(format!(
            "player animation must reference .ycd asset: '{reference}'"
        ));
    }
    Ok((path, selector))
}

fn load_animation_clip(reference: &str) -> Result<AnimationClip, String> {
    let (path, selector) = split_animation_ref(reference)?;
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let payload = assets
        .decode_v1(&AssetDecodeRequest {
            logical_path: path.clone(),
            output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
            selector: serde_json::Value::Null,
        })
        .map_err(|error| {
            format!(
                "player animation asset decode failed ref='{reference}' path='{path}' err='{error}'"
            )
        })?;
    decode_ycd_body(&payload, selector.as_deref()).map_err(|error| {
        format!("player animation YCD decode failed ref='{reference}' err='{error}'")
    })
}

fn validate_animation_clip(
    clip_ref: &str,
    clip: &AnimationClip,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: &ModelSkeletonMetadata,
) -> Result<(), String> {
    if clip.joint_count() != skeleton.joints.len() {
        return Err(format!(
            "player animation/skeleton joint count mismatch clip={} skeleton={} ref='{}'",
            clip.joint_count(),
            skeleton.joints.len(),
            clip_ref
        ));
    }
    if !clip.skeleton_ref.trim().is_empty()
        && !clip
            .skeleton_ref
            .eq_ignore_ascii_case(assignment.skeleton_source.as_deref().unwrap_or_default())
    {
        return Err(format!(
            "player animation skeleton ref mismatch clip='{}' assignment='{}'",
            clip.skeleton_ref,
            assignment.skeleton_source.as_deref().unwrap_or("<none>")
        ));
    }
    for (index, joint) in skeleton.joints.iter().enumerate() {
        if clip.joint_tags.get(index).copied() != Some(joint.tag) {
            return Err(format!(
                "player animation skeleton tag mismatch ref='{}' index={} clip={:?} skeleton={}",
                clip_ref,
                index,
                clip.joint_tags.get(index),
                joint.tag
            ));
        }
    }
    Ok(())
}

fn load_runtime_animation_clip(
    reference: &str,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: &ModelSkeletonMetadata,
) -> Result<PlayerAnimationRuntimeClip, String> {
    let clip = load_animation_clip(reference)?;
    validate_animation_clip(reference, &clip, assignment, skeleton)?;
    Ok(PlayerAnimationRuntimeClip {
        clip_ref: reference.to_owned(),
        clip,
    })
}

pub(super) fn prepare_player_animation_binding(
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    parts: &[PlayerRuntimeModelPart],
    skeleton: Option<&ModelSkeletonMetadata>,
) -> Result<Option<PlayerAnimationRuntimeBinding>, String> {
    use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;

    let skinned_parts = parts
        .iter()
        .filter_map(|part| part.skin.as_ref())
        .collect::<Vec<_>>();
    if skinned_parts.is_empty() {
        return Ok(None);
    }
    let skeleton = skeleton
        .ok_or_else(|| "skinned player model requires authored skeleton metadata".to_owned())?;
    let source_to_model = skinned_parts[0].source_to_model;
    for (part_index, skin) in skinned_parts.iter().enumerate() {
        if skin.source_to_model != source_to_model {
            return Err(format!(
                "skinned player model source-space transform mismatch part={part_index}"
            ));
        }
    }

    let Some(idle_ref) = assignment.idle_animation.as_deref() else {
        return Ok(None);
    };
    let mut clips: [Option<PlayerAnimationRuntimeClip>; 8] =
        [None, None, None, None, None, None, None, None];
    clips[locomotion_slot(L::Idle)] =
        Some(load_runtime_animation_clip(idle_ref, assignment, skeleton)?);

    for (state, reference) in [
        (L::Walk, assignment.walk_animation.as_deref()),
        (L::Run, assignment.run_animation.as_deref()),
        (L::Sprint, assignment.sprint_animation.as_deref()),
        (L::Jump, assignment.jump_animation.as_deref()),
        (L::Fall, assignment.fall_animation.as_deref()),
    ] {
        if let Some(reference) = reference {
            clips[locomotion_slot(state)] = Some(load_runtime_animation_clip(
                reference, assignment, skeleton,
            )?);
        }
    }

    let idle = clips[locomotion_slot(L::Idle)]
        .as_ref()
        .expect("idle clip was inserted above");
    let mut current_locals = Vec::with_capacity(skeleton.joints.len());
    let mut palette_scratch = Vec::with_capacity(skeleton.joints.len());
    build_skin_palette(
        &idle.clip,
        skeleton,
        source_to_model,
        0.0,
        &mut current_locals,
        &mut palette_scratch,
    )?;
    let sampled_target_locals = current_locals.clone();
    let transition_from_locals = current_locals.clone();

    Ok(Some(PlayerAnimationRuntimeBinding {
        clips,
        active_state: L::Idle,
        active_slot: locomotion_slot(L::Idle),
        skeleton: skeleton.clone(),
        source_to_model,
        time_seconds: 0.0,
        current_locals,
        sampled_target_locals,
        transition_from_locals,
        palette_scratch,
    }))
}
pub(crate) fn tick_player_skin_animation(world: &mut newengine_ecs::World, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let players = world
        .query::<PlayerAnimationRuntimeBinding>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();

    for player in players {
        let animation_state = world
            .get::<newengine_engine_runtime::gameplay::PlayerAnimationState>(player)
            .copied()
            .unwrap_or_default();
        let (palette, clip_ref, active_state) = {
            let Some(binding) = world.get_mut::<PlayerAnimationRuntimeBinding>(player) else {
                continue;
            };
            let desired_slot = binding.resolve_slot(animation_state.locomotion);
            let transitioned = binding.active_state != animation_state.locomotion
                || binding.active_slot != desired_slot;
            if transitioned {
                // Cross-fade from the pose that was actually visible, not merely from
                // the previous clip. This keeps hands/forearms continuous even if the
                // player changes locomotion state again before the prior fade finishes.
                binding
                    .transition_from_locals
                    .clone_from(&binding.current_locals);
                binding.active_state = animation_state.locomotion;
                binding.active_slot = desired_slot;
                binding.time_seconds = 0.0;
            } else {
                let playback_rate = match animation_state.locomotion {
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Walk => {
                        (animation_state.normalized_speed / 0.40).clamp(0.65, 1.45)
                    }
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Run => {
                        (animation_state.normalized_speed / 0.85).clamp(0.75, 1.45)
                    }
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Sprint => {
                        animation_state.normalized_speed.clamp(1.0, 1.65)
                    }
                    _ => 1.0,
                };
                binding.time_seconds += dt * playback_rate;
            }

            let active_slot = binding.active_slot;
            let active_state = binding.active_state;
            let active_clip = binding.clips[active_slot]
                .as_ref()
                .expect("resolved player animation slot must contain a clip");
            let clip_ref = active_clip.clip_ref.clone();
            if let Err(error) = active_clip
                .clip
                .sample_local_pose(binding.time_seconds, &mut binding.sampled_target_locals)
            {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: player animation sample failed player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }

            let alpha = animation_state.transition_alpha.clamp(0.0, 1.0);
            if alpha < 1.0 {
                if let Err(error) = blend_local_poses(
                    &binding.transition_from_locals,
                    &binding.sampled_target_locals,
                    alpha,
                    &mut binding.current_locals,
                ) {
                    newengine_ulog_api::ulog::warn!(
                        "game-ready: player animation transition failed player={} state='{}' clip='{}': {}",
                        player.stable_u64(),
                        active_state.clip_hint(),
                        clip_ref,
                        error
                    );
                    continue;
                }
            } else {
                binding
                    .current_locals
                    .clone_from(&binding.sampled_target_locals);
            }

            if let Err(error) = build_skin_palette_from_local_pose(
                &binding.skeleton,
                binding.source_to_model,
                &binding.current_locals,
                &mut binding.palette_scratch,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: player skin palette update failed player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }
            if let Err(error) = super::validation::validate_player_palette(
                &binding.palette_scratch,
                binding.skeleton.joints.len(),
                &format!("animated clip {clip_ref}"),
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: unstable player skin palette rejected player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }
            (binding.palette_scratch.clone(), clip_ref, active_state)
        };

        if let Some(pose) =
            world.get_mut::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
        {
            pose.palette = palette;
            pose.revision = pose.revision.saturating_add(1).max(1);
        } else {
            let _ = world.insert(
                player,
                newengine_engine_runtime::gameplay::PlayerSkinPose {
                    palette,
                    revision: 1,
                },
            );
        }
        if dt > 0.0
            && world
                .get::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
                .is_some_and(|pose| pose.revision == 2)
        {
            newengine_ulog_api::ulog::info!(
                "game-ready: first animated player palette committed player={} state='{}' clip='{}'",
                player.stable_u64(),
                active_state.clip_hint(),
                clip_ref
            );
        }
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;

    #[test]
    fn local_pose_crossfade_preserves_endpoints_and_shortest_quaternion_path() {
        let from = [JointLocalPose {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }];
        let to = [JointLocalPose {
            translation: [2.0, 4.0, 6.0],
            // Same identity rotation with opposite quaternion sign.
            rotation: [0.0, 0.0, 0.0, -1.0],
        }];
        let mut out = Vec::new();
        blend_local_poses(&from, &to, 0.5, &mut out).expect("blend");
        assert_eq!(out.len(), 1);
        assert!((out[0].translation[0] - 1.0).abs() <= 1.0e-6);
        assert!((out[0].translation[1] - 2.0).abs() <= 1.0e-6);
        assert!((out[0].translation[2] - 3.0).abs() <= 1.0e-6);
        assert!(out[0].rotation[0].abs() <= 1.0e-6);
        assert!(out[0].rotation[1].abs() <= 1.0e-6);
        assert!(out[0].rotation[2].abs() <= 1.0e-6);
        assert!((out[0].rotation[3].abs() - 1.0).abs() <= 1.0e-6);
    }
}
