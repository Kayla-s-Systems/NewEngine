const FIRST_PERSON_AIM_RESPONSE_HZ: f32 = 18.0;

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

pub(crate) fn equipped_weapon_aim_alpha(world: &newengine_ecs::World, owner: EntityId) -> f32 {
    world
        .query::<EquippedWeaponVisualRoot>()
        .find_map(|(_, visual)| (visual.owner == owner).then_some(visual.aim_alpha.clamp(0.0, 1.0)))
        .unwrap_or(0.0)
}

pub(crate) fn equipped_weapon_recoil_alpha(world: &newengine_ecs::World, owner: EntityId) -> f32 {
    world
        .query::<EquippedWeaponVisualRoot>()
        .find_map(|(_, visual)| {
            (visual.owner == owner).then_some(visual.recoil_alpha.clamp(0.0, 1.0))
        })
        .unwrap_or(0.0)
}

pub(crate) fn equipped_weapon_recoil_yaw_radians(
    world: &newengine_ecs::World,
    owner: EntityId,
) -> f32 {
    world
        .query::<EquippedWeaponVisualRoot>()
        .find_map(|(_, visual)| (visual.owner == owner).then_some(visual.recoil_yaw_radians))
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
}

/// Samples RMB/aim once before animation so body IK and rendered rifle consume the exact same
/// presentation alpha in the same world-runtime frame.
pub(crate) fn tick_equipped_weapon_presentation_input(world: &mut newengine_ecs::World, dt: f32) {
    let roots = world
        .query::<EquippedWeaponVisualRoot>()
        .map(|(entity, visual)| (entity, *visual))
        .collect::<Vec<_>>();
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    for (root, visual) in roots {
        // RMB is a weapon state, not a first-person-only state. Third-person aim must drive the
        // same ReadyHold/ADS contract as full-body first person.
        let obstruction_alpha = world
            .get::<WeaponObstructionState>(visual.owner)
            .map(|state| state.alpha.clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let aim_target = if first_person_aim_held(world, visual.owner) {
            // Aim-blocked keeps the intention to aim, but physically relaxes the weapon out of
            // full ADS as the barrel approaches geometry. This mirrors the original add/sub layer.
            (1.0 - obstruction_alpha * 0.82).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let aim_alpha = smooth_first_person_aim_alpha(visual.aim_alpha, aim_target, dt);
        let shot_sequence = world
            .get::<PlayerWeaponState>(visual.owner)
            .map(|state| state.shot_sequence)
            .unwrap_or(visual.last_shot_sequence);
        let new_shot = shot_sequence != visual.last_shot_sequence;
        let recoil_recovery_hz = world
            .resource::<ItemCatalog>()
            .and_then(|catalog| catalog.get(visual.item))
            .map(|definition| definition.weapon_presentation.clone().sanitized())
            .filter(|presentation| presentation.enabled)
            .map(|presentation| 3.0 / presentation.fire_kick_duration_seconds.max(0.001))
            .unwrap_or(18.0);
        let recoil_alpha = if new_shot {
            1.0
        } else if dt > 0.0 {
            (visual.recoil_alpha * (-recoil_recovery_hz * dt).exp()).clamp(0.0, 1.0)
        } else {
            visual.recoil_alpha
        };
        let recoil_yaw_radians = if new_shot {
            let tuning = world
                .get::<HitscanWeaponTuning>(visual.owner)
                .copied()
                .unwrap_or_default()
                .sanitized();
            let sign = if shot_sequence.is_multiple_of(2) { 1.0 } else { -1.0 };
            let noise = ((newengine_math::avalanche_u64(shot_sequence ^ 0xa409_3822) >> 40)
                as u32) as f32
                / 0x00ff_ffffu32 as f32;
            tuning.recoil_yaw_radians * sign * (0.55 + noise.clamp(0.0, 1.0) * 0.45)
        } else if dt > 0.0 {
            visual.recoil_yaw_radians * (-recoil_recovery_hz * dt).exp()
        } else {
            visual.recoil_yaw_radians
        };
        if let Some(state) = world.get_mut::<EquippedWeaponVisualRoot>(root) {
            state.aim_alpha = aim_alpha;
            state.last_shot_sequence = shot_sequence;
            state.recoil_alpha = recoil_alpha;
            state.recoil_yaw_radians = recoil_yaw_radians;
        }
    }
}

fn first_person_weapon_local_transform(
    world: &newengine_ecs::World,
    owner: EntityId,
    visual_parent: EntityId,
    presentation: &newengine_engine_runtime::gameplay::WeaponPresentationDefinition,
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

    let desired = crate::weapon_grip::weapon_root_from_first_person_view(
        presentation,
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
    // Character presentation/policy changes can rebuild the avatar visual root without replacing
    // the inventory weapon instance. Keep the equipped weapon attached to the current avatar, not
    // to a stale/despawned root from the previous model binding.
    if let Some(avatar_root) = world
        .get::<PlayerModelBinding>(owner)
        .and_then(|binding| binding.visual_root)
        .filter(|entity| world.exists(*entity))
    {
        let _ = set_parent(world, root, Some(avatar_root));
    }
    let weapon_definition = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(visual.item))
        .cloned();
    let presentation = weapon_definition
        .as_ref()
        .map(|definition| definition.weapon_presentation.clone().sanitized())
        .filter(|presentation| presentation.enabled);
    let authored_weapon_presentation = presentation.is_some();
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
    let obstruction_alpha = world
        .get::<WeaponObstructionState>(owner)
        .map(|state| state.alpha.clamp(0.0, 1.0))
        .unwrap_or(0.0);

    let mut right_frame_for_debug = None;
    let mut ready_body_frames_for_debug = None;
    let resolved = if authored_weapon_presentation && legacy_viewmodel_active {
        // Explicit legacy hidden-body mode keeps the old camera-owned viewmodel path. Full-body
        // first person must never enter this branch because visible hands/body need one shared
        // shoulder-owned rifle transform.
        let visual_parent = world
            .get::<PlayerModelBinding>(owner)
            .and_then(|binding| binding.visual_root)
            .filter(|entity| world.exists(*entity));
        visual_parent.and_then(|visual_parent| {
            first_person_weapon_local_transform(
                world,
                owner,
                visual_parent,
                presentation.as_ref().expect("authored presentation"),
                aim_alpha,
            )
        })
    } else if authored_weapon_presentation {
        // Authored ReadyHold: the native firing-hand contact owns handle translation, while the
        // anatomical solve owns base/aim rotation. Support-hand IK consumes this exact same root.
        let body_frames = super::player_model::player_rifle_ready_body_frames(world, owner);
        // Prefer the authored firing-hand prop contact when it remains physically close to the
        // palm. The helper falls back to the anatomical palm/wrist for rigs/clips without a valid
        // prop target. This keeps the rendered weapon attached to the same hand pose the player sees.
        let right_frame = super::player_model::player_right_hand_prop_frame(world, owner);
        let left_frame = super::player_model::player_left_hand_weapon_frame(world, owner);
        let view_forward_model = if first_person_active || aim_alpha > 0.001 {
            super::player_model::player_rifle_view_forward_model(world, owner)
        } else {
            None
        };
        let recoil_alpha = visual.recoil_alpha.clamp(0.0, 1.0);
        let recoil_yaw_radians = visual.recoil_yaw_radians;
        ready_body_frames_for_debug = body_frames;
        right_frame_for_debug = right_frame;
        body_frames.and_then(|(chest, right_shoulder, left_shoulder)| {
            let presentation = presentation.as_ref().expect("authored presentation");
            let handle_anchor = right_frame
                .map(|frame| frame.transform_point3(Vec3::ZERO))
                .filter(|position| position.is_finite());
            let support_anchor = left_frame
                .map(|frame| frame.transform_point3(Vec3::ZERO))
                .filter(|position| position.is_finite());
            crate::weapon_grip::weapon_ready_solve_contract_presented(
                presentation,
                chest,
                right_shoulder,
                left_shoulder,
                view_forward_model,
                aim_alpha,
                recoil_alpha,
                recoil_yaw_radians,
            )
            .and_then(|contract| {
                crate::weapon_grip::weapon_ready_contract_with_contacts(
                    presentation,
                    contract,
                    handle_anchor,
                    support_anchor,
                    aim_alpha,
                    obstruction_alpha,
                )
            })
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
        let (muzzle_position, muzzle_forward) = if let Some(presentation) = presentation.as_ref() {
            let weapon_root = crate::weapon_grip::WeaponRootTransform {
                position: weapon_position,
                rotation: weapon_rotation,
            };
            (
                crate::weapon_grip::weapon_muzzle_position(presentation, weapon_root),
                crate::weapon_grip::weapon_muzzle_forward(weapon_root),
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

    if authored_weapon_presentation
        && !legacy_viewmodel_active
        && !visual.grip_debug_emitted
        && crate::env_config::var_os("NORTHSTAR_DEBUG_WEAPON_GRIP").is_some()
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
        let left_palm = super::player_model::player_left_hand_weapon_frame(world, owner)
            .map(|frame| frame.transform_point3(Vec3::ZERO))
            .filter(|position| position.is_finite());
        let chest = chest_frame.transform_point3(Vec3::ZERO);
        let presentation = presentation.as_ref().expect("authored presentation");
        let Some(contract) = crate::weapon_grip::weapon_ready_solve_contract(
            presentation,
            chest_frame,
            right_shoulder_frame,
            left_shoulder_frame,
        )
        .and_then(|contract| {
            crate::weapon_grip::weapon_ready_contract_with_contacts(
                presentation,
                contract,
                Some(right_palm),
                left_palm,
                aim_alpha,
                obstruction_alpha,
            )
        }) else {
            return;
        };
        let weapon_root = contract.root;
        let handle = crate::weapon_grip::weapon_handle_position(presentation, weapon_root);
        let left_grip =
            crate::weapon_grip::weapon_ready_left_grip_position(presentation, weapon_root);
        let right_target =
            crate::weapon_grip::weapon_ready_right_palm_position(presentation, weapon_root);
        let left_target =
            crate::weapon_grip::weapon_ready_left_palm_position(presentation, weapon_root);
        if let Some(left_frame) = super::player_model::player_left_hand_weapon_frame(world, owner) {
            let left_palm = left_frame.transform_point3(Vec3::ZERO);
            let right_error = (right_palm - right_target).length();
            let left_error = (left_palm - left_target).length();
            newengine_ulog_api::ulog::info!(
                "WEAPON_GRIP player={} space='player_model' chest={:?} right_palm={:?} right_target={:?} right_error_m={:.5} handle={:?} stock={:?} shoulder_pocket={:?} stock_error_m={:.5} left_palm={:?} left_target={:?} left_error_m={:.5} l_grip={:?} policy='authored firing-hand master; soft stock/support constraints; bounded support IK; aim-blocked barrel pivot'",
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
