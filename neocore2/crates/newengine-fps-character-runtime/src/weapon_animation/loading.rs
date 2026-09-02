use super::*;

use newengine_animation_runtime::{
    global_animation_clip_store, AnimationClip, AnimationClipBinding, AnimationClipReference,
    AnimationEventCursor, AnimationEventOccurrence, AnimationSkeletonRuntime, JointLocalPose,
};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient, ASSET_LIST_FILE_BODY_OUTPUT};
use newengine_engine_runtime::gameplay::{
    active_equipped_weapon_binding, queue_weapon_reload_animation_marker, ItemInstanceId,
    PlayerSkinPose, PlayerWeaponState, WeaponActionKind, WeaponActionRuntime,
    WeaponAnimationDefinition, WeaponEntitySockets, WeaponReloadAnimationAuthority,
    WeaponReloadAnimationMarker, WeaponReloadAnimationMarkerInbox, WeaponReloadPhase,
    WeaponReloadTopology, WeaponSocketPose,
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
    joint_frames_scratch: Vec<Mat4>,
    occurrence_scratch: Vec<AnimationEventOccurrence>,
    timeline_event_scratch: Vec<newengine_animation_api::AnimationTimelineEventV1>,
    idle_time: f32,
    fire_time: f32,
    last_shot_sequence: u64,
    reload_active: bool,
    reload_markers_authoritative: bool,
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
    if !descriptor
        .semantic_gateway
        .eq_ignore_ascii_case("engine.animation")
    {
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

fn authored_reload_marker_authority(
    weapon_instance_id: ItemInstanceId,
    reload_topology: WeaponReloadTopology,
    reference: &str,
    clip: &AnimationClip,
) -> Result<Option<WeaponReloadAnimationAuthority>, String> {
    let mut phase_times = [None::<f32>; 5];
    let mut marker_mask = 0_u8;
    let mut recognized = 0_usize;
    for event in &clip.events {
        let Some(phase) = WeaponReloadPhase::from_animation_marker_tag(&event.tag) else {
            continue;
        };
        let index = match phase {
            WeaponReloadPhase::MagazineDetached => 0,
            WeaponReloadPhase::AmmoCommitted => 1,
            WeaponReloadPhase::MagazineInserted => 2,
            WeaponReloadPhase::Chambered => 3,
            WeaponReloadPhase::Complete => 4,
            WeaponReloadPhase::None | WeaponReloadPhase::Started => continue,
        };
        if phase_times[index].replace(event.time_seconds).is_some() {
            return Err(format!(
                "reload clip has duplicate authoritative marker clip='{}' tag='{}'",
                reference, event.tag
            ));
        }
        marker_mask |= phase.marker_bit();
        recognized += 1;
    }
    if recognized == 0 {
        return Ok(None);
    }
    let required_marker_mask = reload_topology.required_animation_marker_mask();
    if marker_mask & required_marker_mask != required_marker_mask {
        return Ok(None);
    }
    let mut previous = None::<f32>;
    for time in phase_times.into_iter().flatten() {
        if previous.is_some_and(|previous| time < previous) {
            return Err(format!(
                "reload clip authoritative markers are out of order clip='{}'",
                reference
            ));
        }
        previous = Some(time);
    }
    Ok(Some(WeaponReloadAnimationAuthority {
        weapon_instance_id,
        clip_duration_seconds: clip.duration_seconds,
        marker_mask,
        required_marker_mask,
    }))
}

fn bridge_reload_timeline_markers(
    world: &mut newengine_ecs::World,
    owner: EntityId,
    weapon_instance_id: ItemInstanceId,
    timeline_events: &[newengine_animation_api::AnimationTimelineEventV1],
) {
    for event in timeline_events {
        let Some(phase) = WeaponReloadPhase::from_animation_marker_tag(event.tag.as_str()) else {
            continue;
        };
        queue_weapon_reload_animation_marker(
            world,
            owner,
            WeaponReloadAnimationMarker {
                weapon_instance_id,
                phase,
                clip_time_seconds: event.clip_time_seconds,
                playback_time_seconds: event.playback_time_seconds,
                loop_index: event.loop_index,
            },
        );
    }
}

fn publish_weapon_palette(
    world: &mut newengine_ecs::World,
    root: EntityId,
    animation_runtime: &AnimationSkeletonRuntime,
    locals: &[JointLocalPose],
) -> Result<(), String> {
    if let Some(pose) = world.get_mut::<PlayerSkinPose>(root) {
        animation_runtime.build_skin_palette_from_local_pose(locals, &mut pose.palette)?;
        pose.revision = pose.revision.wrapping_add(1).max(1);
        return Ok(());
    }

    let mut palette = Vec::with_capacity(animation_runtime.joint_count());
    animation_runtime.build_skin_palette_from_local_pose(locals, &mut palette)?;
    let _ = world.insert(
        root,
        PlayerSkinPose {
            palette,
            revision: 1,
        },
    );
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
