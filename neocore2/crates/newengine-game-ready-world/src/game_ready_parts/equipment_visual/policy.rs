#[derive(Clone, Copy, Debug, PartialEq)]
struct EquippedWeaponVisualRoot {
    owner: EntityId,
    instance_id: newengine_engine_runtime::gameplay::ItemInstanceId,
    item: newengine_engine_runtime::gameplay::ItemId,
    grip_debug_emitted: bool,
    aim_alpha: f32,
    last_shot_sequence: u64,
    recoil_alpha: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EquippedWeaponVisualPart {
    owner: EntityId,
    root: EntityId,
}

fn validate_canonical_rifle_visual_space(min: Vec3, max: Vec3) -> Result<(), String> {
    let center = (min + max) * 0.5;
    let extent = max - min;
    let canonical = center.x.abs() <= 0.20
        && center.y.abs() <= 0.20
        && center.z.abs() <= 0.30
        && extent.x > 0.05
        && extent.x <= 0.40
        && extent.y > 0.05
        && extent.y <= 0.40
        && extent.z >= 0.75
        && extent.z <= 1.25;
    if !canonical {
        return Err(format!(
            "canonical rifle visual-space rejected min={min:?} max={max:?} center={center:?} extent={extent:?}; expected handle-centered +X/+Y/+Z weapon space"
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WeaponVisualAlignment {
    grip_pivot: Vec3,
}

fn decoded_model_bounds(decoded: &[DecodedPrefabMeshPart]) -> Result<(Vec3, Vec3), String> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for part in decoded {
        for vertex in &part.mesh.vertices {
            let point = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
            min = min.min(point);
            max = max.max(point);
        }
    }
    if !min.is_finite() || !max.is_finite() {
        return Err("equipped weapon YDD produced no finite geometry bounds".to_owned());
    }
    Ok((min, max))
}

fn weapon_visual_alignment(
    decoded: &[DecodedPrefabMeshPart],
    authored_presentation: bool,
) -> Result<WeaponVisualAlignment, String> {
    let (min, max) = decoded_model_bounds(decoded)?;
    Ok(WeaponVisualAlignment {
        // Presentation-enabled assets are authored in their definition-owned root/handle space.
        // Generic uncalibrated assets retain the geometric-center fallback.
        grip_pivot: if authored_presentation {
            Vec3::ZERO
        } else {
            (min + max) * 0.5
        },
    })
}

fn equipped_weapon_render_options() -> newengine_model_domain_api::MeshRenderOptions {
    let mut options = newengine_model_domain_api::MeshRenderOptions::world_opaque();
    options.shadow_policy = newengine_model_domain_api::MeshShadowPolicy::CastAndReceive;
    options
}

#[inline]
fn first_person_weapon_render_options() -> newengine_model_domain_api::MeshRenderOptions {
    // First-person weapons are an overlay/view-model domain, not ordinary world opaque geometry.
    // The pose is still authored as a world transform so gameplay and muzzle math share one frame,
    // while the render role keeps it in the forward view-model pass with no self-shadow casting.
    newengine_model_domain_api::MeshRenderOptions::first_person_view_model()
}

fn sync_equipped_weapon_render_policy(
    world: &mut newengine_ecs::World,
    root: EntityId,
    first_person_active: bool,
) {
    let desired = if first_person_active {
        first_person_weapon_render_options()
    } else {
        equipped_weapon_render_options()
    };
    let parts = world
        .query::<EquippedWeaponVisualPart>()
        .filter_map(|(entity, part)| (part.root == root).then_some(entity))
        .collect::<Vec<_>>();
    for entity in parts {
        let mut desired_for_part = desired.clone();
        // Skinned equipped geometry uses receive-only world shadows to avoid invalidating the
        // shadow atlas with rapidly animated first-person/equipment skinning.
        if world.get::<PlayerSkinBinding>(entity).is_some() && !first_person_active {
            desired_for_part.shadow_policy =
                newengine_model_domain_api::MeshShadowPolicy::ReceiveOnly;
        }
        let needs_update = world
            .get::<newengine_model_domain_api::MeshRenderOptions>(entity)
            .map(|current| current != &desired_for_part)
            .unwrap_or(true);
        if needs_update {
            let _ = world.insert(entity, desired_for_part);
        }
    }
}

fn equipped_part_material_asset(
    part_material_ref: Option<&str>,
    material_slot: &str,
    fallback_material_library: Option<&str>,
) -> Option<String> {
    match part_material_ref {
        Some(reference) if reference.contains('@') => Some(reference.trim().to_owned()),
        Some(reference) if !reference.trim().is_empty() => {
            Some(format!("{}@{}", reference.trim(), material_slot))
        }
        _ => fallback_material_library
            .map(str::trim)
            .filter(|reference| !reference.is_empty())
            .map(|reference| {
                if reference.contains('@') {
                    reference.to_owned()
                } else {
                    format!("{reference}@{material_slot}")
                }
            }),
    }
}

fn register_equipped_part_material(
    mats: &MaterialRegistry,
    item_name: &str,
    part_index: usize,
    part: &DecodedPrefabMeshPart,
    fallback_material_library: Option<&str>,
) -> Result<MaterialId, String> {
    let material_asset = equipped_part_material_asset(
        part.material_ref.as_deref(),
        &part.material_slot,
        fallback_material_library,
    );
    let spec = GameReadyMaterialSpec {
        asset: material_asset,
        base_color_texture: None,
        normal_texture: None,
        roughness_texture: None,
        uv_scale: [1.0, 1.0],
        uv_offset: [0.0, 0.0],
        roughness: 0.72,
        normal_scale: 1.0,
        occlusion_strength: 1.0,
    };
    let logical_name = format!(
        "EquippedWeapon/{item_name}/Part{part_index}:{}",
        part.material_slot
    );
    let material_id = register_required_material(
        mats,
        &logical_name,
        MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
        &spec,
    )?;
    let resolved = newengine_materials::api::MaterialRegistryApi::resolve(mats, material_id)
        .ok_or_else(|| {
            format!(
                "required equipped material disappeared after registration name='{logical_name}'"
            )
        })?;
    let mut missing = Vec::new();
    if resolved.textures.base_color_texture.is_none() {
        missing.push("base_color");
    }
    if resolved.textures.normal_texture.is_none() {
        missing.push("normal");
    }
    if resolved.textures.roughness_texture.is_none() {
        missing.push("roughness");
    }
    if !missing.is_empty() {
        return Err(format!(
            "required equipped PBR material is incomplete name='{}' asset={:?} missing={:?}",
            logical_name, spec.asset, missing
        ));
    }
    Ok(material_id)
}

fn clear_equipped_weapon_visual(world: &mut newengine_ecs::World, owner: EntityId) {
    let parts = world
        .query::<EquippedWeaponVisualPart>()
        .filter_map(|(entity, part)| (part.owner == owner).then_some(entity))
        .collect::<Vec<_>>();
    for entity in parts {
        let _ = world.despawn(entity);
    }
    let roots = world
        .query::<EquippedWeaponVisualRoot>()
        .filter_map(|(entity, root)| (root.owner == owner).then_some(entity))
        .collect::<Vec<_>>();
    for entity in roots {
        let _ = world.despawn(entity);
    }
    let _ = world.remove::<EquippedWeaponMuzzle>(owner);
}

fn existing_visual(
    world: &newengine_ecs::World,
    owner: EntityId,
) -> Option<(EntityId, EquippedWeaponVisualRoot)> {
    world
        .query::<EquippedWeaponVisualRoot>()
        .find_map(|(entity, root)| (root.owner == owner).then_some((entity, *root)))
}
