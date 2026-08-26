#[derive(Clone, Copy, Debug)]
struct EyeRuntimeContract {
    left: usize,
    right: usize,
    parent: usize,
}

fn build_eye_runtime_contract(skeleton: &ModelSkeletonMetadata) -> Option<EyeRuntimeContract> {
    let left = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == "l_eyeball")?;
    let right = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == "r_eyeball")?;
    let parent = skeleton.joints.get(left)?.parent_index? as usize;
    if skeleton
        .joints
        .get(right)?
        .parent_index
        .map(|value| value as usize)
        != Some(parent)
        || skeleton.joints.get(parent)?.name != "headb"
    {
        return None;
    }
    Some(EyeRuntimeContract {
        left,
        right,
        parent,
    })
}

fn stabilize_eye_locals(
    contract: Option<&EyeRuntimeContract>,
    skeleton: &ModelSkeletonMetadata,
    pose: &mut [JointLocalPose],
) -> Result<(), String> {
    let Some(contract) = contract else {
        return Ok(());
    };
    for index in [contract.left, contract.right] {
        let joint = skeleton
            .joints
            .get(index)
            .ok_or_else(|| format!("eye joint outside skeleton index={index}"))?;
        let dst = pose
            .get_mut(index)
            .ok_or_else(|| format!("eye joint outside sampled pose index={index}"))?;
        *dst = JointLocalPose {
            translation: joint.position_ls,
            rotation: joint.rotation_ls,
            scale: Some(joint.scale_ls),
        };
    }
    Ok(())
}

#[inline]
fn matrix_max_abs_delta(a: Mat4, b: Mat4) -> f32 {
    a.to_cols_array()
        .into_iter()
        .zip(b.to_cols_array())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max)
}

fn validate_eye_palette(
    contract: Option<&EyeRuntimeContract>,
    palette: &[Mat4],
) -> Result<(), String> {
    let Some(contract) = contract else {
        return Ok(());
    };
    let parent = *palette
        .get(contract.parent)
        .ok_or_else(|| "eye parent outside skin palette".to_owned())?;
    for (side, index) in [("left", contract.left), ("right", contract.right)] {
        let eye = *palette
            .get(index)
            .ok_or_else(|| format!("{side} eye outside skin palette index={index}"))?;
        let drift = matrix_max_abs_delta(eye, parent);
        // With authored bind-local eyes, A_eye=A_parent*Lbind and B_eye=B_parent*Lbind,
        // therefore A_eye*inverse(B_eye) must reduce to the exact parent deformation.
        if !drift.is_finite() || drift > 5.0e-4 {
            return Err(format!(
                "{side} eye palette drift violates animated_global*inverse_bind contract index={index} parent={} max_abs_delta={drift:.8}",
                contract.parent
            ));
        }
    }
    Ok(())
}

fn debug_dump_eye_matrices(
    contract: Option<&EyeRuntimeContract>,
    bind_joint_frames: &[Mat4],
    current_locals: &[JointLocalPose],
    palette: &[Mat4],
    context: &str,
) {
    let Some(contract) = contract else {
        return;
    };
    if crate::env_config::var_os("NORTHSTAR_DEBUG_ABBY_EYES").is_none() {
        return;
    }
    let Some(parent_bind_global) = bind_joint_frames.get(contract.parent).copied() else {
        return;
    };
    let Some(parent_palette) = palette.get(contract.parent).copied() else {
        return;
    };
    let parent_global = parent_palette * parent_bind_global;
    for (side, index) in [("left", contract.left), ("right", contract.right)] {
        let (Some(bind_global), Some(local), Some(palette_matrix)) = (
            bind_joint_frames.get(index).copied(),
            current_locals.get(index),
            palette.get(index).copied(),
        ) else {
            continue;
        };
        let scale = local.scale.unwrap_or([1.0, 1.0, 1.0]);
        let animated_local = Mat4::from_scale_rotation_translation(
            Vec3::new(scale[0], scale[1], scale[2]),
            Quat::from_xyzw(
                local.rotation[0],
                local.rotation[1],
                local.rotation[2],
                local.rotation[3],
            )
            .normalize_or_identity(),
            Vec3::new(
                local.translation[0],
                local.translation[1],
                local.translation[2],
            ),
        );
        let animated_global = palette_matrix * bind_global;
        newengine_ulog_api::ulog::info!(
            "ABBY_EYE_MATRIX context='{}' side={} joint={} parent={} bind_global={:?} parent_global={:?} animated_local={:?} animated_global={:?} palette_matrix={:?} parent_palette={:?} palette_parent_drift={:.8}",
            context,
            side,
            index,
            contract.parent,
            bind_global,
            parent_global,
            animated_local,
            animated_global,
            palette_matrix,
            parent_palette,
            matrix_max_abs_delta(palette_matrix, parent_palette),
        );
    }
}
