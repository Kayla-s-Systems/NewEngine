use super::foliage::{decode_runtime_ydd_prefab, DecodedPrefabMeshPart};
use super::*;

use newengine_engine_runtime::gameplay::{
    DisplayMode, DisplayVisibility, EquippedWeaponBinding, ItemCatalog, PlayerModelBinding,
    PlayerViewVisibility, PlayerViewVisibilityPolicy, PlayerVisualKind, PlayerVisualPart,
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct EquippedWeaponVisualRoot {
    owner: EntityId,
    instance_id: newengine_engine_runtime::gameplay::ItemInstanceId,
    item: newengine_engine_runtime::gameplay::ItemId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EquippedWeaponVisualPart {
    owner: EntityId,
    root: EntityId,
}

#[derive(Clone, Copy, Debug)]
struct WeaponVisualAlignment {
    grip_pivot: Vec3,
    two_hand_support: Option<Vec3>,
}

const RIFLE_MODEL_REF: &str = "models/weapon/rifle/rifle.ydd@rifle";

/// Canonical rifle.gltf is authored in North Star weapon-space:
/// +X right, +Y up, +Z muzzle/forward, origin at the pistol grip.
/// The support landmark was measured from the recovered source before canonicalization and is
/// only used as a direction constraint; weapon scale stays authored by the GLTF itself.
const RIFLE_SUPPORT_LOCAL: Vec3 = Vec3::new(0.07965, 0.03745, 0.459_376_6);

fn weapon_visual_alignment(
    model_ref: &str,
    decoded: &[DecodedPrefabMeshPart],
) -> Result<WeaponVisualAlignment, String> {
    if model_ref.eq_ignore_ascii_case(RIFLE_MODEL_REF) {
        return Ok(WeaponVisualAlignment {
            grip_pivot: Vec3::ZERO,
            two_hand_support: Some(RIFLE_SUPPORT_LOCAL),
        });
    }
    Ok(WeaponVisualAlignment {
        grip_pivot: decoded_model_center(decoded)?,
        two_hand_support: None,
    })
}

fn decoded_model_center(decoded: &[DecodedPrefabMeshPart]) -> Result<Vec3, String> {
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
    Ok((min + max) * 0.5)
}

fn register_equipped_part_material(
    mats: &MaterialRegistry,
    item_name: &str,
    part_index: usize,
    part: &DecodedPrefabMeshPart,
) -> MaterialId {
    let spec = GameReadyMaterialSpec {
        asset: part.material_ref.clone(),
        base_color_texture: None,
        normal_texture: None,
        roughness_texture: None,
        uv_scale: [1.0, 1.0],
        uv_offset: [0.0, 0.0],
        roughness: 0.72,
        normal_scale: 1.0,
        occlusion_strength: 1.0,
    };
    let diagnostic_color = match part.material_slot.as_str() {
        "m00" => [0.10, 0.13, 0.10, 1.0],
        "m01" => [0.07, 0.08, 0.09, 1.0],
        _ => [0.12, 0.13, 0.13, 1.0],
    };
    register_material(
        mats,
        &format!(
            "EquippedWeapon/{item_name}/Part{part_index}:{}",
            part.material_slot
        ),
        diagnostic_color,
        [0.0, 0.0, 0.0],
        1.0,
        MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
        &spec,
    )
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
}

fn existing_visual(
    world: &newengine_ecs::World,
    owner: EntityId,
) -> Option<(EntityId, EquippedWeaponVisualRoot)> {
    world
        .query::<EquippedWeaponVisualRoot>()
        .find_map(|(entity, root)| (root.owner == owner).then_some((entity, *root)))
}

fn spawn_equipped_weapon_visual(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    owner: EntityId,
    binding: EquippedWeaponBinding,
) -> Result<EntityId, String> {
    let definition = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(binding.item))
        .cloned()
        .ok_or_else(|| "equipped item definition is unavailable".to_owned())?;
    let world_definition = definition.world.clone().sanitized();
    let model_ref = world_definition.model_ref.as_deref().ok_or_else(|| {
        format!(
            "equipped weapon '{}' has no authored model",
            definition.name
        )
    })?;
    let avatar_root = world
        .get::<PlayerModelBinding>(owner)
        .and_then(|binding| binding.visual_root)
        .filter(|root| world.exists(*root))
        .ok_or_else(|| "player avatar visual root is not ready".to_owned())?;
    let decoded = decode_runtime_ydd_prefab(model_ref)
        .map_err(|error| format!("equipped weapon model decode failed '{model_ref}': {error}"))?;
    let alignment = weapon_visual_alignment(model_ref, &decoded)?;

    let root = spawn_named(world, format!("Player/EquippedWeapon/{}", definition.name));
    let _ = world.insert(root, Transform::default());
    let _ = world.insert(
        root,
        EquippedWeaponVisualRoot {
            owner,
            instance_id: binding.instance_id,
            item: binding.item,
        },
    );
    let _ = world.insert(
        root,
        DisplayVisibility {
            mode: DisplayMode::GameOnly,
        },
    );
    let _ = set_parent(world, root, Some(avatar_root));

    let authored_scale = Vec3::new(
        world_definition.scale[0],
        world_definition.scale[1],
        world_definition.scale[2],
    );
    let mut spawned = 0usize;
    for (part_index, part) in decoded.iter().enumerate() {
        if !prims.is_registered(part.primitive_id) {
            prims.register_mesh(part.primitive_id, part.name.clone(), part.mesh.clone());
        }
        let material_id = register_equipped_part_material(mats, &definition.name, part_index, part);
        let entity = spawn_game_primitive(
            world,
            &*prims,
            mats,
            PrimitiveSpawnSpec {
                parent: root,
                primitive_id: part.primitive_id,
                material_id,
                name: &format!(
                    "Player/EquippedWeapon/{}/{}-{part_index}",
                    definition.name, part.material_slot
                ),
                // Canonical rifle.gltf is already grip-centered. Generic future weapon sources
                // may still use a decoded center fallback until they author an explicit grip.
                position: -alignment.grip_pivot,
                scale: authored_scale,
                color: [1.0, 1.0, 1.0, 1.0],
                render_options: newengine_model_domain_api::MeshRenderOptions::world_opaque(),
            },
        );
        let _ = world.insert(entity, EquippedWeaponVisualPart { owner, root });
        let _ = world.insert(
            entity,
            PlayerVisualPart {
                owner,
                part_index: part_index as u32,
                kind: PlayerVisualKind::EquippedWeapon,
                material_slot: part.material_slot.clone(),
            },
        );
        let _ = world.insert(
            entity,
            PlayerViewVisibility {
                base_mode: DisplayMode::GameOnly,
                policy: PlayerViewVisibilityPolicy::AlwaysVisible,
            },
        );
        spawned += 1;
    }
    if spawned == 0 {
        clear_equipped_weapon_visual(world, owner);
        return Err("equipped weapon model contains no renderable parts".to_owned());
    }

    newengine_ulog_api::ulog::info!(
        "game-ready: equipped weapon visual bound player={} item='{}' instance={} model='{}' parts={} attachment='r_hand_prop_attachment' alignment='authored-grip/original-rifle-support'",
        owner.stable_u64(),
        definition.name,
        binding.instance_id.0,
        model_ref,
        spawned,
    );
    Ok(root)
}

fn signed_angle_around_axis(from: Vec3, to: Vec3, axis: Vec3) -> f32 {
    let from = (from - axis * from.dot(axis)).normalize_or_zero();
    let to = (to - axis * to.dot(axis)).normalize_or_zero();
    if from.length_squared() <= 1.0e-8 || to.length_squared() <= 1.0e-8 {
        return 0.0;
    }
    axis.dot(from.cross(to))
        .atan2(from.dot(to).clamp(-1.0, 1.0))
}

fn rifle_two_hand_rotation(right: Vec3, left: Vec3, support_local: Vec3) -> Option<Quat> {
    let target = left - right;
    if !target.is_finite() || target.length_squared() <= 1.0e-8 {
        return None;
    }
    let authored_forward = support_local.normalize_or_zero();
    let target_forward = target.normalize_or_zero();
    if authored_forward.length_squared() <= 1.0e-8 || target_forward.length_squared() <= 1.0e-8 {
        return None;
    }

    // Swing the authored grip->fore-end axis exactly onto the animated hand-to-hand line.
    let swing = Quat::from_rotation_arc(authored_forward, target_forward).normalize_or_identity();

    // Resolve the remaining roll by keeping the top of the receiver as close as possible to
    // character/model +Y. This prevents the rifle from hanging blade-like beside the thigh while
    // preserving the exact two-hand support direction from the native Abby stance.
    let current_up = swing * Vec3::Y;
    let model_up = Vec3::Y;
    let roll = signed_angle_around_axis(current_up, model_up, target_forward);
    Some((Quat::from_axis_angle(target_forward, roll) * swing).normalize_or_identity())
}

fn update_weapon_attachment(world: &mut newengine_ecs::World, owner: EntityId, root: EntityId) {
    let Some(right_frame) = super::player_model::player_right_hand_prop_frame(world, owner) else {
        return;
    };
    let Some(visual) = world.get::<EquippedWeaponVisualRoot>(root).copied() else {
        return;
    };
    let (right_scale, right_rotation, right_translation) =
        right_frame.to_scale_rotation_translation();
    if !right_translation.is_finite()
        || !right_rotation.is_finite()
        || !right_scale.is_finite()
        || right_scale.x <= 0.0
        || right_scale.y <= 0.0
        || right_scale.z <= 0.0
    {
        return;
    }

    let model_ref = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(visual.item))
        .and_then(|definition| definition.world.model_ref.as_deref())
        .unwrap_or_default();

    let rotation = if model_ref.eq_ignore_ascii_case(RIFLE_MODEL_REF) {
        let left_translation = super::player_model::player_left_hand_prop_frame(world, owner)
            .map(|frame| frame.to_scale_rotation_translation().2);
        left_translation
            .and_then(|left| rifle_two_hand_rotation(right_translation, left, RIFLE_SUPPORT_LOCAL))
            .unwrap_or(right_rotation)
    } else {
        right_rotation
    };

    if let Some(transform) = world.get_mut::<Transform>(root) {
        transform.position = right_translation;
        transform.rotation = rotation;
        transform.scale = Vec3::ONE;
    }
}

pub(crate) fn tick_equipped_weapon_visuals(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
) {
    let owners = world
        .query::<newengine_engine_runtime::gameplay::PlayerController>()
        .map(|(owner, _)| owner)
        .collect::<Vec<_>>();

    for owner in owners {
        let binding = world.get::<EquippedWeaponBinding>(owner).copied();
        match (binding, existing_visual(world, owner)) {
            (None, Some(_)) => clear_equipped_weapon_visual(world, owner),
            (None, None) => {}
            (Some(binding), Some((root, visual)))
                if visual.instance_id == binding.instance_id && world.exists(root) =>
            {
                update_weapon_attachment(world, owner, root);
            }
            (Some(binding), existing) => {
                if existing.is_some() {
                    clear_equipped_weapon_visual(world, owner);
                }
                match spawn_equipped_weapon_visual(world, prims, mats, owner, binding) {
                    Ok(root) => update_weapon_attachment(world, owner, root),
                    Err(error) => {
                        // Avatar/model admission can lag inventory by a few frames during startup;
                        // retry quietly until both are resident. Non-transient faults remain visible
                        // through the normal asset/material diagnostics.
                        if world
                            .get::<PlayerModelBinding>(owner)
                            .and_then(|binding| binding.visual_root)
                            .is_some()
                        {
                            newengine_ulog_api::ulog::warn!(
                                "game-ready: equipped weapon visual deferred player={} item={:016x}: {}",
                                owner.stable_u64(),
                                binding.item.raw(),
                                error,
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod alignment_tests {
    use super::*;

    #[test]
    fn rifle_two_hand_solver_aligns_support_axis_and_keeps_receiver_up() {
        let right = Vec3::new(0.10, 1.20, 0.20);
        let left = Vec3::new(0.38, 1.12, -0.24);
        let rotation =
            rifle_two_hand_rotation(right, left, RIFLE_SUPPORT_LOCAL).expect("two-hand rotation");
        let authored = (rotation * RIFLE_SUPPORT_LOCAL).normalize();
        let target = (left - right).normalize();
        assert!(
            authored.dot(target) > 0.9999,
            "authored={authored:?} target={target:?}"
        );

        let up = rotation * Vec3::Y;
        let projected_up = (up - target * up.dot(target)).normalize_or_zero();
        let model_up = (Vec3::Y - target * Vec3::Y.dot(target)).normalize_or_zero();
        assert!(
            projected_up.dot(model_up) > 0.999,
            "up={projected_up:?} model_up={model_up:?}"
        );
    }
}
