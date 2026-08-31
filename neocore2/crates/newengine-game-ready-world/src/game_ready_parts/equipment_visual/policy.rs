#[derive(Clone, Copy, Debug, PartialEq)]
struct EquippedWeaponVisualRoot {
    owner: EntityId,
    instance_id: newengine_engine_runtime::gameplay::ItemInstanceId,
    item: newengine_engine_runtime::gameplay::ItemId,
    grip_debug_emitted: bool,
    aim_alpha: f32,
    last_shot_sequence: u64,
    recoil_alpha: f32,
    recoil_yaw_radians: f32,
}

const WEAPON_VISUAL_FAILED_PROBE_TICKS: u64 = 30;
const WEAPON_VISUAL_TRANSIENT_RETRY_TICKS: u64 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WeaponVisualAdmissionKey {
    item: newengine_engine_runtime::gameplay::ItemId,
    instance_id: newengine_engine_runtime::gameplay::ItemInstanceId,
    avatar_root: Option<EntityId>,
    dependency_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WeaponVisualAdmissionFailureClass {
    Deterministic,
    Transient,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum WeaponVisualAdmissionState {
    Pending {
        key: WeaponVisualAdmissionKey,
    },
    Ready {
        item: newengine_engine_runtime::gameplay::ItemId,
        instance_id: newengine_engine_runtime::gameplay::ItemInstanceId,
        root: EntityId,
    },
    Failed {
        key: WeaponVisualAdmissionKey,
        class: WeaponVisualAdmissionFailureClass,
        next_probe_tick: u64,
        reason: String,
    },
}

#[inline]
fn weapon_visual_failure_static_matches(
    key: WeaponVisualAdmissionKey,
    item: newengine_engine_runtime::gameplay::ItemId,
    instance_id: newengine_engine_runtime::gameplay::ItemInstanceId,
    avatar_root: Option<EntityId>,
) -> bool {
    key.item == item && key.instance_id == instance_id && key.avatar_root == avatar_root
}

#[inline]
fn weapon_visual_failure_matches(
    key: WeaponVisualAdmissionKey,
    item: newengine_engine_runtime::gameplay::ItemId,
    instance_id: newengine_engine_runtime::gameplay::ItemInstanceId,
    avatar_root: Option<EntityId>,
    dependency_generation: u64,
) -> bool {
    weapon_visual_failure_static_matches(key, item, instance_id, avatar_root)
        && key.dependency_generation == dependency_generation
}

fn classify_weapon_visual_admission_failure(error: &str) -> WeaponVisualAdmissionFailureClass {
    let error = error.to_ascii_lowercase();
    // Availability/readiness failures are allowed a bounded retry even if the dependency read-model
    // did not advance. Structural/content faults default to deterministic and remain suppressed
    // until an authored dependency generation changes.
    if [
        "not ready",
        "pending",
        "temporar",
        "service unavailable",
        "provider unavailable",
        "gateway unavailable",
        "would block",
        "timeout",
    ]
    .iter()
    .any(|token| error.contains(token))
    {
        WeaponVisualAdmissionFailureClass::Transient
    } else {
        WeaponVisualAdmissionFailureClass::Deterministic
    }
}

fn weapon_dependency_logical_path(reference: &str) -> String {
    let normalized = reference
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_owned();
    normalized
        .rsplit_once('@')
        .map(|(path, _)| path.to_owned())
        .unwrap_or(normalized)
}

fn append_weapon_dependency_status_signature(
    signature: &mut String,
    assets: &AssetServiceClient,
    reference: &str,
) {
    let logical_path = weapon_dependency_logical_path(reference);
    if logical_path.is_empty() {
        return;
    }
    let mut candidates = vec![logical_path.clone()];
    if let Some(alias) = logical_path.strip_prefix("shared/") {
        if !alias.is_empty() {
            candidates.push(alias.to_owned());
        }
    }
    for candidate in candidates {
        signature.push_str("asset=");
        signature.push_str(&candidate);
        signature.push('|');
        match assets.status_json_v1(&candidate) {
            Ok(status) => signature.push_str(&status.to_string()),
            Err(error) => {
                signature.push_str("status-error:");
                signature.push_str(&error);
            }
        }
        signature.push(';');
    }
}

fn weapon_visual_dependency_generation(
    world: &newengine_ecs::World,
    mats: &MaterialRegistry,
    binding: EquippedWeaponBinding,
) -> u64 {
    let mut signature = format!("material-registry-revision={};", mats.revision());
    let Some(definition) = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(binding.item))
    else {
        signature.push_str("item-definition=missing;");
        return fnv1a_64(&signature);
    };

    signature.push_str("item-definition=");
    signature.push_str(&definition.name);
    signature.push_str(&format!(
        ";scale={:?};presentation={};model={:?};material={:?};skeleton={:?};animation_dictionary={:?};idle={:?};fire={:?};reload={:?};spawn_pose={:?};casing_ejection_joint={:?};",
        definition.world.scale,
        definition.weapon_presentation.enabled,
        definition.world.model_ref,
        definition.world.material_library_ref,
        definition.weapon_animation.skeleton,
        definition.weapon_animation.animation_dictionary,
        definition.weapon_animation.idle,
        definition.weapon_animation.fire,
        definition.weapon_animation.reload,
        definition.weapon_animation.spawn_pose,
        definition.weapon_casing.ejection_joint,
    ));

    let assets = AssetServiceClient::new(default_host_api());
    let references = [
        definition.definition_ref.as_deref(),
        definition.world.model_ref.as_deref(),
        definition.world.material_library_ref.as_deref(),
        definition.weapon_animation.skeleton.as_deref(),
        definition.weapon_animation.animation_dictionary.as_deref(),
        definition.weapon_animation.idle.as_deref(),
        definition.weapon_animation.fire.as_deref(),
        definition.weapon_animation.reload.as_deref(),
        definition.weapon_animation.spawn_pose.as_deref(),
    ];
    let mut unique = std::collections::BTreeSet::new();
    for reference in references.into_iter().flatten() {
        let path = weapon_dependency_logical_path(reference);
        if unique.insert(path) {
            append_weapon_dependency_status_signature(&mut signature, &assets, reference);
        }
    }
    fnv1a_64(&signature)
}

fn weapon_visual_admission_key(
    world: &newengine_ecs::World,
    mats: &MaterialRegistry,
    owner: EntityId,
    binding: EquippedWeaponBinding,
) -> WeaponVisualAdmissionKey {
    let avatar_root = world
        .get::<PlayerModelBinding>(owner)
        .and_then(|binding| binding.visual_root)
        .filter(|root| world.exists(*root));
    WeaponVisualAdmissionKey {
        item: binding.item,
        instance_id: binding.instance_id,
        avatar_root,
        dependency_generation: weapon_visual_dependency_generation(world, mats, binding),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WeaponSecondaryDynamicsState {
    initialized: bool,
    rotation_offset_local: Vec3,
    angular_velocity_local: Vec3,
    previous_target_rotation: Quat,
    previous_owner_position_world: Vec3,
    previous_owner_velocity_world: Vec3,
}

impl Default for WeaponSecondaryDynamicsState {
    fn default() -> Self {
        Self {
            initialized: false,
            rotation_offset_local: Vec3::ZERO,
            angular_velocity_local: Vec3::ZERO,
            previous_target_rotation: Quat::IDENTITY,
            previous_owner_position_world: Vec3::ZERO,
            previous_owner_velocity_world: Vec3::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EquippedWeaponVisualPart {
    owner: EntityId,
    root: EntityId,
}

fn validate_canonical_skinned_weapon_visual_space(min: Vec3, max: Vec3) -> Result<(), String> {
    if !min.is_finite() || !max.is_finite() {
        return Err(format!(
            "canonical skinned-weapon visual-space has non-finite bounds min={min:?} max={max:?}"
        ));
    }
    let center = (min + max) * 0.5;
    let extent = max - min;
    // Engine validation owns only the canonical coordinate-space invariant. Weapon class/size is
    // authored data: a pistol must not be rejected by a rifle-length heuristic. The root still
    // has to be close to the geometry and +Z remains the canonical weapon-forward axis.
    let canonical = center.x.abs() <= 0.35
        && center.y.abs() <= 0.35
        && center.z.abs() <= 0.35
        && extent.x > 0.005
        && extent.x <= 0.75
        && extent.y > 0.005
        && extent.y <= 0.75
        && extent.z > 0.05
        && extent.z <= 1.50;
    if !canonical {
        return Err(format!(
            "canonical skinned-weapon visual-space rejected min={min:?} max={max:?} center={center:?} extent={extent:?}; expected root-centered +X/+Y/+Z weapon space independent of weapon class"
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

fn sync_equipped_weapon_render_policy(world: &mut newengine_ecs::World, root: EntityId) {
    // Full-body first person uses the same world-space render domain for hands and equipment.
    // A separate view-model pass gives the weapon a different camera/depth cadence and can make
    // it visibly drift or tremble against the skeleton that actually owns the grip contacts.
    let desired = equipped_weapon_render_options();
    let parts = world
        .query::<EquippedWeaponVisualPart>()
        .filter_map(|(entity, part)| (part.root == root).then_some(entity))
        .collect::<Vec<_>>();
    for entity in parts {
        let mut desired_for_part = desired.clone();
        // Skinned equipped geometry uses receive-only world shadows to avoid invalidating the
        // shadow atlas with rapidly animated first-person/equipment skinning.
        if world.get::<PlayerSkinBinding>(entity).is_some() {
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
    let _ = world.remove::<EquippedWeaponEntity>(owner);
    let _ = world.remove::<WeaponVisualAdmissionState>(owner);
}

fn existing_visual(
    world: &newengine_ecs::World,
    owner: EntityId,
) -> Option<(EntityId, EquippedWeaponVisualRoot)> {
    world
        .query::<EquippedWeaponVisualRoot>()
        .find_map(|(entity, root)| (root.owner == owner).then_some((entity, *root)))
}
