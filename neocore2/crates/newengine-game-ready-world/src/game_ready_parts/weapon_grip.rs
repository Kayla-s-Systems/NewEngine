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
        fire_recoil_alpha.clamp(0.0, 1.0)
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

pub(crate) fn weapon_root_from_first_person_view(
    presentation: &WeaponPresentationDefinition,
    camera_position: Vec3,
    view_rotation: Quat,
    aim_alpha: f32,
) -> Option<WeaponRootTransform> {
    let presentation = presentation.clone().sanitized();
    if !presentation.enabled || !camera_position.is_finite() || !view_rotation.is_finite() {
        return None;
    }
    let view_rotation = view_rotation.normalize_or_identity();
    let aim_alpha = if aim_alpha.is_finite() {
        aim_alpha.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let camera_forward = (view_rotation * -Vec3::Z).normalize_or_zero();
    if camera_forward.length_squared() <= 1.0e-8 {
        return None;
    }
    // First-person has no independent authored weapon-orientation correction. Build a stable
    // camera mount for the canonical weapon frame (+Y up, +Z forward), then compose the same
    // native-rig -> runtime basis used by third-person, grip, muzzle and ADS.
    let camera_up = (view_rotation * Vec3::Y).normalize_or_zero();
    if camera_up.length_squared() <= 1.0e-8 {
        return None;
    }
    let canonical_right = camera_up.cross(camera_forward).normalize_or_zero();
    if canonical_right.length_squared() <= 1.0e-8 {
        return None;
    }
    let canonical_up = camera_forward.cross(canonical_right).normalize_or_zero();
    if canonical_up.length_squared() <= 1.0e-8 {
        return None;
    }
    let camera_mount = Quat::from_mat3(&Mat3::from_cols(
        canonical_right,
        canonical_up,
        camera_forward,
    ))
    .normalize_or_identity();
    let native_to_runtime = q4(presentation.native_rig_to_runtime_basis);
    let runtime_to_native = native_to_runtime.inverse();
    let base_rotation = (camera_mount * native_to_runtime).normalize_or_identity();
    let hip_handle_position =
        camera_position + view_rotation * v3(presentation.first_person_hip_handle_offset);
    let hip_target = camera_position + camera_forward * presentation.first_person_hip_convergence_m;
    let hip_forward = (hip_target - hip_handle_position).normalize_or_zero();
    if hip_forward.length_squared() <= 1.0e-8 {
        return None;
    }
    let native_forward = (runtime_to_native * Vec3::Z).normalize_or_zero();
    let hip_base_forward = (base_rotation * native_forward).normalize_or_zero();
    let hip_rotation = (Quat::from_rotation_arc(hip_base_forward, hip_forward) * base_rotation)
        .normalize_or_identity();
    let local_ads_axis = (v3(presentation.ads_front_sight_from_handle)
        - v3(presentation.ads_rear_sight_from_handle))
    .normalize_or_zero();
    if local_ads_axis.length_squared() <= 1.0e-8 {
        return None;
    }
    let world_ads_axis = (base_rotation * local_ads_axis).normalize_or_zero();
    if world_ads_axis.length_squared() <= 1.0e-8 {
        return None;
    }
    let ads_rotation = (Quat::from_rotation_arc(world_ads_axis, camera_forward) * base_rotation)
        .normalize_or_identity();
    let ads_rear_sight_world =
        camera_position + view_rotation * v3(presentation.ads_camera_to_rear_sight);
    let ads_handle_position =
        ads_rear_sight_world - ads_rotation * v3(presentation.ads_rear_sight_from_handle);
    let rotation = hip_rotation
        .slerp(ads_rotation, aim_alpha)
        .normalize_or_identity();
    let handle_position = hip_handle_position.lerp(ads_handle_position, aim_alpha);
    Some(WeaponRootTransform {
        position: handle_position - rotation * v3(presentation.handle_from_root),
        rotation,
    })
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
            ready_right_palm_to_weapon: [-0.656, 0.722, 0.174, 0.133],
            ready_left_palm_to_weapon: [-0.023, -0.459, -0.303, 0.835],
            right_palm_to_handle: [0.019, 0.033, -0.083],
            first_person_hip_handle_offset: [0.205, -0.205, -0.58],
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
    fn first_person_mount_preserves_canonical_weapon_up() {
        let mut p = fixture();
        p.native_rig_to_runtime_basis = [0.0, 0.0, 0.0, 1.0];
        let camera = Vec3::new(0.0, 1.7, 0.0);
        let view = Quat::IDENTITY;
        let root = weapon_root_from_first_person_view(&p, camera, view, 0.0).unwrap();
        let forward = (root.rotation * Vec3::Z).normalize_or_zero();
        let up = (root.rotation * Vec3::Y).normalize_or_zero();
        assert!(forward.dot(-Vec3::Z) > 0.999);
        assert!(up.dot(Vec3::Y) > 0.999);
    }

    #[test]
    fn first_person_mount_tracks_camera_up_without_roll_inversion() {
        let mut p = fixture();
        p.native_rig_to_runtime_basis = [0.0, 0.0, 0.0, 1.0];
        let camera = Vec3::new(0.0, 1.7, 0.0);
        let view =
            (Quat::from_rotation_y(0.43) * Quat::from_rotation_x(-0.18)).normalize_or_identity();
        let root = weapon_root_from_first_person_view(&p, camera, view, 0.0).unwrap();
        let expected_forward = (view * -Vec3::Z).normalize_or_zero();
        let expected_up = (view * Vec3::Y).normalize_or_zero();
        let forward = (root.rotation * Vec3::Z).normalize_or_zero();
        let up = (root.rotation * Vec3::Y).normalize_or_zero();
        assert!(forward.dot(expected_forward) > 0.999);
        assert!(up.dot(expected_up) > 0.999);
    }

    #[test]
    fn authored_weapon_ads_aligns_sight_axis_to_camera() {
        let p = fixture();
        let camera = Vec3::new(0.0, 1.7, 0.0);
        let view = Quat::IDENTITY;
        let root = weapon_root_from_first_person_view(&p, camera, view, 1.0).expect("ADS");
        let handle = weapon_handle_position(&p, root);
        let rear = handle + root.rotation * v3(p.ads_rear_sight_from_handle);
        let front = handle + root.rotation * v3(p.ads_front_sight_from_handle);
        let sight = (front - rear).normalize_or_zero();
        assert!(sight.dot(-Vec3::Z) > 0.999);
    }
}
