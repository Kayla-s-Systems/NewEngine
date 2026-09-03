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

#[allow(clippy::too_many_arguments)]
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
#[allow(clippy::too_many_arguments)]
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
