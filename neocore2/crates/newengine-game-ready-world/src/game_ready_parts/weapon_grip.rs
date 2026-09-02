use newengine_engine_runtime::gameplay::WeaponPresentationDefinition;
use newengine_math::{Mat3, Mat4, Quat, Vec3};

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

#[inline]
pub(crate) fn weapon_sight_forward(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Vec3 {
    (weapon_front_sight_position(presentation, root)
        - weapon_rear_sight_position(presentation, root))
    .normalize_or_zero()
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WeaponReadySolveContract {
    pub root: WeaponRootTransform,
    pub shoulder_pocket: Vec3,
    pub stock_contact: Vec3,
    pub right_elbow_pole: Vec3,
    pub left_elbow_pole: Vec3,
}

pub(crate) fn weapon_ready_solve_contract(
    presentation: &WeaponPresentationDefinition,
    chest: Mat4,
    right_shoulder: Mat4,
    left_shoulder: Mat4,
) -> Option<WeaponReadySolveContract> {
    weapon_ready_solve_contract_aimed(
        presentation,
        chest,
        right_shoulder,
        left_shoulder,
        None,
        0.0,
    )
}

pub(crate) fn weapon_ready_solve_contract_aimed(
    presentation: &WeaponPresentationDefinition,
    chest: Mat4,
    right_shoulder: Mat4,
    left_shoulder: Mat4,
    view_forward_model: Option<Vec3>,
    aim_alpha: f32,
) -> Option<WeaponReadySolveContract> {
    weapon_ready_solve_contract_presented(
        presentation,
        chest,
        right_shoulder,
        left_shoulder,
        view_forward_model,
        aim_alpha,
        0.0,
        0.0,
    )
}

pub(crate) fn weapon_ready_solve_contract_presented(
    presentation: &WeaponPresentationDefinition,
    chest: Mat4,
    right_shoulder: Mat4,
    left_shoulder: Mat4,
    view_forward_model: Option<Vec3>,
    aim_alpha: f32,
    fire_recoil_alpha: f32,
    fire_recoil_yaw_radians: f32,
) -> Option<WeaponReadySolveContract> {
    let presentation = presentation.clone().sanitized();
    if !presentation.enabled {
        return None;
    }
    let chest_position = chest.transform_point3(Vec3::ZERO);
    let right_position = right_shoulder.transform_point3(Vec3::ZERO);
    let left_position = left_shoulder.transform_point3(Vec3::ZERO);
    if !chest_position.is_finite() || !right_position.is_finite() || !left_position.is_finite() {
        return None;
    }
    let aim_alpha = if aim_alpha.is_finite() {
        aim_alpha.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let fire_recoil_alpha = if fire_recoil_alpha.is_finite() {
        fire_recoil_alpha.clamp(0.0, 4.0)
    } else {
        0.0
    };
    let fire_recoil_yaw_radians = if fire_recoil_yaw_radians.is_finite() {
        fire_recoil_yaw_radians.clamp(-0.5, 0.5)
    } else {
        0.0
    };
    let left_axis = (left_position - right_position).normalize_or_zero();
    let shoulder_mid = (left_position + right_position) * 0.5;
    let torso_up = (shoulder_mid - chest_position).normalize_or_zero();
    if left_axis.length_squared() < 1.0e-8 || torso_up.length_squared() < 1.0e-8 {
        return None;
    }
    let up_hint = (Vec3::Y * 0.85 + torso_up * 0.15).normalize_or_zero();
    let forward_axis = left_axis.cross(up_hint).normalize_or_zero();
    if forward_axis.length_squared() < 1.0e-8 {
        return None;
    }
    let up_axis = forward_axis.cross(left_axis).normalize_or_zero();
    let body_rotation =
        Quat::from_mat3(&Mat3::from_cols(left_axis, up_axis, forward_axis)).normalize_or_identity();
    // `ready_body_to_root_rotation` is authored in the weapon's native rig basis. Complete the
    // same native->runtime conversion used by palm-owned attachment before any contacts, ADS or
    // muzzle projection are evaluated. Omitting this term made native rifles with -Y-up appear
    // 180 degrees rolled around the barrel in torso-owned ReadyHold.
    let base_rotation = (body_rotation
        * q4(presentation.ready_body_to_root_rotation)
        * q4(presentation.native_rig_to_runtime_basis))
    .normalize_or_identity();
    let base_handle_rotation =
        (base_rotation * handle_rotation_from_root(&presentation)).normalize_or_identity();
    let local_sight_axis = (v3(presentation.ads_front_sight_from_handle)
        - v3(presentation.ads_rear_sight_from_handle))
    .normalize_or_zero();
    let rotation = view_forward_model
        .filter(|forward| forward.is_finite() && forward.length_squared() > 1.0e-8)
        .map(|forward| {
            let desired_forward = forward.normalize();
            let current_sight_axis = (base_handle_rotation * local_sight_axis).normalize_or_zero();
            if current_sight_axis.length_squared() <= 1.0e-8 {
                base_rotation
            } else {
                (Quat::from_rotation_arc(current_sight_axis, desired_forward) * base_rotation)
                    .normalize_or_identity()
            }
        })
        .unwrap_or(base_rotation);
    let rotation = if fire_recoil_alpha > 0.0 || fire_recoil_yaw_radians.abs() > 1.0e-6 {
        let pitch_axis = (body_rotation * Vec3::X).normalize_or_zero();
        let yaw_axis = (body_rotation * Vec3::Y).normalize_or_zero();
        let pitch = if pitch_axis.length_squared() > 1.0e-8 {
            Quat::from_axis_angle(
                pitch_axis,
                -presentation.fire_kick_pitch_radians * fire_recoil_alpha,
            )
        } else {
            Quat::IDENTITY
        };
        let yaw = if yaw_axis.length_squared() > 1.0e-8 {
            Quat::from_axis_angle(yaw_axis, fire_recoil_yaw_radians)
        } else {
            Quat::IDENTITY
        };
        (yaw * pitch * rotation).normalize_or_identity()
    } else {
        rotation
    };
    let shoulder_offset = v3(presentation.ready_shoulder_pocket_offset)
        .lerp(v3(presentation.ads_shoulder_pocket_offset), aim_alpha);
    let shoulder_pocket = right_position + body_rotation * shoulder_offset;
    let stock_from_root = v3(presentation.handle_from_root)
        + handle_rotation_from_root(&presentation) * v3(presentation.stock_contact_from_handle);
    let position = shoulder_pocket - rotation * stock_from_root;
    let root = WeaponRootTransform { position, rotation };
    let stock_contact = weapon_handle_position(&presentation, root)
        + weapon_handle_rotation(&presentation, root) * v3(presentation.stock_contact_from_handle);
    let right_elbow_pole =
        right_position + body_rotation * v3(presentation.ready_right_elbow_pole_offset);
    let left_elbow_pole =
        left_position + body_rotation * v3(presentation.ready_left_elbow_pole_offset);
    (position.is_finite()
        && rotation.is_finite()
        && shoulder_pocket.is_finite()
        && stock_contact.is_finite()
        && right_elbow_pole.is_finite()
        && left_elbow_pole.is_finite())
    .then_some(WeaponReadySolveContract {
        root,
        shoulder_pocket,
        stock_contact,
        right_elbow_pole,
        left_elbow_pole,
    })
}

/// Resolve the full-body first-person weapon root in camera/model space. This consumes the FPP
/// fields that are already authored in YTYP: hip handle offset, sight points, camera-to-rear-sight
/// offset and hip convergence distance. The result is still the ordinary world weapon entity;
/// both real arms are solved to this root afterwards, so there is no duplicate viewmodel.
pub(crate) fn weapon_first_person_solve_contract_presented(
    presentation: &WeaponPresentationDefinition,
    eye_position_model: Vec3,
    view_rotation_model: Quat,
    right_shoulder: Mat4,
    left_shoulder: Mat4,
    aim_alpha: f32,
    fire_recoil_alpha: f32,
    fire_recoil_yaw_radians: f32,
) -> Option<WeaponReadySolveContract> {
    let presentation = presentation.clone().sanitized();
    if !presentation.enabled || !eye_position_model.is_finite() || !view_rotation_model.is_finite()
    {
        return None;
    }
    let view_rotation = view_rotation_model.normalize_or_identity();
    let view_right = (view_rotation * Vec3::X).normalize_or_zero();
    let view_up = (view_rotation * Vec3::Y).normalize_or_zero();
    let view_forward = (view_rotation * -Vec3::Z).normalize_or_zero();
    if view_right.length_squared() <= 1.0e-8
        || view_up.length_squared() <= 1.0e-8
        || view_forward.length_squared() <= 1.0e-8
    {
        return None;
    }

    let right_position = right_shoulder.transform_point3(Vec3::ZERO);
    let left_position = left_shoulder.transform_point3(Vec3::ZERO);
    if !right_position.is_finite() || !left_position.is_finite() {
        return None;
    }

    let aim_alpha = if aim_alpha.is_finite() {
        aim_alpha.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let fire_recoil_alpha = if fire_recoil_alpha.is_finite() {
        fire_recoil_alpha.clamp(0.0, 4.0)
    } else {
        0.0
    };
    let fire_recoil_yaw_radians = if fire_recoil_yaw_radians.is_finite() {
        fire_recoil_yaw_radians.clamp(-0.5, 0.5)
    } else {
        0.0
    };

    // ReadyHold's canonical body frame is X=left, Y=up, Z=forward. Build the same frame from
    // the gameplay camera so native-rig basis correction stays identical between FPP and TPP.
    let presentation_frame = Quat::from_mat3(&Mat3::from_cols(-view_right, view_up, view_forward))
        .normalize_or_identity();
    let base_rotation = (presentation_frame
        * q4(presentation.ready_body_to_root_rotation)
        * q4(presentation.native_rig_to_runtime_basis))
    .normalize_or_identity();
    let base_handle_rotation =
        (base_rotation * handle_rotation_from_root(&presentation)).normalize_or_identity();

    let rear_from_handle = v3(presentation.ads_rear_sight_from_handle);
    let front_from_handle = v3(presentation.ads_front_sight_from_handle);
    let local_sight_axis = (front_from_handle - rear_from_handle).normalize_or_zero();
    let hip_handle = eye_position_model
        + view_rotation * v3(presentation.first_person_full_body_hip_handle_offset);
    if !hip_handle.is_finite() {
        return None;
    }
    let convergence_distance = presentation.first_person_hip_convergence_m.max(0.1);
    let convergence_point = eye_position_model + view_forward * convergence_distance;
    let hip_rear_estimate = hip_handle + base_handle_rotation * rear_from_handle;
    let hip_forward = (convergence_point - hip_rear_estimate).normalize_or_zero();
    let hip_forward = if hip_forward.length_squared() > 1.0e-8 {
        hip_forward
    } else {
        view_forward
    };
    let desired_sight_forward = hip_forward
        .lerp(view_forward, aim_alpha)
        .normalize_or_zero();
    let rotation = if local_sight_axis.length_squared() > 1.0e-8 {
        let current_sight_axis = (base_handle_rotation * local_sight_axis).normalize_or_zero();
        if current_sight_axis.length_squared() > 1.0e-8
            && desired_sight_forward.length_squared() > 1.0e-8
        {
            (Quat::from_rotation_arc(current_sight_axis, desired_sight_forward) * base_rotation)
                .normalize_or_identity()
        } else {
            base_rotation
        }
    } else {
        base_rotation
    };

    // Camera recoil and weapon recoil are separate layers in REDengine. This root carries the
    // weapon-side impulse only; the camera runtime may apply its own smaller additive response.
    let rotation = if fire_recoil_alpha > 0.0 || fire_recoil_yaw_radians.abs() > 1.0e-6 {
        let pitch = Quat::from_axis_angle(
            view_right,
            -presentation.fire_kick_pitch_radians * fire_recoil_alpha,
        );
        let yaw = Quat::from_axis_angle(view_up, fire_recoil_yaw_radians);
        (yaw * pitch * rotation).normalize_or_identity()
    } else {
        rotation
    };

    let handle_from_root = v3(presentation.handle_from_root);
    let handle_rotation_from_root = handle_rotation_from_root(&presentation);
    let hip_root_position = hip_handle - rotation * handle_from_root;
    let ads_rear_target =
        eye_position_model + view_rotation * v3(presentation.ads_camera_to_rear_sight);
    let ads_root_position = ads_rear_target
        - rotation * (handle_from_root + handle_rotation_from_root * rear_from_handle);
    let position = hip_root_position.lerp(ads_root_position, aim_alpha);
    let root = WeaponRootTransform { position, rotation };

    let shoulder_offset = v3(presentation.ready_shoulder_pocket_offset)
        .lerp(v3(presentation.ads_shoulder_pocket_offset), aim_alpha);
    let shoulder_pocket = right_position + presentation_frame * shoulder_offset;
    let stock_contact = weapon_handle_position(&presentation, root)
        + weapon_handle_rotation(&presentation, root) * v3(presentation.stock_contact_from_handle);
    let right_elbow_pole =
        right_position + presentation_frame * v3(presentation.ready_right_elbow_pole_offset);
    let left_elbow_pole =
        left_position + presentation_frame * v3(presentation.ready_left_elbow_pole_offset);

    (position.is_finite()
        && rotation.is_finite()
        && shoulder_pocket.is_finite()
        && stock_contact.is_finite()
        && right_elbow_pole.is_finite()
        && left_elbow_pole.is_finite())
    .then_some(WeaponReadySolveContract {
        root,
        shoulder_pocket,
        stock_contact,
        right_elbow_pole,
        left_elbow_pole,
    })
}

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
#[cfg(test)]
#[path = "weapon_grip/tests.rs"]
mod tests;
