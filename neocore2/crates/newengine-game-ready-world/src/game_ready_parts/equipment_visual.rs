use super::foliage::{decode_runtime_ydd_prefab, DecodedPrefabMeshPart};
use super::*;

use newengine_engine_runtime::gameplay::{
    CharacterBody, DisplayMode, DisplayVisibility, EquippedWeaponBinding, EquippedWeaponMuzzle,
    HitscanWeaponTuning, ItemCatalog, PlayerCommandFrame, PlayerModelAssignment,
    PlayerModelBinding, PlayerStanceState, PlayerViewState, PlayerViewVisibility,
    PlayerViewVisibilityPolicy, PlayerVisualKind, PlayerVisualPart, PlayerWeaponState,
};

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

fn validate_canonical_rifle_visual_space(min: Vec3, max: Vec3) -> Result<(), String> {
    let center = (min + max) * 0.5;
    let extent = max - min;
    // `rifle.gltf` is authored in handle-centered weapon space: +X right, +Y up, +Z muzzle.
    // The previous runtime YDD was stale crowd/character-space geometry with center.y ~= 1.086 m.
    // Reject that artifact class instead of silently drawing the weapon above the character.
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

fn weapon_visual_alignment(
    model_ref: &str,
    decoded: &[DecodedPrefabMeshPart],
) -> Result<WeaponVisualAlignment, String> {
    let (min, max) = decoded_model_bounds(decoded)?;
    if model_ref.eq_ignore_ascii_case(crate::weapon_grip::RIFLE_MODEL_REF) {
        validate_canonical_rifle_visual_space(min, max)?;
        // Geometry produced by `export_rifle_source_gltf.py` already subtracts the recovered
        // crowd grip pivot before writing POSITION. Therefore mesh origin == pistol grip/handle.
        // `RIFLE_HANDLE_LOCAL` belongs to the authored skeleton hierarchy and must not be applied
        // a second time to the visual geometry.
        return Ok(WeaponVisualAlignment {
            grip_pivot: Vec3::ZERO,
        });
    }
    Ok(WeaponVisualAlignment {
        grip_pivot: (min + max) * 0.5,
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
        let needs_update = world
            .get::<newengine_model_domain_api::MeshRenderOptions>(entity)
            .map(|current| current != &desired)
            .unwrap_or(true);
        if needs_update {
            let _ = world.insert(entity, desired.clone());
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
    // Resolve every authored material before admitting the visual. A temporary materials-service
    // gap must defer the whole weapon instead of freezing one or more parts on diagnostic black.
    let material_ids = decoded
        .iter()
        .enumerate()
        .map(|(part_index, part)| {
            register_equipped_part_material(
                mats,
                &definition.name,
                part_index,
                part,
                world_definition.material_library_ref.as_deref(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let root = spawn_named(world, format!("Player/EquippedWeapon/{}", definition.name));
    let _ = world.insert(root, Transform::default());
    let last_shot_sequence = world
        .get::<PlayerWeaponState>(owner)
        .map(|state| state.shot_sequence)
        .unwrap_or(0);
    let _ = world.insert(
        root,
        EquippedWeaponVisualRoot {
            owner,
            instance_id: binding.instance_id,
            item: binding.item,
            grip_debug_emitted: false,
            aim_alpha: 0.0,
            last_shot_sequence,
            recoil_alpha: 0.0,
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
        let material_id = material_ids[part_index];
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
                // Translate the authored grip to the visual root. Translation must include
                // authored scale because local points are transformed as T * S * p.
                position: Vec3::new(
                    -alignment.grip_pivot.x * authored_scale.x,
                    -alignment.grip_pivot.y * authored_scale.y,
                    -alignment.grip_pivot.z * authored_scale.z,
                ),
                scale: authored_scale,
                color: [1.0, 1.0, 1.0, 1.0],
                render_options: equipped_weapon_render_options(),
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
        "game-ready: equipped weapon visual bound player={} item='{}' instance={} model='{}' parts={} attachment='readyhold-spined/bilateral-hand-ik' alignment='calibrated-palm-contacts'",
        owner.stable_u64(),
        definition.name,
        binding.instance_id.0,
        model_ref,
        spawned,
    );
    Ok(root)
}

const FIRST_PERSON_AIM_RESPONSE_HZ: f32 = 18.0;
const RIFLE_RECOIL_RECOVERY_HZ: f32 =
    3.0 / crate::weapon_grip::RIFLE_FIRE_KICK_DURATION_SECONDS;

#[inline]
fn first_person_aim_held(world: &newengine_ecs::World, owner: EntityId) -> bool {
    // Read the render-frame command transport first. WorldRuntime presentation runs before the
    // fixed gameplay step, so PlayerWeaponState::aiming can legitimately be one simulation tick
    // behind RMB. The command frame is the current input sample and makes ADS immediate.
    world
        .get::<PlayerCommandFrame>(owner)
        .is_some_and(|commands| {
            commands
                .actions
                .is_held(newengine_gameplay_fps_api::action::PLAYER_AIM)
        })
        || world
            .get::<PlayerWeaponState>(owner)
            .is_some_and(|state| state.aiming)
}

#[inline]
fn smooth_first_person_aim_alpha(current: f32, target: f32, dt: f32) -> f32 {
    let current = if current.is_finite() {
        current.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let target = target.clamp(0.0, 1.0);
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    if dt <= 0.0 {
        return target;
    }
    let alpha = 1.0 - (-FIRST_PERSON_AIM_RESPONSE_HZ * dt).exp();
    (current + (target - current) * alpha).clamp(0.0, 1.0)
}

pub(crate) fn equipped_rifle_aim_alpha(world: &newengine_ecs::World, owner: EntityId) -> f32 {
    world
        .query::<EquippedWeaponVisualRoot>()
        .find_map(|(_, visual)| (visual.owner == owner).then_some(visual.aim_alpha.clamp(0.0, 1.0)))
        .unwrap_or(0.0)
}

pub(crate) fn equipped_rifle_recoil_alpha(world: &newengine_ecs::World, owner: EntityId) -> f32 {
    world
        .query::<EquippedWeaponVisualRoot>()
        .find_map(|(_, visual)| {
            (visual.owner == owner).then_some(visual.recoil_alpha.clamp(0.0, 1.0))
        })
        .unwrap_or(0.0)
}

/// Samples RMB/aim once before animation so body IK and rendered rifle consume the exact same
/// presentation alpha in the same world-runtime frame.
pub(crate) fn tick_equipped_weapon_presentation_input(world: &mut newengine_ecs::World, dt: f32) {
    let roots = world
        .query::<EquippedWeaponVisualRoot>()
        .map(|(entity, visual)| (entity, *visual))
        .collect::<Vec<_>>();
    let dt = if dt.is_finite() && dt > 0.0 { dt.min(0.1) } else { 0.0 };
    for (root, visual) in roots {
        // RMB is a weapon state, not a first-person-only state. Third-person aim must drive the
        // same ReadyHold/ADS contract as full-body first person.
        let aim_target = if first_person_aim_held(world, visual.owner) { 1.0 } else { 0.0 };
        let aim_alpha = smooth_first_person_aim_alpha(visual.aim_alpha, aim_target, dt);
        let shot_sequence = world
            .get::<PlayerWeaponState>(visual.owner)
            .map(|state| state.shot_sequence)
            .unwrap_or(visual.last_shot_sequence);
        let new_shot = shot_sequence != visual.last_shot_sequence;
        let recoil_alpha = if new_shot {
            1.0
        } else if dt > 0.0 {
            (visual.recoil_alpha * (-RIFLE_RECOIL_RECOVERY_HZ * dt).exp()).clamp(0.0, 1.0)
        } else {
            visual.recoil_alpha
        };
        if let Some(state) = world.get_mut::<EquippedWeaponVisualRoot>(root) {
            state.aim_alpha = aim_alpha;
            state.last_shot_sequence = shot_sequence;
            state.recoil_alpha = recoil_alpha;
        }
    }
}

fn first_person_rifle_local_transform(
    world: &newengine_ecs::World,
    owner: EntityId,
    visual_parent: EntityId,
    aim_alpha: f32,
) -> Option<(Vec3, Quat)> {
    let (player_position, player_body_rotation) =
        newengine_transform::read_entity_world_pose_local_chain(world, owner)?;
    let eye_height = world
        .get::<PlayerStanceState>(owner)
        .map(|state| state.current_eye_height)
        .or_else(|| {
            world
                .get::<CharacterBody>(owner)
                .map(|body| body.standing_eye_height)
        })
        .unwrap_or(1.6)
        .max(0.01);

    // A first-person weapon must consume the same view owner as the renderer. CharacterMotor is
    // normally authoritative and is updated at input/render cadence, but scripted/runtime camera
    // paths can move the camera without mutating the motor. In that case the previous resolved
    // CameraRig is a one-render-frame fallback instead of freezing the rifle on body facing.
    let active_camera = world
        .resource::<newengine_scene::SceneState>()
        .and_then(|state| state.active_camera.or(state.root));
    let resolved_camera_rig = active_camera
        .and_then(|camera| world.get::<newengine_sim::CameraRigComp>(camera))
        .map(|rig| rig.0)
        .filter(|rig| rig.position.is_finite() && rig.rotation.is_finite());
    let camera_rot_offset = active_camera
        .and_then(|camera| world.get::<newengine_sim::FollowTargetCameraController>(camera))
        .filter(|controller| controller.target == owner)
        .map(|controller| controller.rot_offset)
        .unwrap_or(Quat::IDENTITY)
        .normalize_or_identity();

    let view_rotation = world
        .get::<newengine_sim::CharacterMotor>(owner)
        .map(|motor| {
            (Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0) * camera_rot_offset)
                .normalize_or_identity()
        })
        .or_else(|| resolved_camera_rig.map(|rig| rig.rotation.normalize_or_identity()))
        .unwrap_or(player_body_rotation.normalize_or_identity());
    let camera_position = resolved_camera_rig
        .map(|rig| rig.position)
        .unwrap_or(player_position + Vec3::Y * eye_height);

    let desired = crate::weapon_grip::rifle_root_from_first_person_view(
        camera_position,
        view_rotation,
        aim_alpha,
    )?;

    // EquippedWeapon root remains parented under the avatar visual root so third-person can keep
    // using authored hand-local sockets. Convert the desired camera/world pose back into that
    // parent's local space instead of reparenting every frame.
    let (parent_position, parent_rotation) =
        newengine_transform::read_entity_world_pose_local_chain(world, visual_parent)?;
    let parent_rotation = parent_rotation.normalize_or_identity();
    let parent_inverse = parent_rotation.inverse();
    let local_position = parent_inverse * (desired.position - parent_position);
    let local_rotation = (parent_inverse * desired.rotation).normalize_or_identity();
    (local_position.is_finite() && local_rotation.is_finite())
        .then_some((local_position, local_rotation))
}

fn update_weapon_attachment(
    world: &mut newengine_ecs::World,
    owner: EntityId,
    root: EntityId,
    _dt: f32,
) {
    let Some(visual) = world.get::<EquippedWeaponVisualRoot>(root).copied() else {
        return;
    };
    let model_ref = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(visual.item))
        .and_then(|definition| definition.world.model_ref.as_deref())
        .unwrap_or_default();
    let is_rifle = model_ref.eq_ignore_ascii_case(crate::weapon_grip::RIFLE_MODEL_REF);
    let first_person_active = world
        .resource::<PlayerViewState>()
        .copied()
        .unwrap_or_default()
        .first_person_active;
    let legacy_viewmodel_active = first_person_active
        && world
            .get::<PlayerModelAssignment>(owner)
            .is_some_and(|assignment| assignment.hide_in_first_person);
    sync_equipped_weapon_render_policy(world, root, legacy_viewmodel_active);
    let aim_alpha = visual.aim_alpha.clamp(0.0, 1.0);

    let mut right_frame_for_debug = None;
    let mut ready_body_frames_for_debug = None;
    let resolved = if is_rifle && legacy_viewmodel_active {
        // Explicit legacy hidden-body mode keeps the old camera-owned viewmodel path. Full-body
        // first person must never enter this branch because visible hands/body need one shared
        // shoulder-owned rifle transform.
        let visual_parent = world
            .get::<PlayerModelBinding>(owner)
            .and_then(|binding| binding.visual_root)
            .filter(|entity| world.exists(*entity));
        visual_parent.and_then(|visual_parent| {
            first_person_rifle_local_transform(world, owner, visual_parent, aim_alpha)
        })
    } else if is_rifle {
        // Full-body ReadyHold: stock owns translation, view direction owns aiming rotation, and
        // both arms solve against this exact root in player_model_animation.
        let body_frames = super::player_model::player_rifle_ready_body_frames(world, owner);
        let right_frame = super::player_model::player_right_hand_weapon_frame(world, owner);
        let view_forward_model = if first_person_active || aim_alpha > 0.001 {
            super::player_model::player_rifle_view_forward_model(world, owner)
        } else {
            None
        };
        let recoil_alpha = visual.recoil_alpha.clamp(0.0, 1.0);
        ready_body_frames_for_debug = body_frames;
        right_frame_for_debug = right_frame;
        body_frames.and_then(|(chest, right_shoulder, left_shoulder)| {
            crate::weapon_grip::rifle_ready_solve_contract_presented(
                chest,
                right_shoulder,
                left_shoulder,
                view_forward_model,
                aim_alpha,
                recoil_alpha,
            )
            .map(|contract| (contract.root.position, contract.root.rotation))
        })
    } else {
        let right_frame = super::player_model::player_right_hand_prop_frame(world, owner);
        right_frame.and_then(|right_frame| {
            let (scale, rotation, translation) = right_frame.to_scale_rotation_translation();
            (translation.is_finite()
                && rotation.is_finite()
                && scale.is_finite()
                && scale.x > 0.0
                && scale.y > 0.0
                && scale.z > 0.0)
                .then_some((translation, rotation))
        })
    };
    let Some((position, rotation)) = resolved else {
        return;
    };

    if let Some(transform) = world.get_mut::<Transform>(root) {
        transform.position = position;
        transform.rotation = rotation;
        // Weapon scale is authored on mesh children. Skeleton scale must not multiply it.
        transform.scale = Vec3::ONE;
    }

    // Publish the exact barrel pose used by the rendered weapon. Combat/audio/VFX consume this
    // instead of reconstructing a second approximate muzzle from the camera.
    if let Some((weapon_position, weapon_rotation)) =
        newengine_transform::read_entity_world_pose_local_chain(world, root)
    {
        let weapon_rotation = weapon_rotation.normalize_or_identity();
        let (muzzle_position, muzzle_forward) = if is_rifle {
            let rifle_root = crate::weapon_grip::RifleRootTransform {
                position: weapon_position,
                rotation: weapon_rotation,
            };
            (
                crate::weapon_grip::rifle_muzzle_position(rifle_root),
                crate::weapon_grip::rifle_muzzle_forward(rifle_root),
            )
        } else {
            let forward = (weapon_rotation * Vec3::Z).normalize_or_zero();
            let offset = world
                .get::<HitscanWeaponTuning>(owner)
                .map(|tuning| tuning.sanitized().muzzle_forward_offset)
                .unwrap_or(0.52);
            (weapon_position + forward * offset, forward)
        };
        if let Some(muzzle) = EquippedWeaponMuzzle::new(muzzle_position, muzzle_forward) {
            let _ = world.insert(owner, muzzle);
        } else {
            let _ = world.remove::<EquippedWeaponMuzzle>(owner);
        }
    }

    if is_rifle
        && !legacy_viewmodel_active
        && !visual.grip_debug_emitted
        && std::env::var_os("NORTHSTAR_DEBUG_WEAPON_GRIP").is_some()
    {
        let Some(right_frame) = right_frame_for_debug else {
            return;
        };
        let Some((chest_frame, right_shoulder_frame, left_shoulder_frame)) =
            ready_body_frames_for_debug
        else {
            return;
        };
        let right_palm = right_frame.transform_point3(Vec3::ZERO);
        let chest = chest_frame.transform_point3(Vec3::ZERO);
        let Some(contract) = crate::weapon_grip::rifle_ready_solve_contract(
            chest_frame,
            right_shoulder_frame,
            left_shoulder_frame,
        ) else {
            return;
        };
        let rifle_root = contract.root;
        let handle = crate::weapon_grip::rifle_handle_position(rifle_root);
        let left_grip = crate::weapon_grip::rifle_ready_left_grip_position(rifle_root);
        let right_target = crate::weapon_grip::rifle_ready_right_palm_position(rifle_root);
        let left_target = crate::weapon_grip::rifle_ready_left_palm_position(rifle_root);
        if let Some(left_frame) = super::player_model::player_left_hand_weapon_frame(world, owner) {
            let left_palm = left_frame.transform_point3(Vec3::ZERO);
            let right_error = (right_palm - right_target).length();
            let left_error = (left_palm - left_target).length();
            newengine_ulog_api::ulog::info!(
                "WEAPON_GRIP player={} space='player_model' chest={:?} right_palm={:?} right_target={:?} right_error_m={:.5} handle={:?} stock={:?} shoulder_pocket={:?} stock_error_m={:.5} left_palm={:?} left_target={:?} left_error_m={:.5} l_grip={:?} policy='ReadyHold stock->shoulder anchor; pole-vector bilateral IK; constrained wrist basis'",
                owner.stable_u64(),
                chest,
                right_palm,
                right_target,
                right_error,
                handle,
                contract.stock_contact,
                contract.shoulder_pocket,
                (contract.stock_contact - contract.shoulder_pocket).length(),
                left_palm,
                left_target,
                left_error,
                left_grip,
            );
            if let Some(state) = world.get_mut::<EquippedWeaponVisualRoot>(root) {
                state.grip_debug_emitted = true;
            }
        }
    }
}

pub(crate) fn tick_equipped_weapon_visuals(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    dt: f32,
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
                update_weapon_attachment(world, owner, root, dt);
            }
            (Some(binding), existing) => {
                if existing.is_some() {
                    clear_equipped_weapon_visual(world, owner);
                }
                match spawn_equipped_weapon_visual(world, prims, mats, owner, binding) {
                    Ok(root) => update_weapon_attachment(world, owner, root, dt),
                    Err(error) => {
                        // Avatar/model admission can lag inventory by a few frames during startup;
                        // retry quietly until both are resident. Non-transient faults remain visible
                        // through the normal asset/material diagnostics.
                        if world
                            .get::<PlayerModelBinding>(owner)
                            .and_then(|binding| binding.visual_root)
                            .is_some()
                        {
                            let tick = world.tick();
                            if tick <= 4 || tick.is_multiple_of(120) {
                                newengine_ulog_api::ulog::warn!(
                                    "game-ready: equipped weapon visual deferred player={} item={:016x} tick={}: {}",
                                    owner.stable_u64(),
                                    binding.item.raw(),
                                    tick,
                                    error,
                                );
                            }
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
    fn canonical_rifle_visual_space_accepts_handle_centered_source() {
        let min = Vec3::new(-0.069_917_45, -0.065_805_55, -0.372_692_38);
        let max = Vec3::new(0.120_714_34, 0.127_575_71, 0.633_752_35);
        validate_canonical_rifle_visual_space(min, max).expect("canonical rifle space");
    }

    #[test]
    fn stale_crowd_space_rifle_is_rejected() {
        let min = Vec3::new(0.051_332_55, 0.582_858_7, -0.221_605_55);
        let max = Vec3::new(0.241_964_34, 1.589_303_4, -0.028_224_29);
        assert!(validate_canonical_rifle_visual_space(min, max).is_err());
    }
    #[test]
    fn equipped_weapon_is_explicit_cast_and_receive_world_opaque() {
        let options = equipped_weapon_render_options();
        assert_eq!(
            options.role,
            newengine_model_domain_api::MeshRenderRole::WorldOpaque
        );
        assert_eq!(
            options.shadow_policy,
            newengine_model_domain_api::MeshShadowPolicy::CastAndReceive
        );
        assert_eq!(
            options.depth_policy,
            newengine_model_domain_api::MeshDepthPolicy::ReadWrite
        );
    }

    #[test]
    fn rifle_recoil_recovery_is_fast_and_monotonic() {
        let mut value = 1.0_f32;
        for _ in 0..12 {
            let next = value * (-RIFLE_RECOIL_RECOVERY_HZ * (1.0 / 60.0)).exp();
            assert!(next >= 0.0 && next < value);
            value = next;
        }
        assert!(value < 0.05);
    }

    #[test]
    fn first_person_aim_alpha_converges_without_overshoot() {
        let mut value = 0.0;
        for _ in 0..30 {
            value = smooth_first_person_aim_alpha(value, 1.0, 1.0 / 60.0);
            assert!((0.0..=1.0).contains(&value));
        }
        assert!(value > 0.99);
        let released = smooth_first_person_aim_alpha(value, 0.0, 1.0 / 60.0);
        assert!(released < value);
    }

    #[test]
    fn first_person_aim_reads_current_render_frame_command_before_fixed_step() {
        let mut world = newengine_ecs::World::new();
        let owner = world.spawn();
        let mut commands = PlayerCommandFrame::default();
        commands
            .actions
            .held
            .push(newengine_gameplay_fps_api::action::PLAYER_AIM.to_owned());
        let _ = world.insert(owner, commands);
        assert!(first_person_aim_held(&world, owner));
    }

    #[test]
    fn equipped_rifle_material_library_resolves_mesh_slots_to_nemat_entries() {
        assert_eq!(
            equipped_part_material_asset(None, "m00", Some("shared/materials/weapon_rifle.nemat")),
            Some("shared/materials/weapon_rifle.nemat@m00".to_owned())
        );
        assert_eq!(
            equipped_part_material_asset(
                Some("shared/materials/weapon_rifle.nemat"),
                "m01",
                Some("ignored.nemat"),
            ),
            Some("shared/materials/weapon_rifle.nemat@m01".to_owned())
        );
    }
}
