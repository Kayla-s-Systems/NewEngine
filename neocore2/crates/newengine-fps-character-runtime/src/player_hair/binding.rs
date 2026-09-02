#[inline]
fn runtime_player_hair_instance_id(player: EntityId, groom_path: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in player
        .stable_u64()
        .to_le_bytes()
        .into_iter()
        .chain(groom_path.as_bytes().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash.max(1)
}

pub(crate) fn source_mesh_replaced_by_hair_v1(
    prepared: &PreparedPlayerHairV1,
    source_mesh_name: &str,
) -> bool {
    let name = source_mesh_name.trim().to_ascii_lowercase();
    !name.is_empty()
        && prepared.source_mesh_prefixes.iter().any(|prefix| {
            let prefix = prefix.trim().to_ascii_lowercase();
            !prefix.is_empty() && name.starts_with(&prefix)
        })
}

pub(crate) fn unbind_player_hair_v1(world: &mut World, player: EntityId) -> usize {
    let pose_id = player.stable_u64();
    if let Some(registry) = world.resource_mut::<PlayerHairBindingRegistryV1>() {
        registry.bindings.remove(&pose_id);
    }
    let Some(scene) = world.resource_mut::<HairSceneV1>() else {
        return 0;
    };
    let before = scene.instances.len();
    scene
        .instances
        .retain(|instance| instance.skin_pose_id != Some(pose_id));
    before.saturating_sub(scene.instances.len())
}

pub(crate) fn bind_prepared_player_hair_v1(
    world: &mut World,
    player: EntityId,
    prepared: PreparedPlayerHairV1,
) -> Result<(), String> {
    let pose_id = player.stable_u64();
    let state = PlayerHairBindingStateV1 {
        instance_id: prepared.instance.instance_id,
        authored_quality: prepared.instance.quality,
        hide_in_first_person: prepared.hide_in_first_person,
    };
    bind_compiled_player_groom_v1(
        world,
        player,
        prepared.groom,
        prepared.instance,
        prepared.shaders,
    )?;
    if world.resource::<PlayerHairBindingRegistryV1>().is_none() {
        world.insert_resource(PlayerHairBindingRegistryV1::default());
    }
    world
        .resource_mut::<PlayerHairBindingRegistryV1>()
        .ok_or_else(|| "PlayerHairBindingRegistryV1 unavailable after insertion".to_owned())?
        .bindings
        .insert(pose_id, state);
    Ok(())
}

