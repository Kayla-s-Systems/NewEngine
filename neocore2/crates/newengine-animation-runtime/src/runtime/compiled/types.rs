/// Prevalidated, topology-sorted skeleton state for animation hot paths.
///
/// Compilation resolves hierarchy order, tag addressing, inverse bind matrices and
/// source/model-space transforms once. Frame evaluation then performs only pose
/// interpolation and a single forward hierarchy evaluation.
#[derive(Clone, Debug)]
pub struct AnimationSkeletonRuntime {
    source_to_model: Mat4,
    model_to_source: Mat4,
    parent_indices: Vec<Option<usize>>,
    evaluation_order: Vec<usize>,
    /// Parent-before-child joint lists for incremental FK refreshes. Each entry contains the
    /// root joint itself followed by every descendant in canonical evaluation order.
    subtree_evaluation_order: Vec<Vec<usize>>,
    joint_tags: Vec<u32>,
    tag_to_joint: HashMap<u32, usize>,
    ambiguous_tags: HashSet<u32>,
    bind_locals: Vec<JointLocalPose>,
    bind_global_inverse: Vec<Mat4>,
    bind_joint_frames: Vec<Mat4>,
    compatibility_key: u64,
}

/// Immutable clip-to-skeleton addressing resolved before entering the frame loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationClipBinding {
    clip_joint_to_skeleton: Vec<usize>,
    /// O(1) inverse addressing for root motion, masks and ownership queries. `None` means that the
    /// clip does not author the corresponding skeleton joint.
    skeleton_joint_to_clip: Vec<Option<usize>>,
    skeleton_joint_count: usize,
    clip_joint_count: usize,
    full_pose: bool,
}

impl AnimationClipBinding {
    /// Skeleton joints authored by this clip, in clip channel order. Presentation systems use
    /// this immutable compiled addressing to layer partial/full-body clips without assuming that
    /// authored joint tags are dense skeleton indices.
    #[inline]
    pub fn skeleton_joint_indices(&self) -> &[usize] {
        &self.clip_joint_to_skeleton
    }

    #[inline]
    pub fn owns_skeleton_joint(&self, joint_index: usize) -> bool {
        self.skeleton_joint_to_clip
            .get(joint_index)
            .is_some_and(Option::is_some)
    }

    #[inline]
    pub fn clip_joint_for_skeleton(&self, joint_index: usize) -> Option<usize> {
        self.skeleton_joint_to_clip
            .get(joint_index)
            .copied()
            .flatten()
    }
}

#[inline]
fn finite_matrix(matrix: Mat4) -> bool {
    matrix.to_cols_array().iter().all(|value| value.is_finite())
}

#[inline]
fn animation_fingerprint_mix(hash: &mut u64, value: u64) {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn validated_local_matrix(
    pose: JointLocalPose,
    fallback_scale: [f32; 3],
    joint_index: usize,
    require_invertible_scale: bool,
) -> Result<Mat4, String> {
    if pose.translation.iter().any(|value| !value.is_finite()) {
        return Err(format!(
            "animation local translation contains non-finite value joint={joint_index}"
        ));
    }

    let rotation = quat(pose.rotation);
    let rotation_len2 = rotation.length_squared();
    if !rotation_len2.is_finite() || rotation_len2 <= 1.0e-8 {
        return Err(format!(
            "animation local rotation is invalid joint={joint_index} len2={rotation_len2}"
        ));
    }

    let scale = pose.scale.unwrap_or(fallback_scale);
    if scale.iter().any(|value| !value.is_finite()) {
        return Err(format!(
            "animation local scale is non-finite joint={joint_index} scale={scale:?}"
        ));
    }
    // Bind transforms must remain invertible because inverse-bind matrices are compiled once.
    // Animated poses are different: native weapon rigs deliberately author zero scale as
    // visibility state (for example magazine bullets / loader shells). A finite singular
    // animated matrix is valid for FK and skinning and must be preserved instead of replaced
    // with bind scale or rejected.
    if require_invertible_scale && scale.iter().any(|value| value.abs() <= 1.0e-8) {
        return Err(format!(
            "animation local scale is singular joint={joint_index} scale={scale:?}"
        ));
    }

    let matrix = Mat4::from_scale_rotation_translation(
        vec3(scale),
        rotation.normalize(),
        vec3(pose.translation),
    );
    finite_matrix(matrix)
        .then_some(matrix)
        .ok_or_else(|| format!("animation local matrix is non-finite joint={joint_index}"))
}
