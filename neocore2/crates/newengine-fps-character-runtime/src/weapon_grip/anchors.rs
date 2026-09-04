#[inline]
fn v3(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

#[inline]
fn q4(value: [f32; 4]) -> Quat {
    Quat::from_xyzw(value[0], value[1], value[2], value[3]).normalize_or_identity()
}

#[inline]
fn handle_rotation_from_root(presentation: &WeaponPresentationDefinition) -> Quat {
    q4(presentation.handle_rotation_from_root)
}

#[inline]
fn weapon_root_position_from_handle(
    presentation: &WeaponPresentationDefinition,
    handle_position: Vec3,
    root_rotation: Quat,
) -> Vec3 {
    handle_position - root_rotation * v3(presentation.handle_from_root)
}

#[inline]
fn right_palm_to_runtime_weapon_rotation(presentation: &WeaponPresentationDefinition) -> Quat {
    (q4(presentation.right_palm_to_native_rig) * q4(presentation.native_rig_to_runtime_basis))
        .normalize_or_identity()
}

#[inline]
fn weapon_rotation_from_palm(
    presentation: &WeaponPresentationDefinition,
    palm_rotation: Quat,
) -> Quat {
    (palm_rotation * right_palm_to_runtime_weapon_rotation(presentation)).normalize_or_identity()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WeaponRootTransform {
    pub position: Vec3,
    pub rotation: Quat,
}

/// Resolves a third-person weapon root from a character-authored prop socket.
///
/// The socket is an authored reference-composition frame, not a weapon root. Runtime first resolves
/// the weapon handle with the recovered socket -> handle basis, then applies the inverse of the
/// complete weapon-root -> handle transform:
///
/// `weapon_root_world = socket_world * socket_to_handle * inverse(handle_from_weapon_root)`.
#[inline]
pub(crate) fn weapon_root_from_authored_prop_frame(
    presentation: &WeaponPresentationDefinition,
    frame: Mat4,
) -> Option<WeaponRootTransform> {
    let (scale, socket_rotation, handle_position) = frame.to_scale_rotation_translation();
    if !scale.is_finite()
        || scale.x <= 0.0
        || scale.y <= 0.0
        || scale.z <= 0.0
        || !socket_rotation.is_finite()
        || !handle_position.is_finite()
    {
        return None;
    }
    let handle_rotation = (socket_rotation.normalize_or_identity()
        * q4(presentation.authored_socket_to_weapon_handle_basis))
    .normalize_or_identity();
    let rotation = (handle_rotation * handle_rotation_from_root(presentation).inverse())
        .normalize_or_identity();
    let position = weapon_root_position_from_handle(presentation, handle_position, rotation);
    (position.is_finite() && rotation.is_finite())
        .then_some(WeaponRootTransform { position, rotation })
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BilateralAuthoredPropRoot {
    pub root: WeaponRootTransform,
    pub position_residual_m: f32,
    pub angular_residual_deg: f32,
}

/// TLOU-style long-gun authority is bilateral: both authored hand-prop attachments describe the
/// same weapon/handle frame after `base -> add -> arms -> hands` composition. Runtime accepts that
/// contract only while the two authored frames remain coherent, then fuses their tiny compression /
/// sampling residual instead of arbitrarily choosing the firing side as the sole weapon authority.
#[inline]
pub(crate) fn weapon_root_from_bilateral_authored_prop_frames(
    presentation: &WeaponPresentationDefinition,
    right_frame: Mat4,
    left_frame: Mat4,
) -> Option<BilateralAuthoredPropRoot> {
    const MAX_POSITION_RESIDUAL_M: f32 = 0.010;
    const MAX_ANGULAR_RESIDUAL_DEG: f32 = 2.0;

    let right = weapon_root_from_authored_prop_frame(presentation, right_frame)?;
    let left = weapon_root_from_authored_prop_frame(presentation, left_frame)?;
    let right_handle_position = weapon_handle_position(presentation, right);
    let left_handle_position = weapon_handle_position(presentation, left);
    let right_handle_rotation = weapon_handle_rotation(presentation, right).normalize_or_identity();
    let mut left_handle_rotation = weapon_handle_rotation(presentation, left).normalize_or_identity();

    let position_residual_m = right_handle_position.distance(left_handle_position);
    let raw_dot = right_handle_rotation.dot(left_handle_rotation);
    let angular_residual_deg = (2.0 * raw_dot.abs().clamp(-1.0, 1.0).acos()).to_degrees();
    if !position_residual_m.is_finite()
        || !angular_residual_deg.is_finite()
        || position_residual_m > MAX_POSITION_RESIDUAL_M
        || angular_residual_deg > MAX_ANGULAR_RESIDUAL_DEG
    {
        return None;
    }

    // Quaternions q and -q encode the same orientation. Keep both endpoints in one hemisphere before
    // averaging so a valid authored pair cannot cancel itself numerically.
    if raw_dot < 0.0 {
        left_handle_rotation = Quat::from_xyzw(
            -left_handle_rotation.x,
            -left_handle_rotation.y,
            -left_handle_rotation.z,
            -left_handle_rotation.w,
        );
    }
    let handle_position = (right_handle_position + left_handle_position) * 0.5;
    let handle_rotation = right_handle_rotation
        .slerp(left_handle_rotation, 0.5)
        .normalize_or_identity();
    let rotation = (handle_rotation * handle_rotation_from_root(presentation).inverse())
        .normalize_or_identity();
    let position = weapon_root_position_from_handle(presentation, handle_position, rotation);
    if !position.is_finite() || !rotation.is_finite() {
        return None;
    }
    Some(BilateralAuthoredPropRoot {
        root: WeaponRootTransform { position, rotation },
        position_residual_m,
        angular_residual_deg,
    })
}

/// Inverse of `weapon_root_from_authored_prop_frame`: resolve the common authored prop/socket frame
/// that both hands must reproduce for a desired procedural weapon root.
#[inline]
pub(crate) fn authored_prop_frame_from_weapon_root(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Option<Mat4> {
    if !root.position.is_finite() || !root.rotation.is_finite() {
        return None;
    }
    let handle_position = weapon_handle_position(presentation, root);
    let handle_rotation = weapon_handle_rotation(presentation, root).normalize_or_identity();
    let socket_to_handle = q4(presentation.authored_socket_to_weapon_handle_basis);
    let socket_rotation = (handle_rotation * socket_to_handle.inverse()).normalize_or_identity();
    (handle_position.is_finite() && socket_rotation.is_finite()).then_some(
        Mat4::from_scale_rotation_translation(Vec3::ONE, socket_rotation, handle_position),
    )
}

#[allow(dead_code)]
pub(crate) fn weapon_root_from_right_palm(
    presentation: &WeaponPresentationDefinition,
    right_palm: Mat4,
) -> Option<WeaponRootTransform> {
    let presentation = presentation.clone().sanitized();
    if !presentation.enabled {
        return None;
    }
    let (scale, palm_rotation, palm_position) = right_palm.to_scale_rotation_translation();
    if !scale.is_finite()
        || scale.x <= 0.0
        || scale.y <= 0.0
        || scale.z <= 0.0
        || !palm_rotation.is_finite()
        || !palm_position.is_finite()
    {
        return None;
    }
    let rotation = weapon_rotation_from_palm(&presentation, palm_rotation);
    let handle_position = palm_position + palm_rotation * v3(presentation.right_palm_to_handle);
    Some(WeaponRootTransform {
        position: weapon_root_position_from_handle(&presentation, handle_position, rotation),
        rotation,
    })
}

/// Resolve the physical firing-grip anchor from an authored character palm frame without changing
/// weapon orientation. This is valid only after an authored long-gun hand pose has been applied.
/// Inverse of the hand-owned palm -> runtime-weapon orientation contract. FPP ADS may rotate the
/// weapon around the fixed firing handle; the real firing palm must then follow this exact inverse
/// transform rather than the separate third-person ReadyHold palm orientation.
#[inline]
pub(crate) fn weapon_hand_owned_right_palm_rotation(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Quat {
    (root.rotation * right_palm_to_runtime_weapon_rotation(presentation).inverse())
        .normalize_or_identity()
}

#[inline]
pub(crate) fn weapon_hand_owned_right_palm_position(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Vec3 {
    let palm_rotation = weapon_hand_owned_right_palm_rotation(presentation, root);
    weapon_handle_position(presentation, root)
        - palm_rotation * v3(presentation.right_palm_to_handle)
}

pub(crate) fn weapon_handle_anchor_from_right_palm(
    presentation: &WeaponPresentationDefinition,
    right_palm: Mat4,
) -> Option<Vec3> {
    let presentation = presentation.clone().sanitized();
    if !presentation.enabled {
        return None;
    }
    let (scale, rotation, position) = right_palm.to_scale_rotation_translation();
    if !scale.is_finite()
        || scale.x <= 0.0
        || scale.y <= 0.0
        || scale.z <= 0.0
        || !rotation.is_finite()
        || !position.is_finite()
    {
        return None;
    }
    let anchor =
        position + rotation.normalize_or_identity() * v3(presentation.right_palm_to_handle);
    anchor.is_finite().then_some(anchor)
}

/// Resolve the physical support-grip anchor from an authored character palm frame. The support
/// hand may influence weapon orientation, but never owns weapon translation.
pub(crate) fn weapon_left_grip_anchor_from_left_palm(
    presentation: &WeaponPresentationDefinition,
    left_palm: Mat4,
) -> Option<Vec3> {
    let presentation = presentation.clone().sanitized();
    if !presentation.enabled {
        return None;
    }
    let (scale, rotation, position) = left_palm.to_scale_rotation_translation();
    if !scale.is_finite()
        || scale.x <= 0.0
        || scale.y <= 0.0
        || scale.z <= 0.0
        || !rotation.is_finite()
        || !position.is_finite()
    {
        return None;
    }
    let anchor =
        position + rotation.normalize_or_identity() * v3(presentation.ready_left_palm_to_left_grip);
    anchor.is_finite().then_some(anchor)
}

#[inline]
pub(crate) fn weapon_rear_sight_position(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Vec3 {
    weapon_handle_position(presentation, root)
        + weapon_handle_rotation(presentation, root) * v3(presentation.ads_rear_sight_from_handle)
}

#[inline]
pub(crate) fn weapon_front_sight_position(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Vec3 {
    weapon_handle_position(presentation, root)
        + weapon_handle_rotation(presentation, root) * v3(presentation.ads_front_sight_from_handle)
}
