/// Reconcile an authored ReadyHold with the character's live contact points. The firing-hand
/// contact is the primary kinematic joint and therefore owns weapon translation. Stock/shoulder
/// and support-hand contacts only contribute bounded angular corrections around that handle. This
/// mirrors the original layered grip graph: animation establishes the pose, constraints remove
/// small contact error, and no bilateral IK stage is allowed to drag the rifle away from the hand.
pub(crate) fn weapon_ready_contract_with_contacts(
    presentation: &WeaponPresentationDefinition,
    mut contract: WeaponReadySolveContract,
    handle_anchor: Option<Vec3>,
    support_anchor: Option<Vec3>,
    aim_alpha: f32,
    obstruction_alpha: f32,
) -> Option<WeaponReadySolveContract> {
    let presentation = presentation.clone().sanitized();
    let aim_alpha = if aim_alpha.is_finite() {
        aim_alpha.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let obstruction_alpha = if obstruction_alpha.is_finite() {
        obstruction_alpha.clamp(0.0, 1.0)
    } else {
        0.0
    };

    fn rotate_about_handle(
        presentation: &WeaponPresentationDefinition,
        contract: &mut WeaponReadySolveContract,
        delta: Quat,
        handle: Vec3,
    ) {
        contract.root.rotation = (delta * contract.root.rotation).normalize_or_identity();
        contract.root.position =
            weapon_root_position_from_handle(presentation, handle, contract.root.rotation);
        contract.stock_contact = handle
            + weapon_handle_rotation(presentation, contract.root)
                * v3(presentation.stock_contact_from_handle);
    }

    fn bounded_arc(from: Vec3, to: Vec3, max_angle: f32, weight: f32) -> Option<Quat> {
        let from = from.normalize_or_zero();
        let to = to.normalize_or_zero();
        if from.length_squared() <= 1.0e-8 || to.length_squared() <= 1.0e-8 {
            return None;
        }
        let full = Quat::from_rotation_arc(from, to).normalize_or_identity();
        let angle = 2.0 * full.w.abs().clamp(0.0, 1.0).acos();
        if !angle.is_finite() || angle <= 1.0e-6 {
            return None;
        }
        let t = (weight.clamp(0.0, 1.0) * (max_angle.max(0.0) / angle).min(1.0)).clamp(0.0, 1.0);
        Some(Quat::IDENTITY.slerp(full, t).normalize_or_identity())
    }

    let Some(handle_anchor) = handle_anchor.filter(|anchor| anchor.is_finite()) else {
        // Anatomical ReadyHold owns translation when no explicit firing-hand contact is supplied.
        // Obstruction may still pivot the barrel around that authored handle, but relaxed hands
        // are observations only and must never drag the weapon root away from the torso solve.
        if obstruction_alpha > 0.0 {
            let handle = weapon_handle_position(&presentation, contract.root);
            let local_right = (weapon_handle_rotation(&presentation, contract.root) * Vec3::X)
                .normalize_or_zero();
            if local_right.length_squared() > 1.0e-8 {
                let delta =
                    Quat::from_axis_angle(local_right, -50.0_f32.to_radians() * obstruction_alpha);
                rotate_about_handle(&presentation, &mut contract, delta, handle);
            }
        }
        return (contract.root.position.is_finite()
            && contract.root.rotation.is_finite()
            && contract.stock_contact.is_finite())
        .then_some(contract);
    };

    // Explicit contact mode is reserved for presentation paths where an authored hand socket is
    // intentionally the kinematic owner (for example a dedicated manipulation/reload contract).
    contract.root.position =
        weapon_root_position_from_handle(&presentation, handle_anchor, contract.root.rotation);
    contract.stock_contact = handle_anchor
        + weapon_handle_rotation(&presentation, contract.root)
            * v3(presentation.stock_contact_from_handle);

    // Shoulder stock is a soft rotational constraint, never a positional owner. At full ADS the
    // view axis has priority, so the shoulder correction is intentionally weaker.
    let stock_vector = weapon_handle_rotation(&presentation, contract.root)
        * v3(presentation.stock_contact_from_handle);
    let shoulder_vector = contract.shoulder_pocket - handle_anchor;
    if let Some(delta) = bounded_arc(
        stock_vector,
        shoulder_vector,
        18.0_f32.to_radians(),
        0.58 * (1.0 - aim_alpha * 0.55),
    ) {
        rotate_about_handle(&presentation, &mut contract, delta, handle_anchor);
    }

    // The support hand contributes only a small angular correction. Large disagreement is left to
    // the authored character clip instead of twisting the rifle/arms into a rubber IK pose.
    if let Some(support_anchor) = support_anchor.filter(|anchor| anchor.is_finite()) {
        let grip_vector = weapon_handle_rotation(&presentation, contract.root)
            * v3(presentation.left_grip_from_handle);
        let support_vector = support_anchor - handle_anchor;
        if let Some(delta) = bounded_arc(
            grip_vector,
            support_vector,
            12.0_f32.to_radians(),
            0.42 * (1.0 - aim_alpha * 0.35),
        ) {
            rotate_about_handle(&presentation, &mut contract, delta, handle_anchor);
        }
    }

    // Original long-gun graphs have explicit aim-blocked add/sub layers. Until those exact clips
    // are composed, reproduce their physical invariant procedurally: pivot the barrel upward about
    // the firing hand, never translate the weapon through the obstacle.
    if obstruction_alpha > 0.0 {
        let local_right =
            (weapon_handle_rotation(&presentation, contract.root) * Vec3::X).normalize_or_zero();
        if local_right.length_squared() > 1.0e-8 {
            let delta =
                Quat::from_axis_angle(local_right, -50.0_f32.to_radians() * obstruction_alpha);
            rotate_about_handle(&presentation, &mut contract, delta, handle_anchor);
        }
    }

    (contract.root.position.is_finite()
        && contract.root.rotation.is_finite()
        && contract.stock_contact.is_finite())
    .then_some(contract)
}

#[inline]
fn quat_from_rotation_vector(rotation_vector: Vec3) -> Quat {
    if !rotation_vector.is_finite() {
        return Quat::IDENTITY;
    }
    let angle = rotation_vector.length();
    if !angle.is_finite() || angle <= 1.0e-7 {
        return Quat::IDENTITY;
    }
    Quat::from_axis_angle(rotation_vector / angle, angle).normalize_or_identity()
}

/// Applies a small local-space secondary rotation around the firing-hand handle. The handle is an
/// exact kinematic pivot; only the long-gun orientation is allowed to lag.
pub(crate) fn weapon_root_with_secondary_rotation(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
    rotation_offset_local: Vec3,
) -> Option<WeaponRootTransform> {
    if !rotation_offset_local.is_finite() {
        return None;
    }
    let presentation = presentation.clone().sanitized();
    let handle = weapon_handle_position(&presentation, root);
    let rotation =
        (root.rotation * quat_from_rotation_vector(rotation_offset_local)).normalize_or_identity();
    let position = weapon_root_position_from_handle(&presentation, handle, rotation);
    (position.is_finite() && rotation.is_finite())
        .then_some(WeaponRootTransform { position, rotation })
}

/// Applies the same secondary rotation to the ReadyHold contract so support-hand IK consumes the
/// same spring state as the rendered weapon (one presentation frame behind by design).
pub(crate) fn weapon_ready_contract_with_secondary_rotation(
    presentation: &WeaponPresentationDefinition,
    mut contract: WeaponReadySolveContract,
    rotation_offset_local: Vec3,
) -> Option<WeaponReadySolveContract> {
    contract.root =
        weapon_root_with_secondary_rotation(presentation, contract.root, rotation_offset_local)?;
    let presentation = presentation.clone().sanitized();
    let handle = weapon_handle_position(&presentation, contract.root);
    contract.stock_contact = handle
        + weapon_handle_rotation(&presentation, contract.root)
            * v3(presentation.stock_contact_from_handle);
    contract.stock_contact.is_finite().then_some(contract)
}

#[inline]
pub(crate) fn weapon_handle_position(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Vec3 {
    root.position + root.rotation * v3(presentation.handle_from_root)
}

#[inline]
pub(crate) fn weapon_handle_rotation(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Quat {
    (root.rotation * handle_rotation_from_root(presentation)).normalize_or_identity()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WeaponHandleFrameError {
    pub position_m: f32,
    pub angular_degrees: f32,
}

pub(crate) fn weapon_handle_frame_error_from_authored_socket(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
    socket_frame: Mat4,
) -> Option<WeaponHandleFrameError> {
    let (scale, socket_rotation, socket_position) = socket_frame.to_scale_rotation_translation();
    if !scale.is_finite()
        || scale.x <= 0.0
        || scale.y <= 0.0
        || scale.z <= 0.0
        || !socket_rotation.is_finite()
        || !socket_position.is_finite()
    {
        return None;
    }
    let expected_rotation = (socket_rotation.normalize_or_identity()
        * q4(presentation.authored_socket_to_weapon_handle_basis))
    .normalize_or_identity();
    let actual_rotation = weapon_handle_rotation(presentation, root);
    let dot = actual_rotation.dot(expected_rotation).abs().clamp(0.0, 1.0);
    let angular_degrees = (2.0 * dot.acos()).to_degrees();
    let position_m = weapon_handle_position(presentation, root).distance(socket_position);
    (position_m.is_finite() && angular_degrees.is_finite()).then_some(WeaponHandleFrameError {
        position_m,
        angular_degrees,
    })
}

#[inline]
pub(crate) fn weapon_muzzle_position(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Vec3 {
    root.position + root.rotation * v3(presentation.muzzle_from_root)
}

#[inline]
pub(crate) fn weapon_muzzle_forward(root: WeaponRootTransform) -> Vec3 {
    (root.rotation * Vec3::Z).normalize_or_zero()
}

#[inline]
pub(crate) fn weapon_ready_left_grip_position(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Vec3 {
    weapon_handle_position(presentation, root)
        + weapon_handle_rotation(presentation, root) * v3(presentation.left_grip_from_handle)
}

#[inline]
pub(crate) fn weapon_ready_right_palm_rotation(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Quat {
    (root.rotation * q4(presentation.ready_right_palm_to_weapon).inverse()).normalize_or_identity()
}

#[inline]
pub(crate) fn weapon_ready_left_palm_rotation(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Quat {
    (root.rotation * q4(presentation.ready_left_palm_to_weapon).inverse()).normalize_or_identity()
}

#[inline]
pub(crate) fn weapon_ready_right_palm_position(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Vec3 {
    let rotation = weapon_ready_right_palm_rotation(presentation, root);
    let firing_grip = weapon_handle_position(presentation, root);
    firing_grip - rotation * v3(presentation.right_palm_to_handle)
}

#[inline]
pub(crate) fn weapon_ready_left_palm_position(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Vec3 {
    let rotation = weapon_ready_left_palm_rotation(presentation, root);
    weapon_ready_left_grip_position(presentation, root)
        - rotation * v3(presentation.ready_left_palm_to_left_grip)
}
