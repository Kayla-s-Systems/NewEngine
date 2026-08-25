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
    let base_rotation =
        (body_rotation * q4(presentation.ready_body_to_root_rotation)).normalize_or_identity();
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
    let rotation = if fire_recoil_alpha > 0.0 {
        let pitch_axis = (body_rotation * Vec3::X).normalize_or_zero();
        if pitch_axis.length_squared() > 1.0e-8 {
            (Quat::from_axis_angle(
                pitch_axis,
                -presentation.fire_kick_pitch_radians * fire_recoil_alpha,
            ) * rotation)
                .normalize_or_identity()
        } else {
            rotation
        }
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
    let base_rotation =
        (view_rotation * q4(presentation.first_person_view_basis)).normalize_or_identity();
    let hip_handle_position =
        camera_position + view_rotation * v3(presentation.first_person_hip_handle_offset);
    let hip_target = camera_position + camera_forward * presentation.first_person_hip_convergence_m;
    let hip_forward = (hip_target - hip_handle_position).normalize_or_zero();
    if hip_forward.length_squared() <= 1.0e-8 {
        return None;
    }
    let hip_base_forward = (base_rotation * Vec3::Z).normalize_or_zero();
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
            first_person_view_basis: [1.0, 0.0, 0.0, 0.0],
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
