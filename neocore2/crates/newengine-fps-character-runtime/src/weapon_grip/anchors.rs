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
