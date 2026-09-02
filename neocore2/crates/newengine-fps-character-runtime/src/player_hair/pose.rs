/// Bridge the already-authoritative player animation palette into the provider-neutral
/// render.hair pose registry. Hair never evaluates animation graphs itself.
pub(crate) fn publish_player_hair_pose(world: &mut World, player: EntityId, model_to_world: Mat4) {
    let Some(pose) = world
        .get::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
        .cloned()
    else {
        return;
    };
    let pose_id = player.stable_u64();
    if pose_id == 0 {
        return;
    }
    let hair_pose = HairSkinPoseV1 {
        pose_id,
        revision: pose.revision,
        joint_deforms: pose
            .palette
            .iter()
            .map(|matrix| matrix.to_cols_array())
            .collect(),
    };

    if world.resource::<HairSkinPoseRegistryV1>().is_none() {
        world.insert_resource(HairSkinPoseRegistryV1::default());
    }
    let upsert_result = world
        .resource_mut::<HairSkinPoseRegistryV1>()
        .ok_or_else(|| "HairSkinPoseRegistryV1 unavailable after insertion".to_owned())
        .and_then(|registry| registry.upsert(hair_pose));
    if let Err(error) = upsert_result {
        newengine_ulog_api::ulog::warn!(
            "fps-character: player hair pose publication rejected player={}: {}",
            player.stable_u64(),
            error
        );
        return;
    }

    // Instances are authored/configured by content. The product bridge keeps the root transform
    // synchronized and mirrors the character's first-person visibility policy without teaching
    // the renderer about gameplay camera modes.
    let first_person_active = world
        .resource::<newengine_engine_runtime::gameplay::PlayerViewState>()
        .map(|state| state.first_person_active)
        .unwrap_or(false);
    let binding_state = world
        .resource::<PlayerHairBindingRegistryV1>()
        .and_then(|registry| registry.bindings.get(&pose_id).copied());
    if let Some(scene) = world.resource_mut::<HairSceneV1>() {
        let root = model_to_world.to_cols_array();
        for instance in &mut scene.instances {
            if instance.skin_pose_id == Some(pose_id) {
                instance.root_transform = root;
                if let Some(binding) =
                    binding_state.filter(|binding| binding.instance_id == instance.instance_id)
                {
                    instance.quality = if binding.hide_in_first_person && first_person_active {
                        HairQualityTier::Off
                    } else {
                        binding.authored_quality
                    };
                }
            }
        }
    }
}
