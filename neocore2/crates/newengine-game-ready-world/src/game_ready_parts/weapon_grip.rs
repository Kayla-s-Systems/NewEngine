use newengine_math::{Mat3, Mat4, Quat, Vec3};

/// Canonical runtime rifle visual.
pub(crate) const RIFLE_MODEL_REF: &str = "shared/models/weapon/rifle/rifle.ydd@rifle";

/// Canonical authored rig landmarks. The GLTF/YDD root is near the pistol grip but is not the
/// `handle` joint itself; preserving this ~3.3 cm offset is required for the mesh to sit inside
/// the right palm instead of merely placing the weapon root there.
pub(crate) const RIFLE_HANDLE_FROM_ROOT: Vec3 = Vec3::new(0.0, 0.014_173_783, -0.029_675_78);
/// Physical muzzle socket measured from the canonical `rifle.gltf` front cap. The authored mesh
/// ends at z=0.63375235; this point is pushed ~6 mm beyond the barrel so flash/tracer geometry
/// cannot intersect the muzzle surface. Runtime weapon forward is local +Z.
pub(crate) const RIFLE_MUZZLE_FROM_ROOT: Vec3 = Vec3::new(0.107_590_85, 0.041_388_50, 0.640);
/// Canonical left support grip relative to the authored handle.
#[allow(dead_code)] // Retained as the authored source landmark behind the ReadyHold contact.
pub(crate) const RIFLE_LEFT_GRIP_FROM_HANDLE: Vec3 = Vec3::new(-0.021, 0.043_053_337, 0.305_839_27);
/// ReadyHold uses the original Mini-14 `l_grip` exactly. Earlier runtime code moved this contact
/// 9.5 cm rearward to make procedural IK easier, but the original gun-range Abby clip proves the
/// authored ~36.8 cm inter-palm spacing is correct. Reachability must be solved by the upper body,
/// never by moving a physical weapon socket.
pub(crate) const RIFLE_READY_LEFT_GRIP_OFFSET: Vec3 = Vec3::ZERO;

/// Explicit three-contact weapon contract. The canonical handle is the firing-hand grip origin;
/// the support hand sits farther down the handguard; the stock contact is expressed from handle
/// so gameplay/debug code can reason about all physical contacts in one weapon-local frame.
pub(crate) const RIFLE_RIGHT_HAND_GRIP_FROM_HANDLE: Vec3 = Vec3::ZERO;
pub(crate) const RIFLE_LEFT_HAND_GRIP_FROM_HANDLE: Vec3 = RIFLE_LEFT_GRIP_FROM_HANDLE;
pub(crate) const RIFLE_STOCK_CONTACT_FROM_HANDLE: Vec3 =
    Vec3::new(-0.020_290_91, 0.052_923_85, -0.340_682_30);

/// Rear-most physical buttstock contact, measured from the canonical GLTF geometry rather than
/// guessed from the `stock` helper joint. The rear 5 mm surface centroid is the point that should
/// meet the character shoulder pocket in ReadyHold.
pub(crate) const RIFLE_STOCK_CONTACT_FROM_ROOT: Vec3 =
    Vec3::new(-0.020_290_91, 0.067_097_63, -0.370_358_08);
/// Anatomical shoulder-pocket offset from the animated right shoulder in ReadyHold body space:
/// slightly inward, about 10 cm below the shoulder joint, and 5 cm behind the shoulder line.
pub(crate) const RIFLE_READY_SHOULDER_POCKET_OFFSET: Vec3 =
    Vec3::new(0.047_192, -0.154_854, -0.040_955);
/// ADS tightens the shoulder/cheek weld without changing the physical stock ownership. The
/// standing calibration is constrained by Abby's native arm lengths and the original Mini-14
/// handle/l_grip spacing; ADS only raises the pocket ~2.5 cm and moves it slightly inward.
pub(crate) const RIFLE_ADS_SHOULDER_POCKET_OFFSET: Vec3 =
    Vec3::new(0.040, -0.130, -0.030);
/// Original `assault-fire` is six frames at 30 Hz (~166.7 ms). Presentation recoil follows that
/// cadence but remains shoulder-owned: the stock never detaches from the shoulder pocket.
pub(crate) const RIFLE_FIRE_KICK_DURATION_SECONDS: f32 = 1.0 / 6.0;
pub(crate) const RIFLE_FIRE_KICK_PITCH_RADIANS: f32 = 0.024_434_61; // 1.4 degrees
/// Standing ReadyHold weapon basis in anatomical body space (+X character-left, +Y up, +Z forward).
/// This is a constrained calibration: exact original Mini-14 handle/l_grip geometry is preserved,
/// stock remains shoulder-owned, and the resulting Abby shoulder->palm reaches are ~0.36/~0.44 m.
pub(crate) const RIFLE_READY_BODY_TO_ROOT_ROTATION: Quat =
    Quat::from_xyzw(0.036_246_98, 0.607_723_0, -0.041_297_83, 0.792_245_8);
/// Preferred elbow pole offsets from each shoulder, expressed in anatomical body space. They keep
/// elbows down/out instead of letting an unconstrained CCD chain flip across the rifle axis.
pub(crate) const RIFLE_READY_RIGHT_ELBOW_POLE_OFFSET: Vec3 = Vec3::new(-0.150, -0.140, 0.060);
pub(crate) const RIFLE_READY_LEFT_ELBOW_POLE_OFFSET: Vec3 = Vec3::new(0.150, -0.160, 0.080);

/// Palm-center contact calibration from the same authored ReadyHold source. These offsets keep
/// the hand bones around the physical grip surfaces instead of placing joint origins directly on
/// weapon markers.
pub(crate) const RIFLE_READY_LEFT_PALM_TO_LEFT_GRIP: Vec3 =
    Vec3::new(0.002_934_01, 0.100_549_51, 0.006_111_19);
pub(crate) const RIFLE_READY_RIGHT_PALM_TO_WEAPON: Quat =
    Quat::from_xyzw(-0.656_295_36, 0.721_937_2, 0.173_994_88, 0.133_45);
pub(crate) const RIFLE_READY_LEFT_PALM_TO_WEAPON: Quat =
    Quat::from_xyzw(-0.023_270_27, -0.459_249_1, -0.302_612_16, 0.834_850_1);

/// Native Abby forward-rifle calibration, expressed in animated `r_palm` local space.
///
/// The recovered weapon rig uses its local +Y as visual down while North Star runtime weapon
/// space is +Y up. Forward is already +Z in both spaces. A 180-degree local-Z roll therefore
/// canonicalizes up/down without changing muzzle direction.
///
/// Runtime attachment must never rotate the rifle toward the left hand. The right palm owns the
/// weapon; left-arm IK follows `l_grip` after the same basis conversion.
pub(crate) const RIFLE_RIGHT_PALM_TO_HANDLE: Vec3 =
    Vec3::new(0.018_735_905, 0.033_302_292, -0.083_076_47);
#[allow(dead_code)] // Retained for explicit LowCarry/palm-owned stance.
pub(crate) const RIFLE_RIGHT_PALM_TO_NATIVE_RIG: Quat =
    Quat::from_xyzw(-0.721_937_2, -0.656_295_36, -0.133_450_0, 0.173_994_88);
/// 180 degrees around local +Z: `(x,y,z,w) = (0,0,1,0)`.
#[allow(dead_code)] // Retained for explicit LowCarry/palm-owned stance.
pub(crate) const RIFLE_NATIVE_RIG_TO_RUNTIME_BASIS: Quat = Quat::from_xyzw(0.0, 0.0, 1.0, 0.0);

/// First-person view-model calibration in camera-local space. The rifle's authored muzzle is +Z
/// while North Star camera forward is -Z. A 180-degree local-X turn maps +Z -> -Z and also flips
/// the authored weapon up axis into the correct first-person presentation. The previous local-Y
/// basis preserved forward alignment but left the canonical rifle rolled upside-down.
pub(crate) const RIFLE_FIRST_PERSON_VIEW_BASIS: Quat = Quat::from_xyzw(1.0, 0.0, 0.0, 0.0);
pub(crate) const RIFLE_FIRST_PERSON_HIP_HANDLE_OFFSET: Vec3 = Vec3::new(0.205, -0.205, -0.58);
/// Rear sight/eye reference relative to the canonical rifle handle.
pub(crate) const RIFLE_FP_ADS_REAR_SIGHT_FROM_HANDLE: Vec3 = Vec3::new(0.0, -0.058, 0.235);
/// Front sight reference relative to the canonical rifle handle. The corrected first-person
/// basis maps authored local -Y to camera-up, so the sight landmarks belong on the -Y/top side.
pub(crate) const RIFLE_FP_ADS_FRONT_SIGHT_FROM_HANDLE: Vec3 = Vec3::new(0.0, -0.070, 0.640);
/// Desired rear-sight position in camera-local ADS space: exactly on the eye/crosshair axis and
/// far enough beyond the near plane to expose the sight instead of the stock/receiver.
pub(crate) const RIFLE_FP_ADS_CAMERA_TO_REAR_SIGHT: Vec3 = Vec3::new(0.0, 0.0, -0.075);
const RIFLE_FIRST_PERSON_HIP_CONVERGENCE_M: f32 = 12.0;

#[allow(dead_code)] // Retained for explicit LowCarry/palm-owned stance.
#[inline]
fn rifle_rotation_from_palm(palm_rotation: Quat) -> Quat {
    (palm_rotation * RIFLE_RIGHT_PALM_TO_NATIVE_RIG * RIFLE_NATIVE_RIG_TO_RUNTIME_BASIS)
        .normalize_or_identity()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RifleRootTransform {
    pub position: Vec3,
    pub rotation: Quat,
}

/// Resolves the canonical rifle root/handle from the physical right-palm frame.
/// The returned root is exactly the weapon `handle`. Canonical rifle POSITION data is already
/// handle-centered by the offline GLTF exporter, so the visual mesh needs no second pivot offset.
#[allow(dead_code)] // Retained for explicit LowCarry/palm-owned stance.
pub(crate) fn rifle_root_from_right_palm(right_palm: Mat4) -> Option<RifleRootTransform> {
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
    let rotation = rifle_rotation_from_palm(palm_rotation);
    let handle_position = palm_position + palm_rotation * RIFLE_RIGHT_PALM_TO_HANDLE;
    Some(RifleRootTransform {
        position: handle_position - rotation * RIFLE_HANDLE_FROM_ROOT,
        rotation,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RifleReadySolveContract {
    pub root: RifleRootTransform,
    pub shoulder_pocket: Vec3,
    pub stock_contact: Vec3,
    pub right_elbow_pole: Vec3,
    pub left_elbow_pole: Vec3,
}

/// Builds an anatomical ReadyHold frame from the shoulder line instead of trusting `spined` local
/// axes (whose Naughty Dog basis is intentionally not body-forward/body-up). The shoulder line
/// defines character-left; chest->shoulder-mid defines up; their cross product defines forward.
pub(crate) fn rifle_ready_solve_contract(
    chest: Mat4,
    right_shoulder: Mat4,
    left_shoulder: Mat4,
) -> Option<RifleReadySolveContract> {
    rifle_ready_solve_contract_aimed(chest, right_shoulder, left_shoulder, None, 0.0)
}

/// Shoulder-owned ReadyHold with optional view alignment for full-body first person.
///
/// Weapon translation is always derived from stock -> right shoulder pocket. When a view
/// direction is supplied, only the weapon orientation is swung so its sight axis follows that
/// direction; both hand IK targets are then derived from this exact same root. This eliminates
/// the old split-brain path where the rendered first-person rifle followed the camera while the
/// visible body solved its arms against a different third-person rifle transform.
pub(crate) fn rifle_ready_solve_contract_aimed(
    chest: Mat4,
    right_shoulder: Mat4,
    left_shoulder: Mat4,
    view_forward_model: Option<Vec3>,
    aim_alpha: f32,
) -> Option<RifleReadySolveContract> {
    rifle_ready_solve_contract_presented(
        chest,
        right_shoulder,
        left_shoulder,
        view_forward_model,
        aim_alpha,
        0.0,
    )
}

/// Unified shouldered presentation solve used by both the rendered rifle and bilateral arm IK.
/// `fire_recoil_alpha` adds the short original-style muzzle rise around the planted stock contact;
/// it may change orientation, but stock -> shoulder remains an invariant.
pub(crate) fn rifle_ready_solve_contract_presented(
    chest: Mat4,
    right_shoulder: Mat4,
    left_shoulder: Mat4,
    view_forward_model: Option<Vec3>,
    aim_alpha: f32,
    fire_recoil_alpha: f32,
) -> Option<RifleReadySolveContract> {
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
    // Shoulder/chest translation captures torso lean, but following it 1:1 made the weapon pitch
    // down with Abby's internal spine geometry. Keep model up authoritative and admit only a
    // small amount of torso lean so ReadyHold remains stable under locomotion.
    let up_hint = (Vec3::Y * 0.85 + torso_up * 0.15).normalize_or_zero();
    let forward_axis = left_axis.cross(up_hint).normalize_or_zero();
    if forward_axis.length_squared() < 1.0e-8 {
        return None;
    }
    let up_axis = forward_axis.cross(left_axis).normalize_or_zero();
    let body_rotation =
        Quat::from_mat3(&Mat3::from_cols(left_axis, up_axis, forward_axis)).normalize_or_identity();

    let base_rotation = (body_rotation * RIFLE_READY_BODY_TO_ROOT_ROTATION).normalize_or_identity();
    let rotation = view_forward_model
        .filter(|forward| forward.is_finite() && forward.length_squared() > 1.0e-8)
        .map(|forward| {
            let desired_forward = forward.normalize();
            // The authored sight rail is the aiming authority. Align it rather than blindly using
            // weapon +Z so rear/front sight calibration and muzzle presentation stay coherent.
            let local_sight_axis = (RIFLE_FP_ADS_FRONT_SIGHT_FROM_HANDLE
                - RIFLE_FP_ADS_REAR_SIGHT_FROM_HANDLE)
                .normalize_or_zero();
            let current_sight_axis = (base_rotation * local_sight_axis).normalize_or_zero();
            if current_sight_axis.length_squared() <= 1.0e-8 {
                base_rotation
            } else {
                let swing = Quat::from_rotation_arc(current_sight_axis, desired_forward);
                (swing * base_rotation).normalize_or_identity()
            }
        })
        .unwrap_or(base_rotation);
    // Fire kick rotates around the anatomical left/right shoulder axis. Recomputing root
    // translation from the stock contact below keeps the butt planted while the muzzle rises.
    let recoil_rotation = if fire_recoil_alpha > 0.0 {
        let pitch_axis = (body_rotation * Vec3::X).normalize_or_zero();
        if pitch_axis.length_squared() > 1.0e-8 {
            let kick = Quat::from_axis_angle(
                pitch_axis,
                -RIFLE_FIRE_KICK_PITCH_RADIANS * fire_recoil_alpha,
            );
            (kick * rotation).normalize_or_identity()
        } else {
            rotation
        }
    } else {
        rotation
    };
    let rotation = recoil_rotation;

    let shoulder_offset =
        RIFLE_READY_SHOULDER_POCKET_OFFSET.lerp(RIFLE_ADS_SHOULDER_POCKET_OFFSET, aim_alpha);
    let shoulder_pocket = right_position + body_rotation * shoulder_offset;
    let position = shoulder_pocket - rotation * RIFLE_STOCK_CONTACT_FROM_ROOT;
    let root = RifleRootTransform { position, rotation };
    let stock_contact = rifle_handle_position(root) + rotation * RIFLE_STOCK_CONTACT_FROM_HANDLE;
    let right_elbow_pole = right_position + body_rotation * RIFLE_READY_RIGHT_ELBOW_POLE_OFFSET;
    let left_elbow_pole = left_position + body_rotation * RIFLE_READY_LEFT_ELBOW_POLE_OFFSET;

    (position.is_finite()
        && rotation.is_finite()
        && shoulder_pocket.is_finite()
        && stock_contact.is_finite()
        && right_elbow_pole.is_finite()
        && left_elbow_pole.is_finite())
    .then_some(RifleReadySolveContract {
        root,
        shoulder_pocket,
        stock_contact,
        right_elbow_pole,
        left_elbow_pole,
    })
}

/// Resolves the first-person rifle directly from the current camera pose.
///
/// Hip-fire keeps the weapon lower-right, ADS moves it onto the optical axis, and in both states
/// the authored +Z muzzle axis converges on the center camera ray. First-person yaw/pitch is thus
/// camera-authoritative instead of inheriting a hand/chest attachment.
pub(crate) fn rifle_root_from_first_person_view(
    camera_position: Vec3,
    view_rotation: Quat,
    aim_alpha: f32,
) -> Option<RifleRootTransform> {
    if !camera_position.is_finite() || !view_rotation.is_finite() {
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

    // Hip-fire remains the existing camera-relative convergence solution. ADS must not be built
    // from a guessed handle offset: it has its own optical contract based on rear/front sights.
    let base_rotation = (view_rotation * RIFLE_FIRST_PERSON_VIEW_BASIS).normalize_or_identity();
    let hip_handle_position =
        camera_position + view_rotation * RIFLE_FIRST_PERSON_HIP_HANDLE_OFFSET;
    let hip_target = camera_position + camera_forward * RIFLE_FIRST_PERSON_HIP_CONVERGENCE_M;
    let hip_forward = (hip_target - hip_handle_position).normalize_or_zero();
    if hip_forward.length_squared() <= 1.0e-8 {
        return None;
    }
    let hip_base_forward = (base_rotation * Vec3::Z).normalize_or_zero();
    let hip_swing = Quat::from_rotation_arc(hip_base_forward, hip_forward);
    let hip_rotation = (hip_swing * base_rotation).normalize_or_identity();

    // ADS: align the authored rear->front sight line exactly with the camera center ray.
    let local_ads_axis = (RIFLE_FP_ADS_FRONT_SIGHT_FROM_HANDLE
        - RIFLE_FP_ADS_REAR_SIGHT_FROM_HANDLE)
        .normalize_or_zero();
    if local_ads_axis.length_squared() <= 1.0e-8 {
        return None;
    }
    let world_ads_axis = (base_rotation * local_ads_axis).normalize_or_zero();
    if world_ads_axis.length_squared() <= 1.0e-8 {
        return None;
    }
    let ads_axis_correction = Quat::from_rotation_arc(world_ads_axis, camera_forward);
    let ads_rotation = (ads_axis_correction * base_rotation).normalize_or_identity();

    // Place the rear sight at the eye-side camera anchor. This naturally pushes the handle/stock
    // behind the camera while leaving the sight and front post in front, producing a real sight
    // picture instead of an enlarged receiver/stock.
    let ads_rear_sight_world = camera_position + view_rotation * RIFLE_FP_ADS_CAMERA_TO_REAR_SIGHT;
    let ads_handle_position =
        ads_rear_sight_world - ads_rotation * RIFLE_FP_ADS_REAR_SIGHT_FROM_HANDLE;

    let rotation = hip_rotation
        .slerp(ads_rotation, aim_alpha)
        .normalize_or_identity();
    let handle_position = hip_handle_position.lerp(ads_handle_position, aim_alpha);

    Some(RifleRootTransform {
        position: handle_position - rotation * RIFLE_HANDLE_FROM_ROOT,
        rotation,
    })
}

#[inline]
pub(crate) fn rifle_handle_position(root: RifleRootTransform) -> Vec3 {
    root.position + root.rotation * RIFLE_HANDLE_FROM_ROOT
}

#[inline]
pub(crate) fn rifle_muzzle_position(root: RifleRootTransform) -> Vec3 {
    root.position + root.rotation * RIFLE_MUZZLE_FROM_ROOT
}

#[inline]
pub(crate) fn rifle_muzzle_forward(root: RifleRootTransform) -> Vec3 {
    (root.rotation * Vec3::Z).normalize_or_zero()
}

#[allow(dead_code)] // Raw authored l_grip; production ReadyHold uses the corrected explicit contact.
#[inline]
pub(crate) fn rifle_left_grip_position(root: RifleRootTransform) -> Vec3 {
    rifle_handle_position(root) + root.rotation * RIFLE_LEFT_GRIP_FROM_HANDLE
}

#[inline]
pub(crate) fn rifle_ready_left_grip_position(root: RifleRootTransform) -> Vec3 {
    rifle_handle_position(root) + root.rotation * RIFLE_LEFT_HAND_GRIP_FROM_HANDLE
}

#[inline]
pub(crate) fn rifle_ready_right_palm_rotation(root: RifleRootTransform) -> Quat {
    (root.rotation * RIFLE_READY_RIGHT_PALM_TO_WEAPON.inverse()).normalize_or_identity()
}

#[inline]
pub(crate) fn rifle_ready_left_palm_rotation(root: RifleRootTransform) -> Quat {
    (root.rotation * RIFLE_READY_LEFT_PALM_TO_WEAPON.inverse()).normalize_or_identity()
}

#[inline]
pub(crate) fn rifle_ready_right_palm_position(root: RifleRootTransform) -> Vec3 {
    let rotation = rifle_ready_right_palm_rotation(root);
    let firing_grip =
        rifle_handle_position(root) + root.rotation * RIFLE_RIGHT_HAND_GRIP_FROM_HANDLE;
    firing_grip - rotation * RIFLE_RIGHT_PALM_TO_HANDLE
}

#[inline]
pub(crate) fn rifle_ready_left_palm_position(root: RifleRootTransform) -> Vec3 {
    let rotation = rifle_ready_left_palm_rotation(root);
    rifle_ready_left_grip_position(root) - rotation * RIFLE_READY_LEFT_PALM_TO_LEFT_GRIP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rifle_root_is_owned_only_by_right_palm_transform() {
        let palm = Mat4::from_scale_rotation_translation(
            Vec3::ONE,
            Quat::from_rotation_y(0.4),
            Vec3::new(1.0, 2.0, 3.0),
        );
        let root = rifle_root_from_right_palm(palm).expect("rifle root");
        let expected_handle =
            Vec3::new(1.0, 2.0, 3.0) + Quat::from_rotation_y(0.4) * RIFLE_RIGHT_PALM_TO_HANDLE;
        let expected_rotation = rifle_rotation_from_palm(Quat::from_rotation_y(0.4));
        let expected_root = expected_handle - expected_rotation * RIFLE_HANDLE_FROM_ROOT;
        assert!((root.position - expected_root).length() < 1.0e-6);
        assert!((rifle_handle_position(root) - expected_handle).length() < 1.0e-6);
        assert!(root.rotation.dot(expected_rotation).abs() > 0.999_999);
    }

    #[test]
    fn rifle_runtime_basis_flips_native_up_without_reversing_muzzle() {
        let native = RIFLE_RIGHT_PALM_TO_NATIVE_RIG.normalize_or_identity();
        let runtime = (RIFLE_RIGHT_PALM_TO_NATIVE_RIG * RIFLE_NATIVE_RIG_TO_RUNTIME_BASIS)
            .normalize_or_identity();
        let native_forward = native * Vec3::Z;
        let runtime_forward = runtime * Vec3::Z;
        let native_up = native * Vec3::Y;
        let runtime_up = runtime * Vec3::Y;
        assert!(native_forward.dot(runtime_forward) > 0.999_999);
        assert!(native_up.dot(runtime_up) < -0.999_999);
    }

    #[test]
    fn canonical_handle_preserves_authored_root_offset() {
        assert!((RIFLE_HANDLE_FROM_ROOT.length() - 0.032_889).abs() < 1.0e-4);
    }

    #[test]
    fn canonical_muzzle_is_outside_front_cap_and_points_weapon_forward() {
        assert!(RIFLE_MUZZLE_FROM_ROOT.z > 0.633_752_35);
        let root = RifleRootTransform {
            position: Vec3::new(2.0, 3.0, 4.0),
            rotation: Quat::from_euler(newengine_math::EulerRot::YXZ, 0.6, -0.25, 0.0),
        };
        let muzzle = rifle_muzzle_position(root);
        let forward = rifle_muzzle_forward(root);
        assert!(muzzle.is_finite());
        assert!(forward.dot(root.rotation * Vec3::Z) > 0.999_999);
    }

    #[test]
    fn canonical_left_grip_has_forward_reach_from_handle() {
        assert!(RIFLE_LEFT_GRIP_FROM_HANDLE.z > 0.30);
        assert!((RIFLE_LEFT_GRIP_FROM_HANDLE.length() - 0.309_567_84).abs() < 1.0e-5);
    }

    #[test]
    fn first_person_rifle_basis_preserves_forward_and_corrects_upside_down_roll() {
        let view = Quat::IDENTITY;
        let root = rifle_root_from_first_person_view(Vec3::ZERO, view, 1.0)
            .expect("first-person rifle root");
        let forward = (root.rotation * Vec3::Z).normalize_or_zero();
        let weapon_up = (root.rotation * Vec3::Y).normalize_or_zero();
        assert!(forward.dot(-Vec3::Z) > 0.999);
        // Canonical rifle geometry arrives with the visible top opposite its raw local +Y in the
        // first-person presentation, so the corrected basis must keep that top camera-up.
        assert!(weapon_up.dot(-Vec3::Y) > 0.999);
    }

    #[test]
    fn first_person_rifle_muzzle_converges_on_camera_center_ray() {
        let camera_position = Vec3::new(2.0, 1.6, -3.0);
        let view = Quat::from_euler(newengine_math::EulerRot::YXZ, 0.73, -0.41, 0.0);
        let root = rifle_root_from_first_person_view(camera_position, view, 0.0)
            .expect("first-person rifle root");
        let handle = rifle_handle_position(root);
        let muzzle_axis = (root.rotation * Vec3::Z).normalize_or_zero();
        let camera_forward = (view * -Vec3::Z).normalize_or_zero();
        let target = camera_position + camera_forward * RIFLE_FIRST_PERSON_HIP_CONVERGENCE_M;
        let expected = (target - handle).normalize_or_zero();
        assert!(muzzle_axis.dot(expected) > 0.999_999);
    }

    #[test]
    fn first_person_ads_aligns_sight_line_to_camera_center_ray() {
        let camera = Vec3::new(1.0, 1.7, -2.0);
        let view = Quat::from_euler(newengine_math::EulerRot::YXZ, 0.37, -0.19, 0.0);
        let ads = rifle_root_from_first_person_view(camera, view, 1.0).expect("ads");
        let handle = rifle_handle_position(ads);
        let rear = handle + ads.rotation * RIFLE_FP_ADS_REAR_SIGHT_FROM_HANDLE;
        let front = handle + ads.rotation * RIFLE_FP_ADS_FRONT_SIGHT_FROM_HANDLE;
        let sight_axis = (front - rear).normalize_or_zero();
        let camera_forward = (view * -Vec3::Z).normalize_or_zero();
        let expected_rear = camera + view * RIFLE_FP_ADS_CAMERA_TO_REAR_SIGHT;

        assert!(sight_axis.dot(camera_forward) > 0.999_999);
        assert!((rear - expected_rear).length() < 1.0e-5);
        let handle_camera_ls = view.inverse() * (handle - camera);
        assert!(
            handle_camera_ls.y < 0.0,
            "ADS handle must stay below the eye instead of lifting the rifle above it: {handle_camera_ls:?}"
        );
    }

    #[test]
    fn first_person_ads_places_stock_behind_eye_instead_of_filling_view() {
        let ads = rifle_root_from_first_person_view(Vec3::ZERO, Quat::IDENTITY, 1.0).expect("ads");
        let handle = rifle_handle_position(ads);
        let stock = ads.position + ads.rotation * RIFLE_STOCK_CONTACT_FROM_ROOT;
        assert!(
            handle.z > 0.10,
            "handle should move behind camera in ADS: {handle:?}"
        );
        assert!(
            stock.z > handle.z,
            "stock should be farther behind eye: stock={stock:?} handle={handle:?}"
        );
    }

    #[test]
    fn explicit_ready_contacts_share_one_handle_space() {
        let derived_stock = RIFLE_STOCK_CONTACT_FROM_ROOT - RIFLE_HANDLE_FROM_ROOT;
        assert!((derived_stock - RIFLE_STOCK_CONTACT_FROM_HANDLE).length() < 1.0e-6);
        let derived_left = RIFLE_LEFT_GRIP_FROM_HANDLE + RIFLE_READY_LEFT_GRIP_OFFSET;
        assert!((derived_left - RIFLE_LEFT_HAND_GRIP_FROM_HANDLE).length() < 1.0e-6);
        assert_eq!(RIFLE_RIGHT_HAND_GRIP_FROM_HANDLE, Vec3::ZERO);
    }

    #[test]
    fn aimed_ready_hold_keeps_stock_planted_and_aligns_sight_axis() {
        let chest = Mat4::from_translation(Vec3::new(0.0, 1.285_745, 0.0));
        let right_shoulder = Mat4::from_translation(Vec3::new(-0.17, 1.345_745, 0.0));
        let left_shoulder = Mat4::from_translation(Vec3::new(0.17, 1.345_745, 0.0));
        let desired = Vec3::new(0.0, 0.35, 1.0).normalize();
        let contract = rifle_ready_solve_contract_aimed(
            chest,
            right_shoulder,
            left_shoulder,
            Some(desired),
            1.0,
        )
        .expect("aimed ready solve");
        let handle = rifle_handle_position(contract.root);
        let rear = handle + contract.root.rotation * RIFLE_FP_ADS_REAR_SIGHT_FROM_HANDLE;
        let front = handle + contract.root.rotation * RIFLE_FP_ADS_FRONT_SIGHT_FROM_HANDLE;
        let sight = (front - rear).normalize_or_zero();
        assert!(sight.dot(desired) > 0.999_999);
        assert!((contract.stock_contact - contract.shoulder_pocket).length() < 1.0e-6);
    }

    #[test]
    fn ready_support_target_preserves_original_mini14_l_grip() {
        assert_eq!(RIFLE_READY_LEFT_GRIP_OFFSET, Vec3::ZERO);
        assert_eq!(RIFLE_LEFT_HAND_GRIP_FROM_HANDLE, RIFLE_LEFT_GRIP_FROM_HANDLE);
        assert!(RIFLE_LEFT_HAND_GRIP_FROM_HANDLE.z > 0.30);
    }

    #[test]
    fn fire_kick_raises_muzzle_without_unplanting_stock() {
        let chest = Mat4::from_translation(Vec3::new(0.0, 1.28, 0.0));
        let right_shoulder = Mat4::from_translation(Vec3::new(-0.17, 1.35, 0.0));
        let left_shoulder = Mat4::from_translation(Vec3::new(0.17, 1.35, 0.0));
        let rest = rifle_ready_solve_contract_presented(
            chest, right_shoulder, left_shoulder, None, 0.0, 0.0,
        )
        .expect("rest contract");
        let kicked = rifle_ready_solve_contract_presented(
            chest, right_shoulder, left_shoulder, None, 0.0, 1.0,
        )
        .expect("kicked contract");
        assert!((kicked.stock_contact - kicked.shoulder_pocket).length() < 1.0e-6);
        assert!((rest.stock_contact - rest.shoulder_pocket).length() < 1.0e-6);
        let rest_forward = rest.root.rotation * Vec3::Z;
        let kicked_forward = kicked.root.rotation * Vec3::Z;
        assert!(kicked_forward.y > rest_forward.y, "fire kick must raise muzzle");
    }

    #[test]
    fn ready_hold_stock_is_anchored_and_contact_targets_are_reachable() {
        let chest = Mat4::from_scale_rotation_translation(
            Vec3::ONE,
            Quat::IDENTITY,
            Vec3::new(0.0, 1.285_745, 0.0),
        );
        let right_shoulder = Mat4::from_translation(Vec3::new(-0.17, 1.345_745, 0.0));
        let left_shoulder = Mat4::from_translation(Vec3::new(0.17, 1.345_745, 0.0));
        let contract = rifle_ready_solve_contract(chest, right_shoulder, left_shoulder)
            .expect("ready solve contract");
        let right = rifle_ready_right_palm_position(contract.root);
        let left = rifle_ready_left_palm_position(contract.root);
        let right_shoulder_position = right_shoulder.transform_point3(Vec3::ZERO);
        let left_shoulder_position = left_shoulder.transform_point3(Vec3::ZERO);
        assert!(right.is_finite() && left.is_finite());
        assert!((contract.stock_contact - contract.shoulder_pocket).length() < 1.0e-6);
        let right_reach = (right - right_shoulder_position).length();
        let left_reach = (left - left_shoulder_position).length();
        assert!(
            (0.35..=0.37).contains(&right_reach),
            "ReadyHold right-arm reach drifted outside Abby standing calibration: {right_reach:.6} m"
        );
        assert!(
            (0.43..=0.45).contains(&left_reach),
            "ReadyHold left-arm reach drifted outside Abby standing calibration: {left_reach:.6} m"
        );
        // Character-left is +X in this canonical test basis: poles must be outward and below.
        assert!(contract.right_elbow_pole.x < right_shoulder_position.x);
        assert!(contract.left_elbow_pole.x > left_shoulder_position.x);
        assert!(contract.right_elbow_pole.y < right_shoulder_position.y);
        assert!(contract.left_elbow_pole.y < left_shoulder_position.y);
    }
}
