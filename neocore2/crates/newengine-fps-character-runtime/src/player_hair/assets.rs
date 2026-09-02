/// Decode one compiled NorthStar NEHAIR groom through the canonical engine.assets VFS path.
pub fn load_nehair_groom_v1(logical_path: &str) -> Result<HairGroomAssetV1, String> {
    let logical_path = logical_path.trim().replace('\\', "/");
    if logical_path.is_empty()
        || logical_path.starts_with('/')
        || logical_path.contains(":/")
        || logical_path.contains("../")
    {
        return Err("NEHAIR path must be a VFS-relative logical asset path".to_owned());
    }
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let bytes = assets
        .raw_bytes_v1(&logical_path)
        .map_err(|error| format!("NEHAIR VFS read failed path='{logical_path}': {error}"))?;
    newengine_asset_format_nehair::decode_nehair(&bytes)
        .map_err(|error| format!("NEHAIR decode failed path='{logical_path}': {error}"))
}

/// Publish one compiled groom into the renderer-owned HairGroomRegistryV1.
pub fn install_nehair_groom_v1(world: &mut World, logical_path: &str) -> Result<(), String> {
    let groom = load_nehair_groom_v1(logical_path)?;
    if world.resource::<HairGroomRegistryV1>().is_none() {
        world.insert_resource(HairGroomRegistryV1::default());
    }
    world
        .resource_mut::<HairGroomRegistryV1>()
        .ok_or_else(|| "HairGroomRegistryV1 unavailable after insertion".to_owned())?
        .insert(groom)
}

/// Bind an already-decoded groom to an animated player. Content owns the instance/material
/// settings and shader set; this bridge only resolves the opaque pose id and registry ownership.
pub fn bind_compiled_player_groom_v1(
    world: &mut World,
    player: EntityId,
    groom: HairGroomAssetV1,
    mut instance: HairInstanceDescV1,
    shaders: HairShaderSetV1,
) -> Result<(), String> {
    let pose_id = player.stable_u64();
    if pose_id == 0 {
        return Err("player entity does not expose a stable non-zero hair pose id".to_owned());
    }
    let groom = groom.normalized()?;
    let groom_ref = groom.groom.clone();
    if world.resource::<HairGroomRegistryV1>().is_none() {
        world.insert_resource(HairGroomRegistryV1::default());
    }
    world
        .resource_mut::<HairGroomRegistryV1>()
        .ok_or_else(|| "HairGroomRegistryV1 unavailable after insertion".to_owned())?
        .insert(groom)?;

    instance.groom = groom_ref;
    instance.skin_pose_id = Some(pose_id);
    instance = instance.normalized()?;

    if world.resource::<HairSceneV1>().is_none() {
        world.insert_resource(HairSceneV1::new(shaders.clone()));
    }
    let scene = world
        .resource_mut::<HairSceneV1>()
        .ok_or_else(|| "HairSceneV1 unavailable after insertion".to_owned())?;
    if scene.shaders != shaders {
        if scene.instances.is_empty() {
            scene.shaders = shaders.clone();
        } else {
            return Err(
                "HairSceneV1 already exists with a different shader set while live instances remain"
                    .to_owned(),
            );
        }
    }
    if scene
        .instances
        .iter()
        .any(|existing| existing.instance_id == instance.instance_id)
    {
        return Err(format!(
            "hair instance id {} is already bound",
            instance.instance_id
        ));
    }
    scene.instances.push(instance);
    Ok(())
}

/// VFS-backed convenience path: decode NEHAIR through engine.assets and bind it to a player.
pub fn bind_player_nehair_v1(
    world: &mut World,
    player: EntityId,
    logical_path: &str,
    instance: HairInstanceDescV1,
    shaders: HairShaderSetV1,
) -> Result<(), String> {
    let groom = load_nehair_groom_v1(logical_path)?;
    bind_compiled_player_groom_v1(world, player, groom, instance, shaders)
}

