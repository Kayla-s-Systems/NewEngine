include!("weapon_ik/solver.rs");


/// Apply third-person RMB/free-aim as an authored upper-body delta, not as a detached weapon-root
/// correction. Native rifle prop sockets live under the wrists, so rotating both clavicle-owned arm
/// chains moves the firing socket, both hands and the muzzle together while leaving neck/head
/// authority untouched. Body turn-in-place may later consume large residual yaw and naturally
/// recenters this local arm offset.
fn apply_native_rifle_clavicle_aim_delta(
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
    pose: &mut [JointLocalPose],
    frames: &mut Vec<Mat4>,
    rig: &WeaponArmIkRig,
    authored_sight_forward: Vec3,
    target_sight_forward: Vec3,
) -> Result<bool, String> {
    let (Some(right_clavicle), Some(left_clavicle)) = (rig.right_clavicle, rig.left_clavicle) else {
        return Ok(false);
    };
    let from = authored_sight_forward.normalize_or_zero();
    let to = target_sight_forward.normalize_or_zero();
    if !from.is_finite()
        || !to.is_finite()
        || from.length_squared() <= 1.0e-8
        || to.length_squared() <= 1.0e-8
    {
        return Ok(false);
    }
    let mut delta = Quat::from_rotation_arc(from, to).normalize_or_identity();
    // Keep a native arm-space envelope. Beyond this the authored body turn owns the remaining yaw;
    // do not ask shoulders to absorb a near-180-degree camera orbit in one frame.
    if delta.w < 0.0 {
        delta = Quat::from_xyzw(-delta.x, -delta.y, -delta.z, -delta.w);
    }
    const MAX_ARM_AIM_RADIANS: f32 = 65.0_f32.to_radians();
    let angle = (2.0 * delta.w.clamp(-1.0, 1.0).acos()).abs();
    if angle > MAX_ARM_AIM_RADIANS && angle > 1.0e-6 {
        delta = Quat::IDENTITY
            .slerp(delta, MAX_ARM_AIM_RADIANS / angle)
            .normalize_or_identity();
    }
    if delta.dot(Quat::IDENTITY).abs() >= 0.999_999_9 {
        return Ok(true);
    }

    let arm_roots = if right_clavicle == left_clavicle {
        [Some(right_clavicle), None]
    } else {
        [Some(right_clavicle), Some(left_clavicle)]
    };
    for clavicle in arm_roots.into_iter().flatten() {
        let current_global = frames
            .get(clavicle)
            .copied()
            .ok_or("rifle ADS clavicle frame missing")?
            .to_scale_rotation_translation()
            .1
            .normalize_or_identity();
        let desired_global = (delta * current_global).normalize_or_identity();
        set_pose_joint_global_rotation(
            skeleton,
            pose,
            frames,
            clavicle,
            desired_global,
        )?;
        refresh_model_joint_frames_subtree(animation_runtime, pose, frames, clavicle)?;
    }
    Ok(true)
}

/// Authored equipment animation owns the third-person weapon pose whenever it supplies a firing-hand
/// contact. A qualified prop socket is the strongest authority; otherwise the animated firing palm owns
/// the complete weapon root transform (translation + orientation). The anatomical ReadyHold contract is
/// only a compatibility fallback for equipment without an authored hand-contact pose. IK is terminal
/// contact stabilization for recoil/obstruction/secondary motion and must be an identity solve at the
/// canonical authored pose. Reload may release constraints.
fn apply_equipped_weapon_support_ik(
    presentation: &newengine_engine_runtime::gameplay::WeaponPresentationDefinition,
    rig: Option<&WeaponArmIkRig>,
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
    pose: &mut [JointLocalPose],
    frames: &mut Vec<Mat4>,
    view_forward_model: Option<Vec3>,
    view_rotation_model: Option<Quat>,
    first_person_eye_model: Option<Vec3>,
    first_person_active: bool,
    aim_alpha: f32,
    recoil_alpha: f32,
    recoil_yaw_radians: f32,
    obstruction_alpha: f32,
    secondary_rotation_offset_local: Vec3,
    authored_hand_contacts: bool,
    authored_prop_socket_authority: bool,
    stabilize_native_support_hand: bool,
    support_right_hand: bool,
    support_left_hand: bool,
    relative_ads_state: Option<&mut EquipmentRelativeAdsState>,
) -> Result<Option<WeaponIkSolveResult>, String> {
    let Some(rig) = rig else {
        return Ok(None);
    };
    rebuild_model_joint_frames(animation_runtime, pose, frames)?;
    let chest = *frames
        .get(rig.chest)
        .ok_or("weapon ReadyHold chest frame is unavailable")?;
    let right_shoulder = *frames
        .get(rig.right_shoulder)
        .ok_or("weapon ReadyHold right shoulder frame is unavailable")?;
    let left_shoulder = *frames
        .get(rig.left_shoulder)
        .ok_or("weapon ReadyHold left shoulder frame is unavailable")?;

    // Original-content weapon presentation is authored through the character's prop domain.
    // `*_hand_prop_attachment` is a sibling of the anatomical palm under the wrist, with its own
    // authored basis; it is not interchangeable with `*_palm`. When a complete grip composition
    // qualifies that socket, the socket owns the weapon root in both TPP and full-body FPP.
    // Camera/palm-driven solving is only a compatibility path for content without that contract.

    // Prop-socket authority is stricter than generic authored hand contact. The same skeleton
    // attachment joint can be present in unrelated full-body/aim clips; applying a basis recovered
    // from a different reference-composition domain to those channels is a cross-domain transform
    // bug. Only the matching complete grip bundle may opt the prop socket into root ownership.
    let authored_prop_root = (authored_hand_contacts && authored_prop_socket_authority)
        .then_some(rig.right_prop_attachment)
        .flatten()
        .and_then(|index| frames.get(index).copied())
        .and_then(|frame| {
            crate::weapon_grip::weapon_root_from_authored_prop_frame(presentation, frame)
        });

    // Third-person Ready/Aim is character-authored. If there is no qualified prop socket, preserve
    // the complete firing-palm -> weapon transform recovered from the authored pose. Keeping only the
    // handle translation here and re-inventing orientation from the torso is destructive for compact
    // pistol grips and for any original-content pose that deliberately authors wrist roll/yaw.
    let authored_hand_root = (authored_prop_root.is_none()
        && authored_hand_contacts
        && support_right_hand
        && !first_person_active)
        .then(|| frames[rig.right_palm])
        .and_then(|frame| crate::weapon_grip::weapon_root_from_right_palm(presentation, frame));
    let support_anchor = (authored_prop_root.is_none()
        && authored_hand_root.is_none()
        && authored_hand_contacts
        && support_left_hand
        && !first_person_active)
        .then(|| frames[rig.left_palm])
        .and_then(|frame| {
            crate::weapon_grip::weapon_left_grip_anchor_from_left_palm(presentation, frame)
        });
    let first_person_hand_root = (authored_prop_root.is_none()
        && first_person_active
        && authored_hand_contacts
        && support_right_hand)
        .then(|| frames[rig.right_palm])
        .zip(view_rotation_model)
        .and_then(|(right_palm, view_rotation)| {
            crate::weapon_grip::weapon_first_person_hand_anchored_root(
                presentation,
                right_palm,
                view_rotation,
                aim_alpha,
                recoil_alpha,
                recoil_yaw_radians,
            )
        });
    let mut relative_ads_state = relative_ads_state;
    let native_relative_target = authored_prop_root
        .filter(|_| !first_person_active)
        .and_then(|root| {
            let authored_sight = crate::weapon_grip::weapon_sight_forward(presentation, root);
            relative_ads_state.as_deref_mut().and_then(|state| {
                view_rotation_model.and_then(|view| state.relative_sight_target(view, authored_sight))
            })
        });

    // Native TPP rifle aim is arm-owned. Move both clavicle/arm chains first; the weapon root is then
    // re-read from the moved right prop socket. This cannot be cancelled by a right-arm reach failure
    // because the firing hand/socket itself is the motion authority.
    let native_arm_aim_applied = if let (Some(root), Some(target)) =
        (authored_prop_root, native_relative_target)
    {
        let authored_sight = crate::weapon_grip::weapon_sight_forward(presentation, root);
        apply_native_rifle_clavicle_aim_delta(
            skeleton,
            animation_runtime,
            pose,
            frames,
            rig,
            authored_sight,
            target,
        )?
    } else {
        false
    };
    let authored_prop_root = if native_arm_aim_applied {
        rig.right_prop_attachment
            .and_then(|index| frames.get(index).copied())
            .and_then(|frame| {
                crate::weapon_grip::weapon_root_from_authored_prop_frame(presentation, frame)
            })
            .or(authored_prop_root)
    } else {
        authored_prop_root
    };
    let native_sight_aim_root = if native_arm_aim_applied {
        authored_prop_root
    } else {
        authored_prop_root
            .filter(|_| !first_person_active)
            .and_then(|root| {
                let target = match relative_ads_state.as_ref() {
                    Some(_) => native_relative_target,
                    None => (aim_alpha > 1.0e-4).then_some(view_forward_model).flatten(),
                }?;
                crate::weapon_grip::weapon_sight_aligned_root_around_stock_contact(
                    presentation,
                    root,
                    target,
                )
            })
    };
    let root_contract = if let Some(root) = authored_prop_root {
        let root = native_sight_aim_root.unwrap_or(root);
        // Qualified original-content grip owns the prop frame directly. Do not reinterpret its
        // sibling anatomical palm as a weapon socket and do not re-derive orientation from torso.
        crate::weapon_grip::weapon_ready_solve_contract_presented(
            presentation,
            chest,
            right_shoulder,
            left_shoulder,
            None,
            aim_alpha,
            0.0,
            0.0,
        )
        .map(|mut contract| {
            contract.root = root;
            let stock_from_handle = Vec3::new(
                presentation.stock_contact_from_handle[0],
                presentation.stock_contact_from_handle[1],
                presentation.stock_contact_from_handle[2],
            );
            contract.stock_contact = crate::weapon_grip::weapon_handle_position(presentation, root)
                + crate::weapon_grip::weapon_handle_rotation(presentation, root)
                    * stock_from_handle;
            contract
        })
    } else if let Some(root) = first_person_hand_root {
        // Compatibility FPP path for rigs without an authored prop socket: preserve the firing
        // handle while camera ADS changes sight orientation.
        crate::weapon_grip::weapon_ready_solve_contract_presented(
            presentation,
            chest,
            right_shoulder,
            left_shoulder,
            None,
            aim_alpha,
            0.0,
            0.0,
        )
        .map(|mut contract| {
            contract.root = root;
            let stock_from_handle = Vec3::new(
                presentation.stock_contact_from_handle[0],
                presentation.stock_contact_from_handle[1],
                presentation.stock_contact_from_handle[2],
            );
            contract.stock_contact = crate::weapon_grip::weapon_handle_position(presentation, root)
                + crate::weapon_grip::weapon_handle_rotation(presentation, root)
                    * stock_from_handle;
            contract
        })
    } else if first_person_active {
        // Defensive compatibility fallback for rigs that explicitly disable the firing-hand support
        // chain. Normal full-body FPP never enters this path.
        first_person_eye_model
            .zip(view_rotation_model)
            .and_then(|(eye, view_rotation)| {
                crate::weapon_grip::weapon_first_person_solve_contract_presented(
                    presentation,
                    eye,
                    view_rotation,
                    right_shoulder,
                    left_shoulder,
                    aim_alpha,
                    recoil_alpha,
                    recoil_yaw_radians,
                )
            })
            .or_else(|| {
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
            })
    } else if let Some(root) = authored_hand_root {
        // The original character grip owns both handle position and weapon orientation. Torso data is
        // retained only for elbow-pole/soft stock metadata; it must not rotate or translate this root.
        crate::weapon_grip::weapon_ready_solve_contract_presented(
            presentation,
            chest,
            right_shoulder,
            left_shoulder,
            None,
            aim_alpha,
            0.0,
            0.0,
        )
        .map(|mut contract| {
            contract.root = root;
            let stock_from_handle = Vec3::new(
                presentation.stock_contact_from_handle[0],
                presentation.stock_contact_from_handle[1],
                presentation.stock_contact_from_handle[2],
            );
            contract.stock_contact = crate::weapon_grip::weapon_handle_position(presentation, root)
                + crate::weapon_grip::weapon_handle_rotation(presentation, root)
                    * stock_from_handle;
            contract
        })
    } else {
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
    };
    let base_contract = root_contract
        .and_then(|contract| {
            if authored_prop_root.is_some() || authored_hand_root.is_some() {
                // Third-person hand/socket-owned roots are exact attachment contracts. Contact/reach
                // fitting may stabilize limbs around them, but may never rewrite the root itself.
                Some(contract)
            } else if first_person_hand_root.is_some() {
                // FPP keeps the firing handle fixed while obstruction pivots the barrel away from the
                // blocker. With both explicit anchors omitted this helper performs only that authored
                // handle-preserving pivot; it cannot pull the weapon toward torso/support estimates.
                crate::weapon_grip::weapon_ready_contract_with_contacts(
                    presentation,
                    contract,
                    None,
                    None,
                    aim_alpha,
                    obstruction_alpha,
                )
            } else {
                crate::weapon_grip::weapon_ready_contract_with_contacts(
                    presentation,
                    contract,
                    None,
                    support_anchor,
                    aim_alpha,
                    obstruction_alpha,
                )
            }
        })
        .ok_or("weapon presentation could not resolve camera/torso contact constraint")?;
    // Reach fitting is valid only while torso/camera space owns translation. Once an authored
    // firing hand supplies the handle anchor, moving the root again would break the exact contact
    // and re-introduce the hand/root feedback loop this branch is designed to avoid.
    let base_contract = if first_person_hand_root.is_some()
        || authored_prop_root.is_some()
        || authored_hand_root.is_some()
    {
        // A hand/socket-owned root must never be translated by reach fitting: that would break the
        // exact firing-palm/handle invariant. Residual contact error belongs to the bounded arm solver.
        base_contract
    } else {
        fit_weapon_contract_to_supported_arm_reach(
            presentation,
            pose,
            frames,
            rig,
            base_contract,
            support_right_hand,
            support_left_hand,
        )
    };
    let mut base_root = base_contract.root;
    let native_prop_owned = authored_prop_root.is_some();
    let mut contract = if native_prop_owned {
        // A qualified prop socket is already the final authored weapon frame. Applying generic
        // secondary rotation here would detach the weapon from that frame and force anatomical IK
        // to compensate, which is exactly the arm inversion visible with native TLOU-style rigs.
        base_contract
    } else {
        crate::weapon_grip::weapon_ready_contract_with_secondary_rotation(
            presentation,
            base_contract,
            secondary_rotation_offset_local,
        )
        .ok_or("weapon ReadyHold could not resolve secondary constraint")?
    };
    // A native prop-owned rifle keeps the firing arm and weapon root authored. The support palm,
    // however, is an anatomical sibling of `l_hand_prop`: a perfect prop-helper frame does not put
    // the visible palm on the foregrip. Strict rifle Ready/Aim therefore stabilizes only the left
    // support arm, while the right arm/root remain immutable.
    let native_view_aim = native_prop_owned && native_sight_aim_root.is_some();
    let solve_right_hand = support_right_hand
        && (!native_prop_owned || (native_view_aim && !native_arm_aim_applied));
    let solve_left_hand =
        support_left_hand && (!native_prop_owned || stabilize_native_support_hand);
    let hand_owned_root = first_person_hand_root.is_some() || authored_hand_root.is_some();

    // Native TLOU prop sockets are siblings of the anatomical palm. When ADS rotates the weapon
    // toward the real sight line, derive the firing-palm target from the *current authored*
    // palm->prop relationship instead of legacy ReadyHold offsets. This preserves Abby's native
    // wrist/socket basis while allowing camera intent to move the complete weapon+arm contract.
    let native_right_palm_target = if native_view_aim {
        rig.right_prop_attachment
            .and_then(|socket_index| frames.get(socket_index).copied())
            .and_then(|socket_frame| {
                let palm_frame = frames.get(rig.right_palm).copied()?;
                let palm_to_socket = palm_frame.inverse() * socket_frame;
                let desired_handle_position =
                    crate::weapon_grip::weapon_handle_position(presentation, contract.root);
                let desired_handle_rotation =
                    crate::weapon_grip::weapon_handle_rotation(presentation, contract.root);
                let socket_to_handle = Quat::from_xyzw(
                    presentation.authored_socket_to_weapon_handle_basis[0],
                    presentation.authored_socket_to_weapon_handle_basis[1],
                    presentation.authored_socket_to_weapon_handle_basis[2],
                    presentation.authored_socket_to_weapon_handle_basis[3],
                )
                .normalize_or_identity();
                let desired_socket_rotation =
                    (desired_handle_rotation * socket_to_handle.inverse()).normalize_or_identity();
                let desired_socket_frame = Mat4::from_scale_rotation_translation(
                    Vec3::ONE,
                    desired_socket_rotation,
                    desired_handle_position,
                );
                let desired_palm_frame = desired_socket_frame * palm_to_socket.inverse();
                let (scale, rotation, position) =
                    desired_palm_frame.to_scale_rotation_translation();
                (scale.is_finite()
                    && scale.x > 0.0
                    && scale.y > 0.0
                    && scale.z > 0.0
                    && rotation.is_finite()
                    && position.is_finite())
                .then_some((position, rotation.normalize_or_identity()))
            })
    } else {
        None
    };
    let right_target = native_right_palm_target
        .map(|target| target.0)
        .unwrap_or_else(|| {
            if hand_owned_root {
                crate::weapon_grip::weapon_hand_owned_right_palm_position(
                    presentation,
                    contract.root,
                )
            } else {
                crate::weapon_grip::weapon_ready_right_palm_position(presentation, contract.root)
            }
        });
    let right_rotation = native_right_palm_target
        .map(|target| target.1)
        .unwrap_or_else(|| {
            if hand_owned_root {
                crate::weapon_grip::weapon_hand_owned_right_palm_rotation(
                    presentation,
                    contract.root,
                )
            } else {
                crate::weapon_grip::weapon_ready_right_palm_rotation(presentation, contract.root)
            }
        });
    let native_left_palm_rotation = frames[rig.left_palm]
        .to_scale_rotation_translation()
        .1
        .normalize_or_identity();
    if solve_right_hand && !right_target.is_finite() {
        return Err("weapon ReadyHold authored right-hand target is non-finite".to_owned());
    }

    let right_solved = if solve_right_hand {
        solve_arm_to_palm_contact(
            skeleton,
            animation_runtime,
            pose,
            frames,
            rig.right_shoulder,
            rig.right_elbow,
            rig.right_wrist,
            rig.right_palm,
            right_target,
            contract.right_elbow_pole,
            right_rotation,
            "right",
        )?
    } else {
        false
    };

    // Sight alignment is never allowed to detach the rendered rifle from the firing hand. If the
    // anatomical right arm cannot reach the requested camera-aligned contract without violating the
    // safe-extension gate, keep the original authored prop root for this frame. Large yaw is then
    // resolved by body turn-in-place instead of rubber-arm stretching or a floating weapon.
    let native_view_aim_committed = if native_view_aim && !native_arm_aim_applied && !right_solved {
        if let Some(authored_root) = authored_prop_root {
            contract.root = authored_root;
            base_root = authored_root;
            let stock_from_handle = Vec3::new(
                presentation.stock_contact_from_handle[0],
                presentation.stock_contact_from_handle[1],
                presentation.stock_contact_from_handle[2],
            );
            contract.stock_contact =
                crate::weapon_grip::weapon_handle_position(presentation, authored_root)
                    + crate::weapon_grip::weapon_handle_rotation(presentation, authored_root)
                        * stock_from_handle;
        }
        false
    } else {
        true
    };

    let left_target = if native_prop_owned && stabilize_native_support_hand {
        // The character prop branch already defines the moving weapon frame. Target the visible
        // support palm directly at the weapon's authored foregrip; legacy palm offsets belong only
        // to the compatibility ReadyHold solver and may come from a different animation baseline.
        crate::weapon_grip::weapon_ready_left_grip_position(presentation, contract.root)
    } else {
        crate::weapon_grip::weapon_ready_left_palm_position(presentation, contract.root)
    };
    let left_rotation = if native_prop_owned && stabilize_native_support_hand {
        native_left_palm_rotation
    } else {
        crate::weapon_grip::weapon_ready_left_palm_rotation(presentation, contract.root)
    };
    if solve_left_hand && !left_target.is_finite() {
        return Err("weapon ReadyHold authored support-hand target is non-finite".to_owned());
    }
    if solve_left_hand {
        solve_arm_to_palm_contact(
            skeleton,
            animation_runtime,
            pose,
            frames,
            rig.left_shoulder,
            rig.left_elbow,
            rig.left_wrist,
            rig.left_palm,
            left_target,
            contract.left_elbow_pole,
            left_rotation,
            "left",
        )?;
    }

    // Each arm solve incrementally refreshed its affected branch, so the shared frame table is
    // already coherent here; a final full-skeleton FK pass would duplicate work every render frame.
    let socket_frame_error = native_prop_owned
        .then_some(rig.right_prop_attachment)
        .flatten()
        .and_then(|index| frames.get(index).copied())
        .and_then(|frame| {
            crate::weapon_grip::weapon_handle_frame_error_from_authored_socket(
                presentation,
                contract.root,
                frame,
            )
        });
    let socket_position_error = socket_frame_error
        .map(|error| error.position_m)
        .unwrap_or(0.0);
    let socket_angular_error = socket_frame_error
        .map(|error| error.angular_degrees)
        .unwrap_or(0.0);
    let right_error = if solve_right_hand && native_view_aim_committed {
        (frames[rig.right_palm].transform_point3(Vec3::ZERO) - right_target).length()
    } else {
        0.0
    };
    let left_error = if solve_left_hand {
        (frames[rig.left_palm].transform_point3(Vec3::ZERO) - left_target).length()
    } else {
        0.0
    };

    if newengine_runtime_env::var_os("NORTHSTAR_DEBUG_WEAPON_CONTACT_FRAMES").is_some() {
        static CONTACT_FRAME_SAMPLES: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        let sample = CONTACT_FRAME_SAMPLES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if sample < 8 {
            let handle = crate::weapon_grip::weapon_handle_position(presentation, contract.root);
            let handle_rotation =
                crate::weapon_grip::weapon_handle_rotation(presentation, contract.root);
            let left_grip =
                crate::weapon_grip::weapon_ready_left_grip_position(presentation, contract.root);
            let right_palm_frame = frames[rig.right_palm];
            let left_palm_frame = frames[rig.left_palm];
            let right_palm = right_palm_frame.transform_point3(Vec3::ZERO);
            let left_palm = left_palm_frame.transform_point3(Vec3::ZERO);
            let (_, right_palm_rotation, _) = right_palm_frame.to_scale_rotation_translation();
            let (_, left_palm_rotation, _) = left_palm_frame.to_scale_rotation_translation();
            let right_prop_frame = rig
                .right_prop_attachment
                .and_then(|index| frames.get(index).copied());
            let left_prop_frame = rig
                .left_prop_attachment
                .and_then(|index| frames.get(index).copied());
            let right_prop = right_prop_frame.map(|frame| frame.transform_point3(Vec3::ZERO));
            let left_prop = left_prop_frame.map(|frame| frame.transform_point3(Vec3::ZERO));
            let right_prop_rotation =
                right_prop_frame.map(|frame| frame.to_scale_rotation_translation().1);
            let left_prop_rotation =
                left_prop_frame.map(|frame| frame.to_scale_rotation_translation().1);
            newengine_ulog_api::ulog::info!(
                "WEAPON_CONTACT_FRAMES right_handle_error_cm={:.4} left_support_error_cm={:.4} weapon_socket_position_error_cm={:.4} weapon_socket_angular_error_deg={:.4} right_palm={:?} right_palm_rotation={:?} right_prop={:?} right_prop_rotation={:?} handle={:?} handle_rotation={:?} left_palm={:?} left_palm_rotation={:?} left_prop={:?} left_prop_rotation={:?} l_grip={:?} weapon_root_position={:?} weapon_root_rotation={:?} native_rig_to_runtime_basis={:?} authored_socket_to_weapon_handle_basis={:?} handle_from_root={:?} handle_rotation_from_root={:?}",
                right_error * 100.0,
                left_error * 100.0,
                socket_position_error * 100.0,
                socket_angular_error,
                right_palm,
                right_palm_rotation,
                right_prop,
                right_prop_rotation,
                handle,
                handle_rotation,
                left_palm,
                left_palm_rotation,
                left_prop,
                left_prop_rotation,
                left_grip,
                contract.root.position,
                contract.root.rotation,
                presentation.native_rig_to_runtime_basis,
                presentation.authored_socket_to_weapon_handle_basis,
                presentation.handle_from_root,
                presentation.handle_rotation_from_root,
            );
        }
    }
    // Stock/shoulder is intentionally a soft angular constraint. It must not be promoted to a
    // hard IK failure because different authored body proportions legitimately leave a few cm of
    // stock compression/clearance. Hand and socket/handle residuals are hard invariants.
    let error = right_error.max(left_error).max(socket_position_error);
    if !error.is_finite() || !socket_angular_error.is_finite() {
        return Err("weapon ReadyHold IK produced non-finite contact error".to_owned());
    }
    Ok(Some(WeaponIkSolveResult {
        error_m: error,
        right_error_m: right_error,
        left_error_m: left_error,
        socket_position_error_m: socket_position_error,
        socket_angular_error_deg: socket_angular_error,
        base_root,
    }))
}

include!("weapon_ik/helper_pose.rs");
