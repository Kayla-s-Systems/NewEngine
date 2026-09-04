fn update_weapon_attachment(
    world: &mut newengine_ecs::World,
    owner: EntityId,
    root: EntityId,
    dt: f32,
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
        let already_parented = world
            .get::<newengine_transform::Parent>(root)
            .is_some_and(|parent| parent.0.stable_id == avatar_root.stable_u64());
        if !already_parented {
            let _ = set_parent(world, root, Some(avatar_root));
        }
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
    // The held item shares the same world-space skeleton/contact domain as the visible hands.
    // There is no separate camera-owned first-person viewmodel path: that path was the source of
    // transform disagreement between hands, weapon, muzzle and renderer.
    sync_equipped_weapon_render_policy(world, root);
    let aim_alpha = visual.aim_alpha.clamp(0.0, 1.0);
    let obstruction_alpha = world
        .get::<WeaponObstructionState>(owner)
        .map(|state| state.alpha.clamp(0.0, 1.0))
        .unwrap_or(0.0);

    let mut right_frame_for_debug = None;
    let mut ready_body_frames_for_debug = None;
    let authored_character_equipment_pose =
        super::player_model::player_has_authored_equipment_pose(world, owner);
    let mut weapon_root_source_for_debug = "hand_prop_fallback";
    let resolved = if authored_weapon_presentation {
        // A character-authored equipment pose owns the weapon attachment contract. Torso-owned
        // ReadyHold is compatibility-only for characters/content without an authored pose-family.
        // Falling back to chest/shoulders for one frame while the authored arm pose remains visible
        // creates the exact split-state failure where the firing arm/weapon flips behind the body.
        let body_frames = super::player_model::player_rifle_ready_body_frames(world, owner);
        let animation_root = super::player_model::player_resolved_weapon_ready_root(world, owner);
        let animation_root_present = animation_root.is_some();
        let right_frame = super::player_model::player_right_hand_prop_frame(world, owner);
        let view_forward_model = if first_person_active || aim_alpha > 0.001 {
            super::player_model::player_rifle_view_forward_model(world, owner)
        } else {
            None
        };
        let recoil_alpha = visual.recoil_alpha.clamp(0.0, 1.0);
        let recoil_yaw_radians = visual.recoil_yaw_radians;
        ready_body_frames_for_debug = body_frames;
        right_frame_for_debug = right_frame;
        let resolved = animation_root
            .map(|root| (root.position, root.rotation))
            .or_else(|| {
                if authored_character_equipment_pose {
                    // Fail closed: keep the last valid authored visual transform rather than
                    // manufacturing a torso/carry root that disagrees with the visible arm pose.
                    return None;
                }
                body_frames.and_then(|(chest, right_shoulder, left_shoulder)| {
                    let presentation = presentation.as_ref()?;
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
                            None,
                            None,
                            aim_alpha,
                            obstruction_alpha,
                        )
                    })
                    .map(|contract| (contract.root.position, contract.root.rotation))
                })
            });
        weapon_root_source_for_debug = if animation_root_present {
            "authored_animation_socket"
        } else {
            "torso_compatibility"
        };
        resolved
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
    let Some((mut position, mut rotation)) = resolved else {
        return;
    };
    let component_offset =
        newengine_engine_runtime::gameplay::active_equipped_weapon_component_modifiers(
            world, owner,
        )
        .presentation_offset_local;
    let component_offset = Vec3::new(
        component_offset[0],
        component_offset[1],
        component_offset[2],
    );
    if component_offset.is_finite() {
        position += rotation.normalize_or_identity() * component_offset;
    }

    // Held long guns get bounded secondary angular inertia around the already-resolved firing-hand
    // pivot. This is not free rigid-body simulation: the handle remains exact, while fast aim/body
    // rotation and owner acceleration may produce only a few degrees of damped lag. Obstruction and
    // recoil tighten the spring so collision response and shot impulse remain immediate.
    let animation_root_authoritative = authored_weapon_presentation
        && super::player_model::player_resolved_weapon_ready_root(world, owner).is_some();
    if secondary_weapon_dynamics_enabled(
        authored_weapon_presentation,
        first_person_active,
        animation_root_authoritative,
    ) {
        if let (Some(presentation), Some((owner_position, owner_rotation))) = (
            presentation.as_ref(),
            newengine_transform::read_entity_world_pose_local_chain(world, owner),
        ) {
            let current_state = world
                .get::<WeaponSecondaryDynamicsState>(root)
                .copied()
                .unwrap_or_default();
            let next_state = step_long_gun_secondary_dynamics(
                current_state,
                presentation,
                rotation,
                owner_position,
                owner_rotation,
                dt,
                aim_alpha,
                visual.recoil_alpha,
                obstruction_alpha,
            );
            let target_root = crate::weapon_grip::WeaponRootTransform { position, rotation };
            if let Some(dynamic_root) = crate::weapon_grip::weapon_root_with_secondary_rotation(
                presentation,
                target_root,
                next_state.rotation_offset_local,
            ) {
                position = dynamic_root.position;
                rotation = dynamic_root.rotation;
            }
            if let Some(state) = world.get_mut::<WeaponSecondaryDynamicsState>(root) {
                *state = next_state;
            } else {
                let _ = world.insert(root, next_state);
            }
        }
    }

    if first_person_active {
        let reset_state = WeaponSecondaryDynamicsState {
            initialized: false,
            ..WeaponSecondaryDynamicsState::default()
        };
        if let Some(state) = world.get_mut::<WeaponSecondaryDynamicsState>(root) {
            *state = reset_state;
        } else {
            let _ = world.insert(root, reset_state);
        }
    }

    let canonical_transform = Transform {
        position,
        rotation: rotation.normalize_or_identity(),
        // Weapon scale is authored on mesh children. Skeleton scale must not multiply it.
        scale: Vec3::ONE,
    };
    let resolved_transform = world
        .get_mut::<newengine_transform::RuntimeTransformEditOverride>(root)
        .map(|manual| manual.resolve_from_base(canonical_transform))
        .unwrap_or(canonical_transform);
    if let Some(transform) = world.get_mut::<Transform>(root) {
        *transform = resolved_transform;
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

            // FPP ironsight is camera-primary. The character/weapon provider moves the authored
            // bilateral grip to the camera-space sight frame; the camera must never chase a rear sight
            // that is still transitioning or embedded in the hand. Keep the legacy/special-optic ADS
            // camera anchor clear for ordinary no-scope ADS.
            if first_person_active {
                if let Some(anchor) = world.get_mut::<
                    newengine_engine_runtime::gameplay::PlayerFirstPersonCameraAnchor,
                >(owner) {
                    anchor.ads_camera_position_ws = None;
                }
            }

            if newengine_runtime_env::var_os("NORTHSTAR_DEBUG_WEAPON_BASIS").is_some() {
                static BASIS_SAMPLES: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let sample = BASIS_SAMPLES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if sample < 2 {
                    let root_forward = (weapon_root.rotation * Vec3::Z).normalize_or_zero();
                    let root_up = (weapon_root.rotation * Vec3::Y).normalize_or_zero();
                    let root_right = (weapon_root.rotation * Vec3::X).normalize_or_zero();
                    let handle =
                        crate::weapon_grip::weapon_handle_position(presentation, weapon_root);
                    let handle_rotation =
                        crate::weapon_grip::weapon_handle_rotation(presentation, weapon_root);
                    let handle_forward = (handle_rotation * Vec3::Z).normalize_or_zero();
                    let handle_up = (handle_rotation * Vec3::Y).normalize_or_zero();
                    let handle_right = (handle_rotation * Vec3::X).normalize_or_zero();
                    let left_grip = crate::weapon_grip::weapon_ready_left_grip_position(
                        presentation,
                        weapon_root,
                    );
                    let muzzle =
                        crate::weapon_grip::weapon_muzzle_position(presentation, weapon_root);
                    newengine_ulog_api::ulog::info!(
                        "WEAPON_BASIS root_pos={:?} root_rot={:?} root_forward={:?} root_up={:?} root_right={:?} handle={:?} handle_rot={:?} handle_forward={:?} handle_up={:?} handle_right={:?} left_grip={:?} muzzle={:?} native_rig_to_runtime_basis={:?} authored_socket_to_weapon_handle_basis={:?} handle_from_root={:?} handle_rotation_from_root={:?}",
                        weapon_root.position,
                        weapon_root.rotation,
                        root_forward,
                        root_up,
                        root_right,
                        handle,
                        handle_rotation,
                        handle_forward,
                        handle_up,
                        handle_right,
                        left_grip,
                        muzzle,
                        presentation.native_rig_to_runtime_basis,
                        presentation.authored_socket_to_weapon_handle_basis,
                        presentation.handle_from_root,
                        presentation.handle_rotation_from_root,
                    );
                }
            }
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
            let (sight, sight_rotation) = presentation
                .as_ref()
                .and_then(|presentation| {
                    let weapon_root = crate::weapon_grip::WeaponRootTransform {
                        position: weapon_position,
                        rotation: weapon_rotation,
                    };
                    let rear =
                        crate::weapon_grip::weapon_rear_sight_position(presentation, weapon_root);
                    let forward =
                        crate::weapon_grip::weapon_sight_forward(presentation, weapon_root);
                    if !rear.is_finite() || forward.length_squared() <= 1.0e-8 {
                        return None;
                    }
                    let root_forward = (weapon_rotation * Vec3::Z).normalize_or_zero();
                    let sight_rotation = if root_forward.length_squared() > 1.0e-8 {
                        (Quat::from_rotation_arc(root_forward, forward) * weapon_rotation)
                            .normalize_or_identity()
                    } else {
                        Quat::from_rotation_arc(Vec3::Z, forward).normalize_or_identity()
                    };
                    let sight = newengine_engine_runtime::gameplay::EquippedWeaponSight::new(
                        rear, forward,
                    )?;
                    Some((sight, sight_rotation))
                })
                .map_or((None, None), |(sight, rotation)| {
                    (Some(sight), Some(rotation))
                });
            let previous = world
                .get::<WeaponEntitySockets>(root)
                .and_then(|sockets| sockets.muzzle);
            if let Some(socket_pose) =
                WeaponSocketPose::stationary(muzzle.position, weapon_rotation)
            {
                let socket_pose = socket_pose.with_measured_motion(previous, dt);
                let mut sockets = world
                    .get::<WeaponEntitySockets>(root)
                    .copied()
                    .unwrap_or_default();
                sockets.muzzle = Some(socket_pose);
                if let (Some(sight), Some(sight_rotation)) = (sight, sight_rotation) {
                    let previous_sight = sockets.sight;
                    sockets.sight = WeaponSocketPose::stationary(sight.position, sight_rotation)
                        .map(|pose| pose.with_measured_motion(previous_sight, dt));
                } else {
                    sockets.sight = None;
                }
                let _ = world.insert(root, sockets);
            }
            // Compatibility projections while gameplay callers migrate to weapon-entity sockets.
            let _ = world.insert(owner, muzzle);
            if let Some(sight) = sight {
                let _ = world.insert(owner, sight);
            } else {
                let _ =
                    world.remove::<newengine_engine_runtime::gameplay::EquippedWeaponSight>(owner);
            }
        } else {
            if let Some(mut sockets) = world.get::<WeaponEntitySockets>(root).copied() {
                sockets.muzzle = None;
                sockets.sight = None;
                let _ = world.insert(root, sockets);
            }
            let _ = world.remove::<EquippedWeaponMuzzle>(owner);
            let _ = world.remove::<newengine_engine_runtime::gameplay::EquippedWeaponSight>(owner);
        }
    }

    if authored_weapon_presentation
        && !visual.grip_debug_emitted
        && newengine_runtime_env::var_os("NORTHSTAR_DEBUG_WEAPON_GRIP").is_some()
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
                None,
                None,
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
                "WEAPON_GRIP player={} space='player_model' chest={:?} right_palm={:?} right_target={:?} right_error_m={:.5} handle={:?} stock={:?} shoulder_pocket={:?} stock_error_m={:.5} left_palm={:?} left_target={:?} left_error_m={:.5} l_grip={:?} root_source={} policy='authored animation socket is terminal; torso solve compatibility-only'",
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
                weapon_root_source_for_debug,
            );
            if let Some(state) = world.get_mut::<EquippedWeaponVisualRoot>(root) {
                state.grip_debug_emitted = true;
            }
        }
    }
}
