pub(crate) fn bind_equipped_weapon_animation(
    world: &mut newengine_ecs::World,
    root: EntityId,
    owner: EntityId,
    instance_id: ItemInstanceId,
    skeleton: ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    definition: &WeaponAnimationDefinition,
    reload_topology: WeaponReloadTopology,
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

    let reload_authority = reload
        .as_ref()
        .map(|reload| {
            authored_reload_marker_authority(
                instance_id,
                reload_topology,
                &reload.reference,
                &reload.clip,
            )
        })
        .transpose()?
        .flatten();
    let reload_markers_authoritative = reload_authority.is_some();
    let _ = world.remove::<WeaponReloadAnimationAuthority>(owner);
    let _ = world.remove::<WeaponReloadAnimationMarkerInbox>(owner);
    if let Some(authority) = reload_authority {
        let _ = world.insert(owner, authority);
        newengine_ulog_api::ulog::info!(
            "fps-character: reload animation marker authority admitted owner={} instance={} clip_duration={:.6}s marker_mask=0x{:02x}",
            owner.stable_u64(),
            instance_id.0,
            authority.clip_duration_seconds,
            authority.marker_mask,
        );
    } else if reload.as_ref().is_some_and(|reload| {
        reload
            .clip
            .events
            .iter()
            .any(|event| WeaponReloadPhase::from_animation_marker_tag(&event.tag).is_some())
    }) {
        newengine_ulog_api::ulog::warn!(
            "fps-character: reload clip has incomplete authoritative marker set; using timeline fallback owner={} instance={}",
            owner.stable_u64(),
            instance_id.0,
        );
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
            joint_frames_scratch: Vec::with_capacity(joint_count),
            occurrence_scratch: Vec::new(),
            timeline_event_scratch: Vec::new(),
            idle_time: 0.0,
            fire_time: f32::INFINITY,
            last_shot_sequence: initial_shot_sequence,
            reload_active: false,
            reload_markers_authoritative,
            casing_ejection_joint_index,
        },
    );
    newengine_ulog_api::ulog::info!(
        "fps-character: equipped weapon skeletal animation bound root={} owner={} joints={} idle='{}' fire='{}' reload='{}' spawn='{}'",
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
