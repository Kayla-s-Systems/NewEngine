use super::*;

use newengine_animation_runtime::{AnimationSkeletonRuntime, JointLocalPose};
use newengine_math::{Mat4, Quat, Vec3};
use newengine_model_skeleton_api::ModelSkeletonMetadata;

#[derive(Clone, Debug)]
pub(super) struct PreparedPlayerSkinSidecar {
    source: String,
    parts: Vec<PlayerRuntimeModelPart>,
    main_runtime: AnimationSkeletonRuntime,
    sidecar_runtime: AnimationSkeletonRuntime,
    sidecar_parent_indices: Vec<Option<usize>>,
    sidecar_evaluation_order: Vec<usize>,
    mapped_master_joints: Vec<Option<usize>>,
    sidecar_model_to_source: Mat4,
    initial_palette: Vec<Mat4>,
    mapped_count: usize,
    local_count: usize,
}

#[derive(Clone, Debug)]
struct PlayerSkinSidecarRuntimeBinding {
    palette_owner: EntityId,
    main_runtime: AnimationSkeletonRuntime,
    sidecar_runtime: AnimationSkeletonRuntime,
    sidecar_parent_indices: Vec<Option<usize>>,
    sidecar_evaluation_order: Vec<usize>,
    mapped_master_joints: Vec<Option<usize>>,
    sidecar_model_to_source: Mat4,
    local_pose: Vec<JointLocalPose>,
    source_globals: Vec<Mat4>,
    palette_scratch: Vec<Mat4>,
}

fn uniform_skin_source_to_model(
    parts: &[PlayerRuntimeModelPart],
    role: &str,
) -> Result<[f32; 16], String> {
    let mut authored = None::<[f32; 16]>;
    for part in parts {
        let Some(skin) = part.skin.as_ref() else {
            continue;
        };
        match authored {
            None => authored = Some(skin.source_to_model),
            Some(expected) if expected == skin.source_to_model => {}
            Some(_) => {
                return Err(format!(
                    "player skin sidecar {role} contains mixed source_to_model transforms mesh='{}'",
                    part.source_mesh_name
                ));
            }
        }
    }
    authored.ok_or_else(|| format!("player skin sidecar {role} has no authored skin stream"))
}

fn topological_joint_order(skeleton: &ModelSkeletonMetadata) -> Result<Vec<usize>, String> {
    let count = skeleton.joints.len();
    let mut resolved = vec![false; count];
    let mut order = Vec::with_capacity(count);
    while order.len() < count {
        let mut progress = false;
        for (index, joint) in skeleton.joints.iter().enumerate() {
            if resolved[index] {
                continue;
            }
            let parent = joint.parent_index.map(|value| value as usize);
            if parent.is_some_and(|parent| parent >= count) {
                return Err(format!(
                    "player skin sidecar parent outside skeleton joint={} parent={} joints={count}",
                    index,
                    parent.unwrap_or_default()
                ));
            }
            if parent.is_none() || parent.is_some_and(|parent| resolved[parent]) {
                resolved[index] = true;
                order.push(index);
                progress = true;
            }
        }
        if !progress {
            return Err("player skin sidecar skeleton hierarchy contains a cycle".to_owned());
        }
    }
    Ok(order)
}

pub(super) fn prepare_player_skin_sidecar(
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    main_parts: &[PlayerRuntimeModelPart],
    main_skeleton: Option<&ModelSkeletonMetadata>,
) -> Result<Option<PreparedPlayerSkinSidecar>, String> {
    let Some(definition) = assignment.presentation.skin_sidecar.as_ref() else {
        return Ok(None);
    };
    let suffix = definition.joint_name_suffix.trim();
    let local_prefix = definition.local_joint_prefix.trim();
    if suffix.is_empty() || local_prefix.is_empty() {
        return Err(
            "authored player skin sidecar requires non-empty joint suffix and local-joint prefix"
                .to_owned(),
        );
    }
    let main_skeleton = main_skeleton.ok_or_else(|| {
        "authored player skin sidecar requires the main player skeleton metadata".to_owned()
    })?;
    let main_source_to_model = uniform_skin_source_to_model(main_parts, "master")?;
    let main_runtime = AnimationSkeletonRuntime::compile(main_skeleton, main_source_to_model)?;

    let (source, parts, sidecar_skeleton) =
        super::assets::ensure_player_runtime_sidecar_parts(prims, mats, assignment, definition)?;
    let sidecar_skeleton = sidecar_skeleton
        .ok_or_else(|| format!("authored player skin sidecar has no skeleton source='{source}'"))?;
    if parts.is_empty() || parts.iter().any(|part| part.skin.is_none()) {
        return Err(format!(
            "authored player skin sidecar must contain only skinned geometry source='{source}' parts={}",
            parts.len()
        ));
    }
    let sidecar_source_to_model = uniform_skin_source_to_model(&parts, "auxiliary")?;
    let sidecar_runtime =
        AnimationSkeletonRuntime::compile(&sidecar_skeleton, sidecar_source_to_model)?;

    let main_by_name = main_skeleton
        .joints
        .iter()
        .enumerate()
        .map(|(index, joint)| (joint.name.as_str(), index))
        .collect::<std::collections::HashMap<_, _>>();
    let mut mapped_master_joints = Vec::with_capacity(sidecar_skeleton.joints.len());
    let mut mapped_count = 0usize;
    let mut local_count = 0usize;
    for joint in &sidecar_skeleton.joints {
        let base_name = joint.name.strip_suffix(suffix).ok_or_else(|| {
            format!(
                "authored player skin sidecar joint is outside declared namespace joint='{}' suffix='{}'",
                joint.name, suffix
            )
        })?;
        if let Some(&master_index) = main_by_name.get(base_name) {
            mapped_master_joints.push(Some(master_index));
            mapped_count += 1;
        } else if base_name.starts_with(local_prefix) {
            mapped_master_joints.push(None);
            local_count += 1;
        } else {
            return Err(format!(
                "authored player skin sidecar joint has neither exact master identity nor declared local prefix joint='{}' base='{}' local_prefix='{}'",
                joint.name, base_name, local_prefix
            ));
        }
    }
    if mapped_count == 0 {
        return Err(format!(
            "authored player skin sidecar has no exact master joint mappings source='{source}'"
        ));
    }

    let sidecar_parent_indices = sidecar_skeleton
        .joints
        .iter()
        .map(|joint| joint.parent_index.map(|value| value as usize))
        .collect::<Vec<_>>();
    let sidecar_evaluation_order = topological_joint_order(&sidecar_skeleton)?;
    let mut initial_palette = Vec::new();
    sidecar_runtime
        .build_skin_palette_from_local_pose(sidecar_runtime.bind_locals(), &mut initial_palette)?;
    let sidecar_model_to_source = Mat4::from_cols_array(&sidecar_source_to_model).inverse();
    if sidecar_model_to_source
        .to_cols_array()
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err("authored player skin sidecar source_to_model is not invertible".to_owned());
    }

    newengine_ulog_api::ulog::info!(
        "fps-character: authored player skin sidecar prepared source='{}' joints={} mapped_master={} local_auxiliary={} suffix='{}' local_prefix='{}' policy='exact-name projection; no proximity reskin/fallback'",
        source,
        sidecar_skeleton.joints.len(),
        mapped_count,
        local_count,
        suffix,
        local_prefix,
    );

    Ok(Some(PreparedPlayerSkinSidecar {
        source,
        parts,
        main_runtime,
        sidecar_runtime,
        sidecar_parent_indices,
        sidecar_evaluation_order,
        mapped_master_joints,
        sidecar_model_to_source,
        initial_palette,
        mapped_count,
        local_count,
    }))
}

pub(super) fn bind_prepared_player_skin_sidecar(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    player: EntityId,
    visual_root: EntityId,
    visual_root_name: &str,
    main_part_count: usize,
    first_person_active: bool,
    hide_local_model_in_first_person: bool,
    prepared: PreparedPlayerSkinSidecar,
) -> Result<u32, String> {
    let palette_owner = spawn_named(world, format!("{visual_root_name}/SkinSidecarPalette"));
    let _ = world.insert(
        palette_owner,
        newengine_engine_runtime::gameplay::PlayerSkinPose {
            palette: prepared.initial_palette,
            revision: 1,
        },
    );

    let visibility_policy = if hide_local_model_in_first_person {
        newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::HideInFirstPerson
    } else {
        newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::AlwaysVisible
    };
    for (sidecar_index, part) in prepared.parts.iter().enumerate() {
        let entity = spawn_named(
            world,
            format!(
                "{visual_root_name}/SkinSidecar/Part{sidecar_index}:{}",
                part.material_slot
            ),
        );
        let _ = world.insert(entity, Transform::default());
        let _ = world.insert(
            entity,
            Primitive {
                id: part.primitive_id,
                color: part.color,
            },
        );
        if let Some(bounds) = primitive_bounds(prims, part.primitive_id) {
            let _ = world.insert(entity, bounds);
        }
        newengine_engine_runtime::gameplay::attach_scene_object_core(
            world,
            entity,
            Vec3::ZERO,
            Vec3::splat(0.25),
        );
        let _ = world.insert(entity, newengine_engine_runtime::gameplay::GameplayActor);
        let skin = part.skin.as_ref().ok_or_else(|| {
            format!(
                "authored player skin sidecar part lost skin stream mesh='{}'",
                part.source_mesh_name
            )
        })?;
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::PlayerSkinBinding {
                owner: palette_owner,
                vertices: skin
                    .vertices
                    .iter()
                    .map(
                        |vertex| newengine_engine_runtime::gameplay::PlayerSkinVertex {
                            joints: vertex.joints,
                            weights: vertex.weights,
                            joints_extra: vertex.joints_extra,
                            weights_extra: vertex.weights_extra,
                        },
                    )
                    .collect(),
                source_to_model: skin.source_to_model,
            },
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::PlayerVisualPart {
                owner: player,
                part_index: (main_part_count + sidecar_index) as u32,
                kind: newengine_engine_runtime::gameplay::PlayerVisualKind::RuntimeModelPart,
                material_slot: part.material_slot.clone(),
            },
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::PlayerViewVisibility {
                base_mode: newengine_engine_runtime::gameplay::DisplayMode::GameOnly,
                policy: visibility_policy,
            },
        );
        let initial_mode = if first_person_active
            && visibility_policy
                == newengine_engine_runtime::gameplay::PlayerViewVisibilityPolicy::HideInFirstPerson
        {
            newengine_engine_runtime::gameplay::DisplayMode::RuntimeHidden
        } else {
            newengine_engine_runtime::gameplay::DisplayMode::GameOnly
        };
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::DisplayVisibility { mode: initial_mode },
        );
        let _ = set_parent(world, entity, Some(visual_root));
        let _ = apply_exact_material(
            world,
            mats,
            entity,
            part.material_id,
            part.material_id,
            part.color,
        );
    }

    let part_count = prepared.parts.len() as u32;
    let joint_count = prepared.sidecar_runtime.joint_count();
    let binding = PlayerSkinSidecarRuntimeBinding {
        palette_owner,
        main_runtime: prepared.main_runtime,
        sidecar_runtime: prepared.sidecar_runtime,
        sidecar_parent_indices: prepared.sidecar_parent_indices,
        sidecar_evaluation_order: prepared.sidecar_evaluation_order,
        mapped_master_joints: prepared.mapped_master_joints,
        sidecar_model_to_source: prepared.sidecar_model_to_source,
        local_pose: Vec::with_capacity(joint_count),
        source_globals: vec![Mat4::IDENTITY; joint_count],
        palette_scratch: Vec::with_capacity(joint_count),
    };
    let _ = world.insert(player, binding);
    newengine_ulog_api::ulog::info!(
        "fps-character: authored player skin sidecar bound player={} source='{}' parts={} mapped_master={} local_auxiliary={} palette_owner={} policy='independent exact auxiliary palette'",
        player.stable_u64(),
        prepared.source,
        part_count,
        prepared.mapped_count,
        prepared.local_count,
        palette_owner.stable_u64(),
    );
    Ok(part_count)
}

pub(super) fn clear_player_skin_sidecar(world: &mut newengine_ecs::World, player: EntityId) {
    let palette_owner = world
        .get::<PlayerSkinSidecarRuntimeBinding>(player)
        .map(|binding| binding.palette_owner);
    let _ = world.remove::<PlayerSkinSidecarRuntimeBinding>(player);
    if let Some(owner) = palette_owner.filter(|owner| world.exists(*owner)) {
        let _ = world.despawn(owner);
    }
}

fn joint_local_matrix(local: JointLocalPose) -> Mat4 {
    let scale = local.scale.unwrap_or([1.0; 3]);
    Mat4::from_scale_rotation_translation(
        Vec3::new(scale[0], scale[1], scale[2]),
        Quat::from_xyzw(
            local.rotation[0],
            local.rotation[1],
            local.rotation[2],
            local.rotation[3],
        )
        .normalize_or_identity(),
        Vec3::new(
            local.translation[0],
            local.translation[1],
            local.translation[2],
        ),
    )
}

fn local_pose_from_matrix(matrix: Mat4, joint: usize) -> Result<JointLocalPose, String> {
    if matrix
        .to_cols_array()
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(format!(
            "skin sidecar local matrix is non-finite joint={joint}"
        ));
    }
    let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
    if !scale.is_finite()
        || scale.x.abs() <= 1.0e-8
        || scale.y.abs() <= 1.0e-8
        || scale.z.abs() <= 1.0e-8
        || !rotation.is_finite()
        || !translation.is_finite()
    {
        return Err(format!(
            "skin sidecar local matrix is not decomposable joint={joint} scale={scale:?}"
        ));
    }
    let rotation = rotation.normalize_or_identity();
    Ok(JointLocalPose {
        translation: [translation.x, translation.y, translation.z],
        rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
        scale: Some([scale.x, scale.y, scale.z]),
    })
}

fn update_sidecar_palette(
    binding: &mut PlayerSkinSidecarRuntimeBinding,
    main_pose: &newengine_engine_runtime::gameplay::PlayerSkinPose,
) -> Result<Vec<Mat4>, String> {
    if main_pose.palette.len() != binding.main_runtime.joint_count() {
        return Err(format!(
            "skin sidecar master palette mismatch palette={} skeleton={}",
            main_pose.palette.len(),
            binding.main_runtime.joint_count()
        ));
    }
    binding.local_pose.clear();
    binding
        .local_pose
        .extend_from_slice(binding.sidecar_runtime.bind_locals());
    if binding.source_globals.len() != binding.sidecar_runtime.joint_count() {
        binding
            .source_globals
            .resize(binding.sidecar_runtime.joint_count(), Mat4::IDENTITY);
    }

    for &joint in &binding.sidecar_evaluation_order {
        let parent = binding.sidecar_parent_indices[joint];
        if let Some(master_joint) = binding.mapped_master_joints[joint] {
            let current_master_model = main_pose.palette[master_joint]
                * binding.main_runtime.bind_joint_frames()[master_joint];
            let desired_source_global = binding.sidecar_model_to_source * current_master_model;
            let local_matrix = if let Some(parent) = parent {
                let parent_global = binding.source_globals[parent];
                let parent_inverse = parent_global.inverse();
                if parent_inverse
                    .to_cols_array()
                    .iter()
                    .any(|value| !value.is_finite())
                {
                    return Err(format!(
                        "skin sidecar parent global is singular joint={joint} parent={parent}"
                    ));
                }
                parent_inverse * desired_source_global
            } else {
                desired_source_global
            };
            binding.local_pose[joint] = local_pose_from_matrix(local_matrix, joint)?;
            binding.source_globals[joint] = desired_source_global;
        } else {
            let local_matrix = joint_local_matrix(binding.local_pose[joint]);
            binding.source_globals[joint] = parent
                .map(|parent| binding.source_globals[parent] * local_matrix)
                .unwrap_or(local_matrix);
        }
    }

    binding
        .sidecar_runtime
        .build_skin_palette_from_local_pose(&binding.local_pose, &mut binding.palette_scratch)?;
    Ok(std::mem::take(&mut binding.palette_scratch))
}

pub(crate) fn tick_player_skin_sidecars(world: &mut newengine_ecs::World) {
    let players = world
        .query::<PlayerSkinSidecarRuntimeBinding>()
        .map(|(player, _)| player)
        .collect::<Vec<_>>();
    for player in players {
        let Some(main_pose) = world
            .get::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
            .cloned()
        else {
            continue;
        };
        let update = {
            let Some(binding) = world.get_mut::<PlayerSkinSidecarRuntimeBinding>(player) else {
                continue;
            };
            let owner = binding.palette_owner;
            update_sidecar_palette(binding, &main_pose).map(|palette| (owner, palette))
        };
        let (owner, palette) = match update {
            Ok(value) => value,
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "fps-character: authored player skin sidecar palette rejected player={} err='{}' policy='retain_last_valid_palette'",
                    player.stable_u64(),
                    error
                );
                continue;
            }
        };
        if !world.exists(owner) {
            newengine_ulog_api::ulog::warn!(
                "fps-character: authored player skin sidecar palette owner disappeared player={} owner={}",
                player.stable_u64(),
                owner.stable_u64()
            );
            continue;
        }
        let recycled = if let Some(pose) =
            world.get_mut::<newengine_engine_runtime::gameplay::PlayerSkinPose>(owner)
        {
            let recycled = std::mem::replace(&mut pose.palette, palette);
            pose.revision = pose.revision.saturating_add(1).max(1);
            Some(recycled)
        } else {
            let _ = world.insert(
                owner,
                newengine_engine_runtime::gameplay::PlayerSkinPose {
                    palette,
                    revision: 1,
                },
            );
            None
        };
        if let Some(recycled) = recycled {
            if let Some(binding) = world.get_mut::<PlayerSkinSidecarRuntimeBinding>(player) {
                binding.palette_scratch = recycled;
            }
        }
    }
}
