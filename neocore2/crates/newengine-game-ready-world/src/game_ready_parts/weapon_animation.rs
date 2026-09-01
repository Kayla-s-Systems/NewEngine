use super::*;

use newengine_animation_runtime::{
    global_animation_clip_store, AnimationClip, AnimationClipBinding, AnimationClipReference,
    AnimationEventCursor, AnimationEventOccurrence, AnimationSkeletonRuntime, JointLocalPose,
};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient, ASSET_LIST_FILE_BODY_OUTPUT};
use newengine_engine_runtime::gameplay::{
    active_equipped_weapon_binding, HitscanWeaponTuning, ItemInstanceId, PlayerSkinPose,
    PlayerWeaponState, WeaponAnimationDefinition, WeaponEntitySockets, WeaponSocketPose,
};
use newengine_math::Mat4;
use newengine_model_skeleton_api::ModelSkeletonMetadata;

#[derive(Clone, Debug)]
struct WeaponRuntimeClip {
    reference: String,
    clip: std::sync::Arc<AnimationClip>,
    binding: AnimationClipBinding,
    event_cursor: AnimationEventCursor,
}

#[derive(Clone, Debug)]
struct EquippedWeaponAnimationRuntime {
    owner: EntityId,
    instance_id: ItemInstanceId,
    animation_runtime: AnimationSkeletonRuntime,
    idle: Option<WeaponRuntimeClip>,
    fire: Option<WeaponRuntimeClip>,
    reload: Option<WeaponRuntimeClip>,
    spawn_pose: Option<WeaponRuntimeClip>,
    sampled_locals: Vec<JointLocalPose>,
    idle_time: f32,
    fire_time: f32,
    last_shot_sequence: u64,
    reload_active: bool,
    casing_ejection_joint_index: Option<usize>,
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
    animation_runtime: &AnimationSkeletonRuntime,
) -> Result<Option<WeaponRuntimeClip>, String> {
    let Some(reference) = reference.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = AnimationClipReference::parse(reference)?;
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let descriptor = assets.resolve_file_type_v1(&parsed.logical_path)?;
    if !descriptor.semantic_gateway.eq_ignore_ascii_case("engine.animation") {
        return Err(format!(
            "weapon animation ref='{reference}' resolves to format module='{}' gateway='{}', expected engine.animation",
            descriptor.module_id, descriptor.semantic_gateway
        ));
    }
    let clip = global_animation_clip_store()
        .load_ycd_clip(reference, |logical_path| {
            assets
                .decode_v1(&AssetDecodeRequest {
                    logical_path: logical_path.to_owned(),
                    output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                    selector: serde_json::Value::Null,
                                    format_descriptor: None,
})
                .map_err(|error| {
                    format!(
                        "weapon animation asset decode failed ref='{reference}' path='{logical_path}': {error}"
                    )
                })
        })
        .map_err(|error| {
            format!("weapon animation shared clip load failed ref='{reference}': {error}")
        })?;
    if !clip.skeleton_ref.trim().is_empty()
        && !skeleton_refs_compatible(&clip.skeleton_ref, expected_skeleton)
    {
        return Err(format!(
            "weapon animation skeleton mismatch ref='{reference}' clip='{}' expected='{}'",
            clip.skeleton_ref, expected_skeleton
        ));
    }
    let binding = clip.bind_to_skeleton(animation_runtime).map_err(|error| {
        format!("weapon animation runtime binding failed ref='{reference}': {error}")
    })?;
    Ok(Some(WeaponRuntimeClip {
        reference: reference.to_owned(),
        clip,
        binding,
        event_cursor: AnimationEventCursor::default(),
    }))
}

fn publish_weapon_palette(
    world: &mut newengine_ecs::World,
    root: EntityId,
    animation_runtime: &AnimationSkeletonRuntime,
    locals: &[JointLocalPose],
) -> Result<(), String> {
    let mut palette = Vec::with_capacity(animation_runtime.joint_count());
    animation_runtime.build_skin_palette_from_local_pose(locals, &mut palette)?;
    let revision = world
        .get::<PlayerSkinPose>(root)
        .map(|pose| pose.revision.wrapping_add(1).max(1))
        .unwrap_or(1);
    let _ = world.insert(root, PlayerSkinPose { palette, revision });
    Ok(())
}

fn resolve_authored_weapon_joint(
    skeleton: &ModelSkeletonMetadata,
    authored_name: Option<&str>,
    semantic: &str,
) -> Result<Option<usize>, String> {
    let Some(name) = authored_name.map(str::trim).filter(|name| !name.is_empty()) else {
        return Ok(None);
    };
    let matches = skeleton
        .joints
        .iter()
        .enumerate()
        .filter_map(|(index, joint)| (joint.name == name).then_some(index))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(format!(
            "authored weapon socket joint is absent semantic='{semantic}' joint='{name}'"
        )),
        [index] => Ok(Some(*index)),
        _ => Err(format!(
            "authored weapon socket joint is ambiguous semantic='{semantic}' joint='{name}' matches={}",
            matches.len()
        )),
    }
}

pub(crate) fn bind_equipped_weapon_animation(
    world: &mut newengine_ecs::World,
    root: EntityId,
    owner: EntityId,
    instance_id: ItemInstanceId,
    skeleton: ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    definition: &WeaponAnimationDefinition,
    casing_ejection_joint: Option<&str>,
    initial_shot_sequence: u64,
) -> Result<(), String> {
    let expected_skeleton = definition
        .skeleton
        .as_deref()
        .ok_or("skinned weapon animation requires authored skeleton ref")?;
    let casing_ejection_joint_index =
        resolve_authored_weapon_joint(&skeleton, casing_ejection_joint, "casing_ejection")?;
    let animation_runtime = AnimationSkeletonRuntime::compile(&skeleton, source_to_model)
        .map_err(|error| format!("weapon animation skeleton compile failed: {error}"))?;
    let mut idle = load_weapon_clip(
        definition.idle.as_deref(),
        expected_skeleton,
        &animation_runtime,
    )?;
    let fire = load_weapon_clip(
        definition.fire.as_deref(),
        expected_skeleton,
        &animation_runtime,
    )?;
    let reload = load_weapon_clip(
        definition.reload.as_deref(),
        expected_skeleton,
        &animation_runtime,
    )?;
    let mut spawn_pose = load_weapon_clip(
        definition.spawn_pose.as_deref(),
        expected_skeleton,
        &animation_runtime,
    )?;
    if idle.is_none() && fire.is_none() && reload.is_none() && spawn_pose.is_none() {
        return Err("skinned weapon has no authored animation clips".to_owned());
    }

    if let Some(initial) = idle.as_mut().or(spawn_pose.as_mut()) {
        initial.event_cursor.restart();
    }

    let mut sampled_locals = animation_runtime.bind_locals().to_vec();
    if let Some(initial) = spawn_pose.as_ref().or(idle.as_ref()) {
        initial.clip.sample_local_pose_bound(
            0.0,
            &animation_runtime,
            &initial.binding,
            &mut sampled_locals,
        )?;
    }
    publish_weapon_palette(world, root, &animation_runtime, &sampled_locals)?;
    let joint_count = animation_runtime.joint_count();
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
            instance_id,
            animation_runtime,
            idle,
            fire,
            reload,
            spawn_pose,
            sampled_locals,
            idle_time: 0.0,
            fire_time: f32::INFINITY,
            last_shot_sequence: initial_shot_sequence,
            reload_active: false,
            casing_ejection_joint_index,
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

fn sample_weapon_runtime_clip(
    entity: EntityId,
    animation_runtime: &AnimationSkeletonRuntime,
    runtime_clip: &mut WeaponRuntimeClip,
    sampled_locals: &mut Vec<JointLocalPose>,
    playback_time_seconds: f32,
    channel: &str,
    occurrence_scratch: &mut Vec<AnimationEventOccurrence>,
    timeline_events: &mut Vec<newengine_animation_api::AnimationTimelineEventV1>,
) -> Result<(), String> {
    runtime_clip.clip.sample_local_pose_bound(
        playback_time_seconds,
        animation_runtime,
        &runtime_clip.binding,
        sampled_locals,
    )?;
    crate::animation_events::collect_timeline_events(
        entity,
        &runtime_clip.reference,
        channel,
        &runtime_clip.clip,
        &mut runtime_clip.event_cursor,
        playback_time_seconds,
        occurrence_scratch,
        timeline_events,
    )?;
    Ok(())
}

fn publish_weapon_skeleton_sockets(
    world: &mut newengine_ecs::World,
    root: EntityId,
    animation_runtime: &AnimationSkeletonRuntime,
    locals: &[JointLocalPose],
    casing_ejection_joint_index: Option<usize>,
    dt: f32,
) -> Result<(), String> {
    let Some(joint_index) = casing_ejection_joint_index else {
        return Ok(());
    };
    let mut frames = Vec::new();
    animation_runtime.build_model_joint_frames_from_local_pose(locals, &mut frames)?;
    let joint_frame = frames
        .get(joint_index)
        .copied()
        .ok_or_else(|| format!("weapon socket joint frame missing index={joint_index}"))?;
    let (root_position, root_rotation) =
        newengine_transform::read_entity_world_pose_local_chain(world, root)
            .ok_or_else(|| format!("weapon root has no world pose entity={}", root.stable_u64()))?;
    let root_frame = Mat4::from_scale_rotation_translation(
        Vec3::ONE,
        root_rotation.normalize_or_identity(),
        root_position,
    );
    let world_frame = root_frame * joint_frame;
    let (_scale, rotation, position) = world_frame.to_scale_rotation_translation();
    let current = WeaponSocketPose::stationary(position, rotation)
        .ok_or_else(|| format!("weapon socket pose non-finite index={joint_index}"))?;
    let previous = world
        .get::<WeaponEntitySockets>(root)
        .and_then(|sockets| sockets.casing_ejection);
    let current = current.with_measured_motion(previous, dt);
    let mut sockets = world
        .get::<WeaponEntitySockets>(root)
        .copied()
        .unwrap_or_default();
    sockets.casing_ejection = Some(current);
    let _ = world.insert(root, sockets);
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
        let active = active_equipped_weapon_binding(world, runtime.owner)
            .is_some_and(|binding| binding.instance_id == runtime.instance_id);
        let state = if active {
            world
                .get::<PlayerWeaponState>(runtime.owner)
                .copied()
                .unwrap_or_default()
        } else {
            // A stale weapon root may survive the same frame as an equipment switch. Freeze its
            // fire/reload state instead of letting it consume the new weapon instance's sequence.
            PlayerWeaponState {
                shot_sequence: runtime.last_shot_sequence,
                ..PlayerWeaponState::default()
            }
        };

        if state.shot_sequence != runtime.last_shot_sequence {
            runtime.last_shot_sequence = state.shot_sequence;
            runtime.fire_time = 0.0;
            if let Some(fire) = runtime.fire.as_mut() {
                fire.event_cursor.restart();
                newengine_ulog_api::ulog::info!(
                    "game-ready: native weapon fire animation triggered root={} owner={} shot={} clip='{}' duration={:.6}s source='NorthStar assault-fire'",
                    root.stable_u64(),
                    runtime.owner.stable_u64(),
                    state.shot_sequence,
                    fire.reference,
                    fire.clip.duration_seconds,
                );
            }
        }

        let reload_active = state.reload_remaining > 0.0;
        if reload_active && !runtime.reload_active {
            if let Some(reload) = runtime.reload.as_mut() {
                reload.event_cursor.restart();
            }
        }
        runtime.reload_active = reload_active;
        let reload_progress = reload_active.then(|| {
            let duration = world
                .get::<HitscanWeaponTuning>(runtime.owner)
                .map(|tuning| tuning.sanitized().reload_duration)
                .filter(|duration| *duration > 1.0e-4)
                .unwrap_or(2.0);
            (1.0 - state.reload_remaining / duration).clamp(0.0, 1.0)
        });

        let mut occurrence_scratch = Vec::new();
        let mut timeline_events = Vec::new();
        let sampled = if let Some(progress) = reload_progress {
            runtime.fire_time = f32::INFINITY;
            let animation_runtime = &runtime.animation_runtime;
            let sampled_locals = &mut runtime.sampled_locals;
            if let Some(reload) = runtime.reload.as_mut() {
                let sample_time = reload.clip.duration_seconds * progress;
                sample_weapon_runtime_clip(
                    root,
                    animation_runtime,
                    reload,
                    sampled_locals,
                    sample_time,
                    "weapon.reload",
                    &mut occurrence_scratch,
                    &mut timeline_events,
                )
            } else {
                Ok(())
            }
        } else {
            let fire_active = runtime
                .fire
                .as_ref()
                .is_some_and(|fire| runtime.fire_time <= fire.clip.duration_seconds);
            if fire_active {
                let sample_time = runtime.fire_time.max(0.0);
                let animation_runtime = &runtime.animation_runtime;
                let sampled_locals = &mut runtime.sampled_locals;
                let result = sample_weapon_runtime_clip(
                    root,
                    animation_runtime,
                    runtime.fire.as_mut().expect("fire clip checked above"),
                    sampled_locals,
                    sample_time,
                    "weapon.fire",
                    &mut occurrence_scratch,
                    &mut timeline_events,
                );
                runtime.fire_time += dt;
                result
            } else if runtime.idle.is_some() {
                runtime.idle_time += dt;
                let sample_time = runtime.idle_time;
                let animation_runtime = &runtime.animation_runtime;
                let sampled_locals = &mut runtime.sampled_locals;
                sample_weapon_runtime_clip(
                    root,
                    animation_runtime,
                    runtime.idle.as_mut().expect("idle clip checked above"),
                    sampled_locals,
                    sample_time,
                    "weapon.idle",
                    &mut occurrence_scratch,
                    &mut timeline_events,
                )
            } else if runtime.spawn_pose.is_some() {
                let animation_runtime = &runtime.animation_runtime;
                let sampled_locals = &mut runtime.sampled_locals;
                let spawn = runtime
                    .spawn_pose
                    .as_mut()
                    .expect("spawn pose checked above");
                let sample_time = spawn.clip.duration_seconds;
                sample_weapon_runtime_clip(
                    root,
                    animation_runtime,
                    spawn,
                    sampled_locals,
                    sample_time,
                    "weapon.spawn",
                    &mut occurrence_scratch,
                    &mut timeline_events,
                )
            } else {
                Ok(())
            }
        };

        let frame_result = sampled.and_then(|_| {
            publish_weapon_palette(
                world,
                root,
                &runtime.animation_runtime,
                &runtime.sampled_locals,
            )
        });
        if let Err(error) = frame_result {
            newengine_ulog_api::ulog::warn!(
                "game-ready: equipped weapon skeletal animation failed root={} owner={}: {}",
                root.stable_u64(),
                runtime.owner.stable_u64(),
                error,
            );
        } else {
            if let Err(error) = publish_weapon_skeleton_sockets(
                world,
                root,
                &runtime.animation_runtime,
                &runtime.sampled_locals,
                runtime.casing_ejection_joint_index,
                dt,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: weapon socket publication failed root={} owner={}: {}",
                    root.stable_u64(),
                    runtime.owner.stable_u64(),
                    error,
                );
            }
            crate::animation_events::publish_timeline_events(world, timeline_events);
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
