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
fn authored_prop_socket_resolves_handle_then_inverse_full_handle_transform_once() {
    let mut p = fixture();
    p.handle_from_root = [0.03, -0.02, 0.11];
    let handle_from_root = Quat::from_euler(newengine_math::EulerRot::YXZ, -0.17, 0.12, -0.03);
    p.handle_rotation_from_root = [
        handle_from_root.x,
        handle_from_root.y,
        handle_from_root.z,
        handle_from_root.w,
    ];
    let basis = Quat::from_euler(newengine_math::EulerRot::YXZ, 0.21, -0.09, 0.04);
    p.authored_socket_to_weapon_handle_basis = [basis.x, basis.y, basis.z, basis.w];
    let socket_rotation = Quat::from_euler(newengine_math::EulerRot::YXZ, -0.37, 0.16, -0.08);
    let socket_position = Vec3::new(-0.24, 1.31, 0.18);
    let socket = Mat4::from_scale_rotation_translation(Vec3::ONE, socket_rotation, socket_position);

    let root = weapon_root_from_authored_prop_frame(&p, socket).expect("authored prop root");
    let expected_handle_rotation = (socket_rotation * basis).normalize_or_identity();
    let expected_root_rotation =
        (expected_handle_rotation * handle_from_root.inverse()).normalize_or_identity();
    assert!(root.rotation.dot(expected_root_rotation).abs() > 0.999_999);
    assert!(weapon_handle_position(&p, root).distance(socket_position) <= 1.0e-6);
    assert!(
        weapon_handle_rotation(&p, root)
            .dot(expected_handle_rotation)
            .abs()
            > 0.999_999
    );

    let support = weapon_ready_left_grip_position(&p, root);
    let expected_support = socket_position + expected_handle_rotation * v3(p.left_grip_from_handle);
    assert!(support.distance(expected_support) <= 1.0e-6);

    let error =
        weapon_handle_frame_error_from_authored_socket(&p, root, socket).expect("frame error");
    assert!(error.position_m <= 1.0e-6);
    assert!(error.angular_degrees <= 0.001);
}

#[test]
fn hand_owned_right_palm_contract_round_trips_native_weapon_basis() {
    let mut p = fixture();
    p.right_palm_to_native_rig = [-0.721_937_2, -0.656_295_36, -0.133_45, 0.173_994_88];
    let basis = Quat::from_euler(newengine_math::EulerRot::YXZ, 0.13, -0.07, 0.04);
    p.native_rig_to_runtime_basis = [basis.x, basis.y, basis.z, basis.w];
    let palm_rotation = Quat::from_euler(newengine_math::EulerRot::YXZ, -0.24, 0.18, 0.11);
    let palm_position = Vec3::new(-0.19, 1.36, -0.08);
    let palm = Mat4::from_scale_rotation_translation(Vec3::ONE, palm_rotation, palm_position);

    let root = weapon_root_from_right_palm(&p, palm).expect("hand-owned weapon root");
    let resolved_rotation = weapon_hand_owned_right_palm_rotation(&p, root);
    let resolved_position = weapon_hand_owned_right_palm_position(&p, root);
    assert!(resolved_rotation.dot(palm_rotation).abs() > 0.999_999);
    assert!(resolved_position.distance(palm_position) <= 1.0e-6);

    let handle = weapon_handle_position(&p, root);
    let reconstructed_handle = resolved_position + resolved_rotation * v3(p.right_palm_to_handle);
    assert!(reconstructed_handle.distance(handle) <= 1.0e-6);
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
    let anchored = weapon_ready_contract_with_contacts(&p, contract, Some(anchor), None, 0.0, 0.0)
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
fn authored_ads_camera_translation_weight_preserves_selected_eye_axes() {
    let mut p = fixture();
    p.ads_camera_translation_weight = [1.0, 0.0, 1.0];
    let root = WeaponRootTransform {
        position: Vec3::new(-0.12, 1.39, -0.06),
        rotation: Quat::from_euler(newengine_math::EulerRot::YXZ, 0.18, -0.09, 0.03),
    };
    let view = Quat::from_euler(newengine_math::EulerRot::YXZ, 0.21, -0.11, 0.0);
    let eye = Vec3::new(0.02, 1.63, -0.01);
    let raw = weapon_ads_camera_position(&p, root, view).expect("raw ADS anchor");
    let resolved =
        weapon_resolved_ads_camera_position(&p, root, view, eye).expect("resolved ADS anchor");
    assert!((resolved.x - raw.x).abs() <= 1.0e-6);
    assert!((resolved.y - eye.y).abs() <= 1.0e-6);
    assert!((resolved.z - raw.z).abs() <= 1.0e-6);
}

#[test]
fn full_ads_camera_weight_can_follow_complete_weapon_anchor_when_authored() {
    let mut p = fixture();
    p.ads_camera_translation_weight = [1.0, 1.0, 1.0];
    let root = WeaponRootTransform {
        position: Vec3::new(-0.12, 1.39, -0.06),
        rotation: Quat::from_rotation_y(0.14),
    };
    let view = Quat::from_rotation_y(0.14);
    let eye = Vec3::new(0.0, 1.63, 0.0);
    let raw = weapon_ads_camera_position(&p, root, view).expect("raw ADS anchor");
    let resolved =
        weapon_resolved_ads_camera_position(&p, root, view, eye).expect("resolved ADS anchor");
    assert!(resolved.distance(raw) <= 1.0e-6);
}

#[test]
fn full_body_fpp_hand_anchored_ads_preserves_grip_and_aligns_real_sights() {
    let p = fixture();
    let palm_rotation = Quat::from_euler(newengine_math::EulerRot::YXZ, -0.24, 0.18, 0.11);
    let palm_position = Vec3::new(-0.19, 1.36, -0.08);
    let palm = Mat4::from_scale_rotation_translation(Vec3::ONE, palm_rotation, palm_position);
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
    let camera = weapon_ads_camera_position(&p, root, view).expect("ADS camera origin");
    let offset = v3(p.ads_camera_to_rear_sight);
    let rear_from_camera = camera + view * offset;
    let actual_rear = weapon_rear_sight_position(&p, root);
    assert!(
        rear_from_camera.distance(actual_rear) <= 1.0e-5,
        "camera eye-relief vector must terminate at the rendered rear sight"
    );
}

#[test]
fn full_body_fpp_hip_keeps_exact_authored_palm_weapon_transform() {
    let p = fixture();
    let palm_rotation = Quat::from_euler(newengine_math::EulerRot::YXZ, -0.24, 0.18, 0.11);
    let palm_position = Vec3::new(-0.19, 1.36, -0.08);
    let palm = Mat4::from_scale_rotation_translation(Vec3::ONE, palm_rotation, palm_position);
    let authored = weapon_root_from_right_palm(&p, palm).expect("authored palm root");
    let resolved =
        weapon_first_person_hand_anchored_root(&p, palm, Quat::from_rotation_y(0.8), 0.0, 0.0, 0.0)
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
    let clear = weapon_ready_contract_with_contacts(&p, raw, Some(handle_anchor), None, 0.0, 0.0)
        .expect("clear contract");
    let blocked = weapon_ready_contract_with_contacts(&p, raw, Some(handle_anchor), None, 0.0, 0.8)
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
