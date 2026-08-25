use super::*;

use newengine_animation_runtime::{
    build_skin_palette_from_local_pose, decode_ycd_body, AnimationClip, JointLocalPose,
};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient, ASSET_LIST_FILE_BODY_OUTPUT};
use newengine_engine_runtime::gameplay::{
    HitscanWeaponTuning, PlayerSkinPose, PlayerWeaponState, WeaponAnimationDefinition,
};
use newengine_model_skeleton_api::ModelSkeletonMetadata;

#[derive(Clone, Debug)]
struct WeaponRuntimeClip {
    reference: String,
    clip: AnimationClip,
}

#[derive(Clone, Debug)]
struct EquippedWeaponAnimationRuntime {
    owner: EntityId,
    skeleton: ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    idle: Option<WeaponRuntimeClip>,
    fire: Option<WeaponRuntimeClip>,
    reload: Option<WeaponRuntimeClip>,
    spawn_pose: Option<WeaponRuntimeClip>,
    sampled_locals: Vec<JointLocalPose>,
    idle_time: f32,
    fire_time: f32,
    last_shot_sequence: u64,
}

fn split_animation_ref(reference: &str) -> Result<(String, Option<String>), String> {
    let reference = reference.trim().replace('\\', "/");
    if reference.is_empty() {
        return Err("weapon animation reference is empty".to_owned());
    }
    let (path, selector) = reference
        .split_once('@')
        .map(|(path, selector)| (path.trim(), Some(selector.trim())))
        .unwrap_or((reference.as_str(), None));
    if path.is_empty() {
        return Err(format!(
            "weapon animation reference has no path ref='{reference}'"
        ));
    }
    let selector = selector
        .filter(|selector| !selector.is_empty())
        .map(ToOwned::to_owned);
    Ok((path.to_owned(), selector))
}

fn normalize_mount_alias(reference: &str) -> String {
    let normalized = reference
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_ascii_lowercase();
    normalized
        .strip_prefix("shared/")
        .unwrap_or(normalized.as_str())
        .to_owned()
}

fn skeleton_refs_compatible(clip_ref: &str, expected: &str) -> bool {
    normalize_mount_alias(clip_ref) == normalize_mount_alias(expected)
}

fn load_weapon_clip(
    reference: Option<&str>,
    expected_skeleton: &str,
    skeleton: &ModelSkeletonMetadata,
) -> Result<Option<WeaponRuntimeClip>, String> {
    let Some(reference) = reference.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let (path, selector) = split_animation_ref(reference)?;
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let body = assets
        .decode_v1(&AssetDecodeRequest {
            logical_path: path,
            output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
            selector: serde_json::Value::Null,
        })
        .map_err(|error| {
            format!("weapon animation asset decode failed ref='{reference}': {error}")
        })?;
    let clip = decode_ycd_body(&body, selector.as_deref()).map_err(|error| {
        format!("weapon animation YCD decode failed ref='{reference}': {error}")
    })?;
    if !clip.skeleton_ref.trim().is_empty()
        && !skeleton_refs_compatible(&clip.skeleton_ref, expected_skeleton)
    {
        return Err(format!(
            "weapon animation skeleton mismatch ref='{reference}' clip='{}' expected='{}'",
            clip.skeleton_ref, expected_skeleton
        ));
    }
    for (clip_index, &tag) in clip.joint_tags.iter().enumerate() {
        if clip.joint_tags[..clip_index].contains(&tag) {
            return Err(format!(
                "weapon animation duplicate joint tag ref='{reference}' tag={tag}"
            ));
        }
        let dense = tag as usize;
        let present = dense < skeleton.joints.len() && skeleton.joints[dense].tag == tag
            || skeleton.joints.iter().any(|joint| joint.tag == tag);
        if !present {
            return Err(format!(
                "weapon animation joint tag absent ref='{reference}' tag={tag} joints={}",
                skeleton.joints.len()
            ));
        }
    }
    Ok(Some(WeaponRuntimeClip {
        reference: reference.to_owned(),
        clip,
    }))
}

fn bind_pose(skeleton: &ModelSkeletonMetadata) -> Vec<JointLocalPose> {
    skeleton
        .joints
        .iter()
        .map(|joint| JointLocalPose {
            translation: joint.position_ls,
            rotation: joint.rotation_ls,
            scale: Some(joint.scale_ls),
        })
        .collect()
}

fn publish_weapon_palette(
    world: &mut newengine_ecs::World,
    root: EntityId,
    skeleton: &ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    locals: &[JointLocalPose],
) -> Result<(), String> {
    let mut palette = Vec::with_capacity(skeleton.joints.len());
    build_skin_palette_from_local_pose(skeleton, source_to_model, locals, &mut palette)?;
    let revision = world
        .get::<PlayerSkinPose>(root)
        .map(|pose| pose.revision.wrapping_add(1).max(1))
        .unwrap_or(1);
    let _ = world.insert(root, PlayerSkinPose { palette, revision });
    Ok(())
}

pub(crate) fn bind_equipped_weapon_animation(
    world: &mut newengine_ecs::World,
    root: EntityId,
    owner: EntityId,
    skeleton: ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    definition: &WeaponAnimationDefinition,
    initial_shot_sequence: u64,
) -> Result<(), String> {
    let expected_skeleton = definition
        .skeleton
        .as_deref()
        .ok_or("skinned weapon animation requires authored skeleton ref")?;
    let idle = load_weapon_clip(definition.idle.as_deref(), expected_skeleton, &skeleton)?;
    let fire = load_weapon_clip(definition.fire.as_deref(), expected_skeleton, &skeleton)?;
    let reload = load_weapon_clip(definition.reload.as_deref(), expected_skeleton, &skeleton)?;
    let spawn_pose = load_weapon_clip(
        definition.spawn_pose.as_deref(),
        expected_skeleton,
        &skeleton,
    )?;
    if idle.is_none() && fire.is_none() && reload.is_none() && spawn_pose.is_none() {
        return Err("skinned weapon has no authored animation clips".to_owned());
    }

    let mut sampled_locals = bind_pose(&skeleton);
    if let Some(initial) = spawn_pose.as_ref().or(idle.as_ref()) {
        initial
            .clip
            .sample_local_pose_for_skeleton(0.0, &skeleton, &mut sampled_locals)?;
    }
    publish_weapon_palette(world, root, &skeleton, source_to_model, &sampled_locals)?;
    let joint_count = skeleton.joints.len();
    let idle_ref = idle
        .as_ref()
        .map(|clip| clip.reference.clone())
        .unwrap_or_else(|| "<none>".to_owned());
    let fire_ref = fire
        .as_ref()
        .map(|clip| clip.reference.clone())
        .unwrap_or_else(|| "<none>".to_owned());
    let reload_ref = reload
        .as_ref()
        .map(|clip| clip.reference.clone())
        .unwrap_or_else(|| "<none>".to_owned());
    let spawn_ref = spawn_pose
        .as_ref()
        .map(|clip| clip.reference.clone())
        .unwrap_or_else(|| "<none>".to_owned());
    let _ = world.insert(
        root,
        EquippedWeaponAnimationRuntime {
            owner,
            skeleton,
            source_to_model,
            idle,
            fire,
            reload,
            spawn_pose,
            sampled_locals,
            idle_time: 0.0,
            fire_time: f32::INFINITY,
            last_shot_sequence: initial_shot_sequence,
        },
    );
    newengine_ulog_api::ulog::info!(
        "game-ready: equipped weapon skeletal animation bound root={} owner={} joints={} idle='{}' fire='{}' reload='{}' spawn='{}'",
        root.stable_u64(),
        owner.stable_u64(),
        joint_count,
        idle_ref,
        fire_ref,
        reload_ref,
        spawn_ref,
    );
    Ok(())
}

pub(crate) fn tick_equipped_weapon_animations(world: &mut newengine_ecs::World, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let roots = world
        .query::<EquippedWeaponAnimationRuntime>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();

    for root in roots {
        let Some(mut runtime) = world.get::<EquippedWeaponAnimationRuntime>(root).cloned() else {
            continue;
        };
        let state = world
            .get::<PlayerWeaponState>(runtime.owner)
            .copied()
            .unwrap_or_default();
        if state.shot_sequence != runtime.last_shot_sequence {
            runtime.last_shot_sequence = state.shot_sequence;
            runtime.fire_time = 0.0;
            if let Some(fire) = runtime.fire.as_ref() {
                newengine_ulog_api::ulog::info!(
                    "game-ready: native weapon fire animation triggered root={} owner={} shot={} clip='{}' duration={:.6}s source='TLOU2 assault-fire'",
                    root.stable_u64(),
                    runtime.owner.stable_u64(),
                    state.shot_sequence,
                    fire.reference,
                    fire.clip.duration_seconds,
                );
            }
        }

        let reload_progress = (state.reload_remaining > 0.0).then(|| {
            let duration = world
                .get::<HitscanWeaponTuning>(runtime.owner)
                .map(|tuning| tuning.sanitized().reload_duration)
                .filter(|duration| *duration > 1.0e-4)
                .unwrap_or(2.0);
            (1.0 - state.reload_remaining / duration).clamp(0.0, 1.0)
        });

        let sampled =
            if let (Some(progress), Some(reload)) = (reload_progress, runtime.reload.as_ref()) {
                runtime.fire_time = f32::INFINITY;
                reload.clip.sample_local_pose_for_skeleton(
                    reload.clip.duration_seconds * progress,
                    &runtime.skeleton,
                    &mut runtime.sampled_locals,
                )
            } else if let Some(fire) = runtime
                .fire
                .as_ref()
                .filter(|fire| runtime.fire_time <= fire.clip.duration_seconds)
            {
                let result = fire.clip.sample_local_pose_for_skeleton(
                    runtime.fire_time.max(0.0),
                    &runtime.skeleton,
                    &mut runtime.sampled_locals,
                );
                runtime.fire_time += dt;
                result
            } else if let Some(idle) = runtime.idle.as_ref() {
                runtime.idle_time += dt;
                idle.clip.sample_local_pose_for_skeleton(
                    runtime.idle_time,
                    &runtime.skeleton,
                    &mut runtime.sampled_locals,
                )
            } else if let Some(spawn) = runtime.spawn_pose.as_ref() {
                spawn.clip.sample_local_pose_for_skeleton(
                    spawn.clip.duration_seconds,
                    &runtime.skeleton,
                    &mut runtime.sampled_locals,
                )
            } else {
                Ok(())
            };

        if let Err(error) = sampled.and_then(|_| {
            publish_weapon_palette(
                world,
                root,
                &runtime.skeleton,
                runtime.source_to_model,
                &runtime.sampled_locals,
            )
        }) {
            newengine_ulog_api::ulog::warn!(
                "game-ready: equipped weapon skeletal animation failed root={} owner={}: {}",
                root.stable_u64(),
                runtime.owner.stable_u64(),
                error,
            );
        }
        let _ = world.insert(root, runtime);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_mount_prefix_is_not_part_of_skeleton_identity() {
        assert!(skeleton_refs_compatible(
            "models/weapon/rifle/rifle.ymt@rifle",
            "shared/models/weapon/rifle/rifle.ymt@rifle"
        ));
    }
}
