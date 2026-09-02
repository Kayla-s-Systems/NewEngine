#[derive(Clone, Debug)]
struct DetachedHeadFollowRig {
    driver_joint: usize,
    followers: Vec<usize>,
}

fn collect_joint_descendants(skeleton: &ModelSkeletonMetadata, roots: &[usize]) -> Vec<usize> {
    let mut followers = Vec::new();
    for index in 0..skeleton.joints.len() {
        let mut cursor = Some(index);
        let mut remaining = skeleton.joints.len();
        while let Some(current) = cursor {
            if roots.contains(&current) {
                followers.push(index);
                break;
            }
            if current >= skeleton.joints.len() || remaining == 0 {
                break;
            }
            remaining -= 1;
            cursor = skeleton.joints[current]
                .parent_index
                .map(|value| value as usize);
        }
    }
    followers.sort_unstable();
    followers.dedup();
    followers
}

fn build_detached_head_follow(
    skeleton: &ModelSkeletonMetadata,
    authored: &newengine_engine_runtime::gameplay::PlayerPaletteFollowRule,
) -> Result<DetachedHeadFollowRig, String> {
    let driver_name = authored.driver_joint.trim();
    let driver_joint = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == driver_name)
        .ok_or_else(|| format!("authored palette-follow driver is absent joint='{driver_name}'"))?;

    let mut roots = Vec::with_capacity(authored.follower_roots.len());
    for name in &authored.follower_roots {
        let name = name.trim();
        let index = skeleton
            .joints
            .iter()
            .position(|joint| joint.name == name)
            .ok_or_else(|| format!("authored palette-follow root is absent joint='{name}'"))?;
        roots.push(index);
    }
    roots.sort_unstable();
    roots.dedup();
    if roots.is_empty() {
        return Err("authored palette-follow rule requires at least one follower root".to_owned());
    }

    let mut followers = if authored.include_descendants {
        collect_joint_descendants(skeleton, &roots)
    } else {
        roots
    };
    followers.retain(|joint| *joint != driver_joint);
    if followers.is_empty() {
        return Err("authored palette-follow rule resolved no followers".to_owned());
    }

    Ok(DetachedHeadFollowRig {
        driver_joint,
        followers,
    })
}

fn apply_detached_head_follow_palette(
    rig: Option<&DetachedHeadFollowRig>,
    palette: &mut [Mat4],
) -> Result<(), String> {
    let Some(rig) = rig else {
        return Ok(());
    };
    let driver_deformation = *palette.get(rig.driver_joint).ok_or_else(|| {
        format!(
            "palette-follow driver outside palette joint={}",
            rig.driver_joint
        )
    })?;

    // Skin-palette entries are model-space deformation transforms. The authored rule explicitly
    // selects which detached branches consume the driver's deformation; runtime does not infer
    // rig names, character families, or follower semantics.
    for &joint in &rig.followers {
        let follower_deformation = *palette
            .get(joint)
            .ok_or_else(|| format!("palette-follow follower outside palette joint={joint}"))?;
        palette[joint] = driver_deformation * follower_deformation;
    }
    Ok(())
}
