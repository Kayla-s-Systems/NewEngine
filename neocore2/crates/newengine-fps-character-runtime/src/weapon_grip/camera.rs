#[inline]
pub(crate) fn weapon_sight_forward(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Vec3 {
    (weapon_front_sight_position(presentation, root)
        - weapon_rear_sight_position(presentation, root))
    .normalize_or_zero()
}

/// Rotate an already-authored third-person weapon root around its physical firing handle until the
/// rendered rear->front sight axis follows the gameplay view. Translation ownership stays with the
/// character-authored handle; only orientation is transferred from camera intent.
///
/// This is deliberately root-in/root-out: native character rigs may have a prop socket that is a
/// sibling of the anatomical palm, so the arm solver must consume the resulting root rather than
/// inventing a separate camera-space attachment.
#[cfg(test)]
pub(crate) fn weapon_sight_aligned_root_around_handle(
    presentation: &WeaponPresentationDefinition,
    authored: WeaponRootTransform,
    view_forward_model: Vec3,
    aim_alpha: f32,
) -> Option<WeaponRootTransform> {
    let presentation = presentation.clone().sanitized();
    if !presentation.enabled
        || !authored.position.is_finite()
        || !authored.rotation.is_finite()
        || !view_forward_model.is_finite()
    {
        return None;
    }
    let view_forward = view_forward_model.normalize_or_zero();
    let sight_forward = weapon_sight_forward(&presentation, authored);
    if view_forward.length_squared() <= 1.0e-8 || sight_forward.length_squared() <= 1.0e-8 {
        return None;
    }
    let aim_alpha = if aim_alpha.is_finite() {
        aim_alpha.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let full_rotation = (Quat::from_rotation_arc(sight_forward, view_forward) * authored.rotation)
        .normalize_or_identity();
    let rotation = authored
        .rotation
        .slerp(full_rotation, aim_alpha)
        .normalize_or_identity();
    let handle = weapon_handle_position(&presentation, authored);
    let position = weapon_root_position_from_handle(&presentation, handle, rotation);
    (position.is_finite() && rotation.is_finite())
        .then_some(WeaponRootTransform { position, rotation })
}

/// Rotate a third-person rifle around its current stock/shoulder contact. This is the native RMB
/// free-aim pivot: both anatomical hands and the muzzle move with mouse/view delta while the stock
/// remains planted instead of spinning the weapon inside a fixed firing palm.
#[inline]
pub(crate) fn weapon_sight_aligned_root_around_stock_contact(
    presentation: &WeaponPresentationDefinition,
    authored: WeaponRootTransform,
    target_sight_forward_model: Vec3,
) -> Option<WeaponRootTransform> {
    let presentation = presentation.clone().sanitized();
    if !presentation.enabled
        || !authored.position.is_finite()
        || !authored.rotation.is_finite()
        || !target_sight_forward_model.is_finite()
    {
        return None;
    }
    let target_forward = target_sight_forward_model.normalize_or_zero();
    let sight_forward = weapon_sight_forward(&presentation, authored);
    if target_forward.length_squared() <= 1.0e-8 || sight_forward.length_squared() <= 1.0e-8 {
        return None;
    }

    let rotation = (Quat::from_rotation_arc(sight_forward, target_forward) * authored.rotation)
        .normalize_or_identity();
    let handle_rotation_local = handle_rotation_from_root(&presentation);
    let root_to_stock = v3(presentation.handle_from_root)
        + handle_rotation_local * v3(presentation.stock_contact_from_handle);
    let stock_contact = authored.position + authored.rotation * root_to_stock;
    let position = stock_contact - rotation * root_to_stock;
    (position.is_finite() && rotation.is_finite())
        .then_some(WeaponRootTransform { position, rotation })
}

/// Camera origin required by the authored eye-relief vector for a rendered weapon root. This is
/// intentionally derived from the actual rear sight after all weapon presentation transforms.
pub(crate) fn weapon_ads_camera_position(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
    view_rotation_ws: Quat,
) -> Option<Vec3> {
    if !view_rotation_ws.is_finite() {
        return None;
    }
    let presentation = presentation.clone().sanitized();
    if !presentation.enabled {
        return None;
    }
    let rear = weapon_rear_sight_position(&presentation, root);
    let offset = v3(presentation.ads_camera_to_rear_sight);
    let camera = rear - view_rotation_ws.normalize_or_identity() * offset;
    camera.is_finite().then_some(camera)
}

/// Resolve the weapon-authored ADS camera translation policy against the stable anatomical eye.
/// The weapon owns per-axis translation weights; camera runtime receives only the final anchor and
/// remains agnostic to weapon family/sight geometry. A zero weight preserves the anatomical eye
/// component, while one consumes the complete rendered-weapon eye-relief component.
pub(crate) fn weapon_resolved_ads_camera_position(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
    view_rotation_ws: Quat,
    eye_center_ws: Vec3,
) -> Option<Vec3> {
    if !eye_center_ws.is_finite() {
        return None;
    }
    let presentation = presentation.clone().sanitized();
    let raw = weapon_ads_camera_position(&presentation, root, view_rotation_ws)?;
    let weight = v3(presentation.ads_camera_translation_weight);
    let delta = raw - eye_center_ws;
    let resolved =
        eye_center_ws + Vec3::new(delta.x * weight.x, delta.y * weight.y, delta.z * weight.z);
    resolved.is_finite().then_some(resolved)
}

/// Full-body first person keeps the authored firing hand as the physical grip owner. Hip/ready
/// therefore uses the exact authored palm->weapon transform. While ADS is engaged, only the weapon
/// orientation rotates around that fixed handle until the real rear->front sight axis matches the
/// input-owned camera forward. The arm pose itself is never stretched toward a camera-space root.
pub(crate) fn weapon_first_person_hand_anchored_root(
    presentation: &WeaponPresentationDefinition,
    right_palm: Mat4,
    view_rotation_model: Quat,
    aim_alpha: f32,
    fire_recoil_alpha: f32,
    fire_recoil_yaw_radians: f32,
) -> Option<WeaponRootTransform> {
    let presentation = presentation.clone().sanitized();
    if !presentation.enabled || !view_rotation_model.is_finite() {
        return None;
    }
    let authored = weapon_root_from_right_palm(&presentation, right_palm)?;
    let handle_anchor = weapon_handle_anchor_from_right_palm(&presentation, right_palm)?;
    let view_rotation = view_rotation_model.normalize_or_identity();
    let view_forward = (view_rotation * -Vec3::Z).normalize_or_zero();
    let view_right = (view_rotation * Vec3::X).normalize_or_zero();
    let view_up = (view_rotation * Vec3::Y).normalize_or_zero();
    if view_forward.length_squared() <= 1.0e-8
        || view_right.length_squared() <= 1.0e-8
        || view_up.length_squared() <= 1.0e-8
    {
        return None;
    }

    let aim_alpha = if aim_alpha.is_finite() {
        aim_alpha.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let sight_forward = weapon_sight_forward(&presentation, authored);
    let ads_rotation = if sight_forward.length_squared() > 1.0e-8 {
        (Quat::from_rotation_arc(sight_forward, view_forward) * authored.rotation)
            .normalize_or_identity()
    } else {
        authored.rotation
    };
    let mut rotation = authored
        .rotation
        .slerp(ads_rotation, aim_alpha)
        .normalize_or_identity();

    let recoil_alpha = if fire_recoil_alpha.is_finite() {
        fire_recoil_alpha.clamp(0.0, 4.0)
    } else {
        0.0
    };
    let recoil_yaw = if fire_recoil_yaw_radians.is_finite() {
        fire_recoil_yaw_radians.clamp(-0.5, 0.5)
    } else {
        0.0
    };
    if recoil_alpha > 0.0 || recoil_yaw.abs() > 1.0e-6 {
        let pitch = Quat::from_axis_angle(
            view_right,
            -presentation.fire_kick_pitch_radians * recoil_alpha,
        );
        let yaw = Quat::from_axis_angle(view_up, recoil_yaw);
        rotation = (yaw * pitch * rotation).normalize_or_identity();
    }

    let position = weapon_root_position_from_handle(&presentation, handle_anchor, rotation);
    (position.is_finite() && rotation.is_finite())
        .then_some(WeaponRootTransform { position, rotation })
}
