use super::*;

use newengine_gameplay_fps_api::WeaponShellCasing;

#[derive(Clone, Copy, Debug)]
struct WeaponShellVisualAdmitted;

#[derive(Clone, Copy, Debug)]
struct WeaponShellVisualLastError(u64);

#[inline]
fn shell_render_options() -> newengine_model_domain_api::MeshRenderOptions {
    let mut options = newengine_model_domain_api::MeshRenderOptions::world_opaque();
    options.shadow_policy = newengine_model_domain_api::MeshShadowPolicy::ReceiveOnly;
    options
}

fn try_admit_shell_visual(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    owner: EntityId,
    casing: WeaponShellCasing,
) -> Result<(), String> {
    let casing_definition = world
        .resource::<newengine_engine_runtime::gameplay::ItemCatalog>()
        .and_then(|catalog| {
            catalog.get(newengine_engine_runtime::gameplay::ItemId(
                casing.weapon_item_id,
            ))
        })
        .map(|definition| definition.weapon_casing.clone().sanitized())
        .ok_or_else(|| {
            format!(
                "weapon casing source definition missing item={:016x}",
                casing.weapon_item_id
            )
        })?;
    if !casing_definition.enabled() {
        return Err(format!(
            "weapon casing definition disabled item={:016x}",
            casing.weapon_item_id
        ));
    }
    let variant = casing_definition
        .variants
        .get(casing.variant as usize % casing_definition.variants.len())
        .cloned()
        .ok_or_else(|| "weapon casing variant list is empty".to_owned())?;
    let model_ref = casing_definition
        .model_ref(casing.variant as usize)
        .ok_or_else(|| "weapon casing model reference is not authored".to_owned())?;
    let decoded = crate::foliage::decode_runtime_ydd_prefab(&model_ref)
        .map_err(|error| format!("weapon shell model decode failed '{model_ref}': {error}"))?;
    if decoded.is_empty() {
        return Err(format!(
            "weapon shell model '{model_ref}' contains no renderable parts"
        ));
    }

    // Resolve every material before mutating the hierarchy so admission is atomic.
    let materials = decoded
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let material_ref = part
                .material_ref
                .as_deref()
                .or(casing_definition.material_ref.as_deref())
                .ok_or_else(|| {
                    format!(
                        "weapon casing material missing item={:016x} variant='{}' part={}",
                        casing.weapon_item_id, variant, index
                    )
                })?;
            crate::material_source::register_required_material_ref(
                mats,
                &format!(
                    "WeaponShell/{:016x}/{variant}/{index}",
                    casing.weapon_item_id
                ),
                MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
                material_ref,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut spawned = 0usize;
    for ((index, part), material_id) in decoded.iter().enumerate().zip(materials) {
        if !prims.is_registered(part.primitive_id) {
            prims.register_mesh(part.primitive_id, part.name.clone(), part.mesh.clone());
        }
        let child = crate::materials_terrain::spawn_game_primitive(
            world,
            &*prims,
            mats,
            crate::materials_terrain::PrimitiveSpawnSpec {
                parent: owner,
                primitive_id: part.primitive_id,
                material_id,
                name: &format!(
                    "WeaponFx/ShellCasing/Native/{:016x}/{}/{}",
                    owner.stable_u64(),
                    variant,
                    index
                ),
                position: Vec3::ZERO,
                scale: Vec3::ONE,
                color: [1.0, 1.0, 1.0, 1.0],
                render_options: shell_render_options(),
            },
        );
        let _ = world.insert(
            child,
            newengine_engine_runtime::gameplay::DisplayVisibility::default(),
        );
        spawned += 1;
    }
    if spawned == 0 {
        return Err(format!("weapon shell model '{model_ref}' spawned no parts"));
    }

    // The root retains Transform + PhysicsBodyDesc + Bounds + Velocity. Only the temporary
    // boot-safe cube is removed after the exact authored YDD hierarchy is resident.
    let _ = world.remove::<Primitive>(owner);
    let _ = world.remove::<newengine_model_domain_api::MeshRenderOptions>(owner);
    let _ = world.insert(owner, WeaponShellVisualAdmitted);
    let _ = world.remove::<WeaponShellVisualLastError>(owner);
    newengine_ulog_api::ulog::info!(
        "game-ready: weapon casing visual admitted entity={} shot={} weapon_item={:016x} variant='{}' model='{}' parts={} persistence='physics-world' source='authored-item-definition'",
        owner.stable_u64(),
        casing.shot_sequence,
        casing.weapon_item_id,
        variant,
        model_ref,
        spawned,
    );
    Ok(())
}

pub(crate) fn tick_weapon_shell_casing_visuals(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
) {
    let pending = world
        .query::<WeaponShellCasing>()
        .filter_map(|(entity, casing)| {
            (world.get::<WeaponShellVisualAdmitted>(entity).is_none()).then_some((entity, *casing))
        })
        .collect::<Vec<_>>();

    for (entity, casing) in pending {
        if let Err(error) = try_admit_shell_visual(world, prims, mats, entity, casing) {
            let hash = fnv1a_64(&error);
            let changed = world
                .get::<WeaponShellVisualLastError>(entity)
                .is_none_or(|previous| previous.0 != hash);
            if changed {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: weapon casing visual deferred entity={} shot={} weapon_item={:016x} variant={} err='{}'",
                    entity.stable_u64(),
                    casing.shot_sequence,
                    casing.weapon_item_id,
                    casing.variant,
                    error,
                );
                let _ = world.insert(entity, WeaponShellVisualLastError(hash));
            }
        }
    }
}
