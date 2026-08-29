#[derive(Clone, Debug)]
struct DetachedHeadFollowRig {
    /// Canonical imported equivalent of North Star `headb`.
    ///
    /// Abby's scalp/hair skin is predominantly weighted to `DEF-spine.006`, and
    /// the original `abby-skel` parents `braid_offset` directly to `headb`.
    /// Detached Blender control/face branches must therefore inherit this same
    /// deformation delta instead of becoming a second animated head space.
    headb_driver: usize,
    control_followers: Vec<usize>,
    face_followers: Vec<usize>,
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

fn build_detached_head_follow(skeleton: &ModelSkeletonMetadata) -> Option<DetachedHeadFollowRig> {
    // Binary authority: original `abby-skel.pak` hierarchy is
    // `... -> neck -> heada -> headb -> braid_offset`. Bind-space comparison maps
    // those joints to `DEF-spine.004/.005/.006` in the imported 709-joint rig.
    let headb_driver = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == "DEF-spine.006")?;

    // The Blender control rig is detached from the deform chain. Keep it useful
    // for authored controls, but project the *same* headb rigid deformation onto
    // it. It is never the skinning authority for Abby's head/hair.
    let control_roots = skeleton
        .joints
        .iter()
        .enumerate()
        .filter_map(|(index, joint)| (joint.name == "MCH-ROT-neck").then_some(index))
        .collect::<Vec<_>>();
    let face_roots = skeleton
        .joints
        .iter()
        .enumerate()
        .filter_map(|(index, joint)| {
            matches!(joint.name.as_str(), "ORG-face" | "MCH-eyes_parent").then_some(index)
        })
        .collect::<Vec<_>>();
    if face_roots.is_empty() {
        return None;
    }

    let control_followers = collect_joint_descendants(skeleton, &control_roots);
    let mut face_followers = collect_joint_descendants(skeleton, &face_roots);
    face_followers.retain(|joint| *joint != headb_driver && !control_followers.contains(joint));

    Some(DetachedHeadFollowRig {
        headb_driver,
        control_followers,
        face_followers,
    })
}

fn apply_detached_head_follow_palette(
    rig: Option<&DetachedHeadFollowRig>,
    palette: &mut [Mat4],
) -> Result<(), String> {
    let Some(rig) = rig else {
        return Ok(());
    };
    let headb_deformation = *palette.get(rig.headb_driver).ok_or_else(|| {
        format!(
            "head-follow canonical headb driver outside palette joint={}",
            rig.headb_driver
        )
    })?;

    // Skin-palette entries are model-space deformation transforms, not local
    // joint transforms. Never rebuild a fake MCH hierarchy by multiplying them
    // parent-by-child. Apply one shared rigid headb delta to every detached
    // control/face branch. Scalp, facial skin, eyes and braid then live in the
    // exact same animated head space as `DEF-spine.006`.
    for &joint in rig
        .control_followers
        .iter()
        .chain(rig.face_followers.iter())
    {
        let detached_deformation = *palette
            .get(joint)
            .ok_or_else(|| format!("detached head follower outside palette joint={joint}"))?;
        palette[joint] = headb_deformation * detached_deformation;
    }
    Ok(())
}

