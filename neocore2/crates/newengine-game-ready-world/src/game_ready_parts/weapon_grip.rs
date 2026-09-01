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
fn weapon_rotation_from_palm(
    presentation: &WeaponPresentationDefinition,
    palm_rotation: Quat,
) -> Quat {
    (palm_rotation
        * q4(presentation.right_palm_to_native_rig)
        * q4(presentation.native_rig_to_runtime_basis))
    .normalize_or_identity()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WeaponRootTransform {
    pub position: Vec3,
    pub rotation: Quat,
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
        position: handle_position - rotation * v3(presentation.handle_from_root),
        rotation,
    })
}

/// Resolve the physical firing-grip anchor from an authored character palm frame without changing
/// weapon orientation. This is valid only after an authored long-gun hand pose has been applied.
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
        + root.rotation * v3(presentation.ads_rear_sight_from_handle)
}

#[inline]
pub(crate) fn weapon_front_sight_position(
    presentation: &WeaponPresentationDefinition,
    root: WeaponRootTransform,
) -> Vec3 {
    weapon_handle_position(presentation, root)
        + root.rotation * v3(presentation.ads_front_sight_from_handle)
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

    let position = handle_anchor - rotation * v3(presentation.handle_from_root);
    (position.is_finite() && rotation.is_finite()).then_some(WeaponRootTransform {
        position,
        rotation,
    })
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
    let local_sight_axis = (v3(presentation.ads_front_sight_from_handle)
        - v3(presentation.ads_rear_sight_from_handle))
    .normalize_or_zero();
    let rotation = view_forward_model
        .filter(|forward| forward.is_finite() && forward.length_squared() > 1.0e-8)
        .map(|forward| {
            let desired_forward = forward.normalize();
            let current_sight_axis = (base_rotation * local_sight_axis).normalize_or_zero();
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
    let stock_from_root =
        v3(presentation.handle_from_root) + v3(presentation.stock_contact_from_handle);
    let position = shoulder_pocket - rotation * stock_from_root;
    let root = WeaponRootTransform { position, rotation };
    let stock_contact = weapon_handle_position(&presentation, root)
        + rotation * v3(presentation.stock_contact_from_handle);
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
    let hip_rear_estimate = hip_handle + base_rotation * rear_from_handle;
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
        let current_sight_axis = (base_rotation * local_sight_axis).normalize_or_zero();
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
    let hip_root_position = hip_handle - rotation * handle_from_root;
    let ads_rear_target =
        eye_position_model + view_rotation * v3(presentation.ads_camera_to_rear_sight);
    let ads_root_position = ads_rear_target - rotation * (handle_from_root + rear_from_handle);
    let position = hip_root_position.lerp(ads_root_position, aim_alpha);
    let root = WeaponRootTransform { position, rotation };

    let shoulder_offset = v3(presentation.ready_shoulder_pocket_offset)
        .lerp(v3(presentation.ads_shoulder_pocket_offset), aim_alpha);
    let shoulder_pocket = right_position + presentation_frame * shoulder_offset;
    let stock_contact = weapon_handle_position(&presentation, root)
        + rotation * v3(presentation.stock_contact_from_handle);
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
            handle - contract.root.rotation * v3(presentation.handle_from_root);
        contract.stock_contact =
            handle + contract.root.rotation * v3(presentation.stock_contact_from_handle);
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
            let local_right = (contract.root.rotation * Vec3::X).normalize_or_zero();
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
        handle_anchor - contract.root.rotation * v3(presentation.handle_from_root);
    contract.stock_contact =
        handle_anchor + contract.root.rotation * v3(presentation.stock_contact_from_handle);

    // Shoulder stock is a soft rotational constraint, never a positional owner. At full ADS the
    // view axis has priority, so the shoulder correction is intentionally weaker.
    let stock_vector = contract.root.rotation * v3(presentation.stock_contact_from_handle);
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
        let grip_vector = contract.root.rotation * v3(presentation.left_grip_from_handle);
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
        let local_right = (contract.root.rotation * Vec3::X).normalize_or_zero();
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
    let position = handle - rotation * v3(presentation.handle_from_root);
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
    contract.stock_contact =
        handle + contract.root.rotation * v3(presentation.stock_contact_from_handle);
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
        + root.rotation * v3(presentation.left_grip_from_handle)
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
mod tests {
    use super::*;

    fn fixture() -> WeaponPresentationDefinition {
        WeaponPresentationDefinition {
            enabled: true,
            handle_from_root: [0.0, 0.01, -0.03],
            muzzle_from_root: [0.1, 0.04, 0.64],
            left_grip_from_handle: [-0.02, 0.04, 0.30],
            stock_contact_from_handle: [-0.02, 0.05, -0.34],
            ready_body_to_root_rotation: [0.036, 0.608, -0.041, 0.792],
            ready_left_palm_to_left_grip: [0.003, 0.101, 0.006],
            ready_right_palm_to_weapon: [-0.656, 0.722, 0.174, 0.133],
            ready_left_palm_to_weapon: [-0.023, -0.459, -0.303, 0.835],
            right_palm_to_handle: [0.019, 0.033, -0.083],
            first_person_hip_handle_offset: [0.205, -0.205, -0.58],
            first_person_full_body_hip_handle_offset: [0.205, -0.205, -0.08],
            ads_rear_sight_from_handle: [0.0, -0.058, 0.235],
            ads_front_sight_from_handle: [0.0, -0.070, 0.640],
            ads_camera_to_rear_sight: [0.0, 0.0, -0.075],
            ..WeaponPresentationDefinition::default()
        }
        .sanitized()
    }

    #[test]
    fn authored_weapon_ready_contract_keeps_stock_on_shoulder() {
        let p = fixture();
        let chest = Mat4::from_translation(Vec3::new(0.0, 1.25, 0.0));
        let right = Mat4::from_translation(Vec3::new(-0.20, 1.48, 0.02));
        let left = Mat4::from_translation(Vec3::new(0.20, 1.48, 0.02));
        let contract = weapon_ready_solve_contract(&p, chest, right, left).expect("contract");
        assert!((contract.stock_contact - contract.shoulder_pocket).length() < 1.0e-5);
    }

    #[test]
    fn authored_weapon_handle_anchor_owns_readyhold_translation() {
        let p = fixture();
        let chest = Mat4::from_translation(Vec3::new(0.0, 1.25, 0.0));
        let right = Mat4::from_translation(Vec3::new(-0.20, 1.48, 0.02));
        let left = Mat4::from_translation(Vec3::new(0.20, 1.48, 0.02));
        let contract = weapon_ready_solve_contract(&p, chest, right, left).expect("contract");
        let anchor = Vec3::new(-0.31, 1.36, 0.41);
        let anchored =
            weapon_ready_contract_with_contacts(&p, contract, Some(anchor), None, 0.0, 0.0)
                .expect("anchored contract");
        let handle = weapon_handle_position(&p, anchored.root);
        assert!((handle - anchor).length() < 1.0e-5);
    }

    #[test]
    fn readyhold_applies_native_rig_to_runtime_basis_exactly_once() {
        let mut p = fixture();
        p.ready_body_to_root_rotation = [0.0, 0.0, 0.0, 1.0];
        p.native_rig_to_runtime_basis = [0.0, 0.0, 1.0, 0.0];
        p.ready_shoulder_pocket_offset = [0.0, 0.0, 0.0];
        p.ads_shoulder_pocket_offset = [0.0, 0.0, 0.0];

        let chest = Mat4::from_translation(Vec3::new(0.0, 1.20, 0.0));
        let right = Mat4::from_translation(Vec3::new(-0.20, 1.45, 0.0));
        let left = Mat4::from_translation(Vec3::new(0.20, 1.45, 0.0));
        let contract = weapon_ready_solve_contract(&p, chest, right, left).expect("ReadyHold");

        let forward = (contract.root.rotation * Vec3::Z).normalize_or_zero();
        let up = (contract.root.rotation * Vec3::Y).normalize_or_zero();
        let right_axis = (contract.root.rotation * Vec3::X).normalize_or_zero();
        assert!(forward.dot(Vec3::Z) > 0.9999);
        assert!(up.dot(-Vec3::Y) > 0.9999);
        assert!(right_axis.dot(-Vec3::X) > 0.9999);
    }

    #[test]
    fn authored_contact_constraints_never_break_firing_handle_anchor() {
        let p = fixture();
        let chest = Mat4::from_translation(Vec3::new(0.0, 1.25, 0.0));
        let right = Mat4::from_translation(Vec3::new(-0.20, 1.48, 0.02));
        let left = Mat4::from_translation(Vec3::new(0.20, 1.48, 0.02));
        let raw = weapon_ready_solve_contract(&p, chest, right, left).expect("raw contract");
        let handle_anchor = Vec3::new(-0.25, 1.31, 0.18);
        let support_anchor = Vec3::new(0.02, 1.28, 0.37);
        let constrained = weapon_ready_contract_with_contacts(
            &p,
            raw,
            Some(handle_anchor),
            Some(support_anchor),
            0.35,
            0.0,
        )
        .expect("contact contract");
        let handle = weapon_handle_position(&p, constrained.root);
        assert!(
            (handle - handle_anchor).length() < 1.0e-5,
            "firing hand is the primary kinematic joint"
        );
    }

    #[test]
    fn secondary_long_gun_rotation_preserves_exact_firing_handle_contact() {
        let p = fixture();
        let root = WeaponRootTransform {
            position: Vec3::new(-0.2, 1.3, 0.1),
            rotation: Quat::from_rotation_y(0.35),
        };
        let handle_before = weapon_handle_position(&p, root);
        let dynamic = weapon_root_with_secondary_rotation(
            &p,
            root,
            Vec3::new(
                2.5_f32.to_radians(),
                -3.0_f32.to_radians(),
                1.0_f32.to_radians(),
            ),
        )
        .expect("secondary root");
        let handle_after = weapon_handle_position(&p, dynamic);
        assert!(
            (handle_after - handle_before).length() < 1.0e-6,
            "secondary dynamics may rotate around the firing hand but may never detach from it"
        );
        assert!(root.rotation.dot(dynamic.rotation).abs() < 0.999_999);
    }

    #[test]
    fn full_body_fpp_hand_anchored_ads_preserves_grip_and_aligns_real_sights() {
        let p = fixture();
        let palm_rotation = Quat::from_euler(newengine_math::EulerRot::YXZ, -0.24, 0.18, 0.11);
        let palm_position = Vec3::new(-0.19, 1.36, -0.08);
        let palm = Mat4::from_rotation_translation(palm_rotation, palm_position);
        let view = Quat::from_euler(newengine_math::EulerRot::YXZ, 0.37, -0.18, 0.0);
        let root = weapon_first_person_hand_anchored_root(&p, palm, view, 1.0, 0.0, 0.0)
            .expect("hand-anchored ADS root");
        let expected_handle = weapon_handle_anchor_from_right_palm(&p, palm).expect("handle");
        let actual_handle = weapon_handle_position(&p, root);
        assert!(
            actual_handle.distance(expected_handle) <= 1.0e-5,
            "ADS may rotate around the firing grip but never translate it"
        );
        let sight_forward = weapon_sight_forward(&p, root);
        let view_forward = (view * -Vec3::Z).normalize_or_zero();
        assert!(
            sight_forward.dot(view_forward) > 0.9999,
            "rear->front sight axis must coincide with gameplay view at full ADS"
        );
    }

    #[test]
    fn full_body_fpp_hip_keeps_exact_authored_palm_weapon_transform() {
        let p = fixture();
        let palm_rotation = Quat::from_euler(newengine_math::EulerRot::YXZ, -0.24, 0.18, 0.11);
        let palm_position = Vec3::new(-0.19, 1.36, -0.08);
        let palm = Mat4::from_rotation_translation(palm_rotation, palm_position);
        let authored = weapon_root_from_right_palm(&p, palm).expect("authored palm root");
        let resolved = weapon_first_person_hand_anchored_root(
            &p,
            palm,
            Quat::from_rotation_y(0.8),
            0.0,
            0.0,
            0.0,
        )
        .expect("hip root");
        assert!(authored.position.distance(resolved.position) <= 1.0e-6);
        assert!(authored.rotation.dot(resolved.rotation).abs() > 0.999_999);
    }

    #[test]
    fn first_person_full_body_hip_consumes_authored_anatomical_handle_offset() {
        let p = fixture();
        let eye = Vec3::new(0.0, 1.62, 0.0);
        let view = Quat::IDENTITY;
        let right = Mat4::from_translation(Vec3::new(-0.20, 1.46, 0.0));
        let left = Mat4::from_translation(Vec3::new(0.20, 1.46, 0.0));
        let contract =
            weapon_first_person_solve_contract_presented(&p, eye, view, right, left, 0.0, 0.0, 0.0)
                .expect("FPP hip contract");
        let handle = weapon_handle_position(&p, contract.root);
        let expected = eye + view * v3(p.first_person_full_body_hip_handle_offset);
        assert!((handle - expected).length() <= 1.0e-5);
    }

    #[test]
    fn first_person_full_body_hip_keeps_bilateral_abby_rifle_contacts_reachable() {
        let p = fixture();
        let eye = Vec3::new(0.0, 1.62, 0.0);
        let view = Quat::IDENTITY;
        let right_shoulder = Vec3::new(-0.087_309, 1.346_941, -0.117_141);
        let left_shoulder = Vec3::new(0.087_361, 1.345_958, 0.137_559);
        let right = Mat4::from_translation(right_shoulder);
        let left = Mat4::from_translation(left_shoulder);
        let contract =
            weapon_first_person_solve_contract_presented(&p, eye, view, right, left, 0.0, 0.0, 0.0)
                .expect("full-body FPP hip contract");
        let right_target = weapon_ready_right_palm_position(&p, contract.root);
        let left_target = weapon_ready_left_palm_position(&p, contract.root);
        // Abby's authored upper+lower arm reaches about 0.520 m to the wrist. Allow the ~1 cm
        // wrist->palm segment, but retain the solver's 6 mm safety margin. This regression guards
        // against reusing a distant viewmodel offset for anatomical full-body hands.
        let max_palm_reach = 0.519_775_4 + 0.010_1 - 0.006;
        assert!(
            right_target.distance(right_shoulder) < max_palm_reach,
            "firing hand must remain anatomically reachable"
        );
        assert!(
            left_target.distance(left_shoulder) < max_palm_reach,
            "support hand must remain anatomically reachable"
        );
    }

    #[test]
    fn first_person_ads_places_rear_sight_at_authored_camera_offset() {
        let p = fixture();
        let eye = Vec3::new(0.0, 1.62, 0.0);
        let view = Quat::from_euler(newengine_math::EulerRot::YXZ, 0.37, -0.18, 0.0);
        let right = Mat4::from_translation(Vec3::new(-0.20, 1.46, 0.0));
        let left = Mat4::from_translation(Vec3::new(0.20, 1.46, 0.0));
        let contract =
            weapon_first_person_solve_contract_presented(&p, eye, view, right, left, 1.0, 0.0, 0.0)
                .expect("FPP ADS contract");
        let handle = weapon_handle_position(&p, contract.root);
        let rear = handle + contract.root.rotation * v3(p.ads_rear_sight_from_handle);
        let expected = eye + view * v3(p.ads_camera_to_rear_sight);
        assert!((rear - expected).length() <= 1.0e-5);
        let sight = (contract.root.rotation
            * (v3(p.ads_front_sight_from_handle) - v3(p.ads_rear_sight_from_handle)))
        .normalize_or_zero();
        let forward = (view * -Vec3::Z).normalize_or_zero();
        assert!(sight.dot(forward) > 0.9999);
    }

    #[test]
    fn aim_blocked_pivots_barrel_without_translating_firing_handle() {
        let p = fixture();
        let chest = Mat4::from_translation(Vec3::new(0.0, 1.25, 0.0));
        let right = Mat4::from_translation(Vec3::new(-0.20, 1.48, 0.02));
        let left = Mat4::from_translation(Vec3::new(0.20, 1.48, 0.02));
        let raw = weapon_ready_solve_contract(&p, chest, right, left).expect("raw contract");
        let handle_anchor = Vec3::new(-0.25, 1.31, 0.18);
        let clear =
            weapon_ready_contract_with_contacts(&p, raw, Some(handle_anchor), None, 0.0, 0.0)
                .expect("clear contract");
        let blocked =
            weapon_ready_contract_with_contacts(&p, raw, Some(handle_anchor), None, 0.0, 0.8)
                .expect("blocked contract");
        let clear_handle = weapon_handle_position(&p, clear.root);
        let blocked_handle = weapon_handle_position(&p, blocked.root);
        assert!((clear_handle - handle_anchor).length() < 1.0e-5);
        assert!((blocked_handle - handle_anchor).length() < 1.0e-5);
        let clear_forward = weapon_muzzle_forward(clear.root);
        let blocked_forward = weapon_muzzle_forward(blocked.root);
        assert!(
            clear_forward.dot(blocked_forward) < 0.98,
            "aim-blocked must visibly pivot the barrel"
        );
    }
    #[test]
    fn first_person_recoil_consumes_full_authored_weapon_kick_independent_of_camera_angle() {
        let mut p = fixture();
        p.fire_kick_pitch_radians = 5.0_f32.to_radians();
        let eye = Vec3::new(0.0, 1.62, 0.0);
        let view = Quat::IDENTITY;
        let right = Mat4::from_translation(Vec3::new(-0.20, 1.46, 0.0));
        let left = Mat4::from_translation(Vec3::new(0.20, 1.46, 0.0));
        let clear =
            weapon_first_person_solve_contract_presented(&p, eye, view, right, left, 0.0, 0.0, 0.0)
                .expect("clear FPP contract");
        let kicked =
            weapon_first_person_solve_contract_presented(&p, eye, view, right, left, 0.0, 1.0, 0.0)
                .expect("recoil FPP contract");
        let dot = clear
            .root
            .rotation
            .dot(kicked.root.rotation)
            .abs()
            .clamp(0.0, 1.0);
        let delta_degrees = (2.0 * dot.acos()).to_degrees();
        assert!(
            (delta_degrees - 5.0).abs() < 0.05,
            "weapon-space recoil must consume the full authored 5 degree kick, got {delta_degrees}"
        );
    }
}
