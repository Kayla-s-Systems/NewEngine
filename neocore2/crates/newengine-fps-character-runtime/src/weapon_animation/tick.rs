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
    joint_frames_scratch: &mut Vec<Mat4>,
    casing_ejection_joint_index: Option<usize>,
    dt: f32,
) -> Result<(), String> {
    let Some(joint_index) = casing_ejection_joint_index else {
        return Ok(());
    };
    animation_runtime.build_model_joint_frames_from_local_pose(locals, joint_frames_scratch)?;
    let joint_frame = joint_frames_scratch
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
                    "fps-character: native weapon fire animation triggered root={} owner={} shot={} clip='{}' duration={:.6}s source='NorthStar assault-fire'",
                    root.stable_u64(),
                    runtime.owner.stable_u64(),
                    state.shot_sequence,
                    fire.reference,
                    fire.clip.duration_seconds,
                );
            }
        }

        let reload_action = active
            .then(|| world.get::<WeaponActionRuntime>(runtime.owner).copied())
            .flatten()
            .filter(|action| {
                action.weapon_instance_id == runtime.instance_id
                    && action.action == WeaponActionKind::Reloading
            });
        let reload_active = reload_action.is_some();
        if reload_active && !runtime.reload_active {
            if let Some(reload) = runtime.reload.as_mut() {
                reload.event_cursor.restart();
            }
        }
        runtime.reload_active = reload_active;
        let reload_progress = reload_action.map(WeaponActionRuntime::progress);

        runtime.occurrence_scratch.clear();
        runtime.timeline_event_scratch.clear();
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
                    &mut runtime.occurrence_scratch,
                    &mut runtime.timeline_event_scratch,
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
                    &mut runtime.occurrence_scratch,
                    &mut runtime.timeline_event_scratch,
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
                    &mut runtime.occurrence_scratch,
                    &mut runtime.timeline_event_scratch,
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
                    &mut runtime.occurrence_scratch,
                    &mut runtime.timeline_event_scratch,
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
                "fps-character: equipped weapon skeletal animation failed root={} owner={}: {}",
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
                &mut runtime.joint_frames_scratch,
                runtime.casing_ejection_joint_index,
                dt,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "fps-character: weapon socket publication failed root={} owner={}: {}",
                    root.stable_u64(),
                    runtime.owner.stable_u64(),
                    error,
                );
            }
            if runtime.reload_markers_authoritative && reload_active {
                bridge_reload_timeline_markers(
                    world,
                    runtime.owner,
                    runtime.instance_id,
                    &runtime.timeline_event_scratch,
                );
            }
            crate::animation_events::publish_timeline_events(
                world,
                &mut runtime.timeline_event_scratch,
            );
        }
        let _ = world.insert(root, runtime);
    }
}
