use std::collections::{HashMap, HashSet};

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
        self.clip_joint_to_skeleton.contains(&joint_index)
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

impl AnimationSkeletonRuntime {
    /// Compiles authored skeleton metadata into an immutable evaluation plan.
    pub fn compile(
        skeleton: &ModelSkeletonMetadata,
        source_to_model: [f32; 16],
    ) -> Result<Self, String> {
        let joint_count = skeleton.joints.len();
        if joint_count == 0 {
            return Err("animation skeleton runtime requires at least one joint".to_owned());
        }

        for (index, joint) in skeleton.joints.iter().enumerate() {
            if joint.index as usize != index {
                return Err(format!(
                    "skeleton joint indices must be dense index={} authored={}",
                    index, joint.index
                ));
            }
            if joint.position_ls.iter().any(|value| !value.is_finite()) {
                return Err(format!(
                    "skeleton bind translation is non-finite joint={index}"
                ));
            }
            let rotation = quat(joint.rotation_ls);
            let rotation_len2 = rotation.length_squared();
            if !rotation_len2.is_finite() || rotation_len2 <= 1.0e-8 {
                return Err(format!(
                    "skeleton bind rotation is invalid joint={index} len2={rotation_len2}"
                ));
            }
            if joint
                .scale_ls
                .iter()
                .any(|value| !value.is_finite() || value.abs() <= 1.0e-8)
            {
                return Err(format!(
                    "skeleton bind scale is singular/non-finite joint={index} scale={:?}",
                    joint.scale_ls
                ));
            }
        }

        let source_to_model = Mat4::from_cols_array(&source_to_model);
        if !affine_invertible(source_to_model) {
            return Err("animation source-to-model transform is singular/non-finite".to_owned());
        }
        let model_to_source = source_to_model.inverse();

        let parent_indices = skeleton
            .joints
            .iter()
            .enumerate()
            .map(|(index, joint)| {
                let parent = joint.parent_index.map(|value| value as usize);
                if parent.is_some_and(|parent| parent >= joint_count) {
                    return Err(format!(
                        "skeleton parent index outside joint table joint={index} parent={parent:?}"
                    ));
                }
                if parent == Some(index) {
                    return Err(format!("skeleton joint cannot parent itself joint={index}"));
                }
                Ok(parent)
            })
            .collect::<Result<Vec<_>, String>>()?;

        // Source formats may store children before parents. Resolve the order once and
        // keep it for all subsequent frame evaluations.
        let mut evaluation_order = Vec::with_capacity(joint_count);
        let mut resolved = vec![false; joint_count];
        while evaluation_order.len() < joint_count {
            let mut progress = false;
            for index in 0..joint_count {
                if resolved[index] {
                    continue;
                }
                if parent_indices[index].is_some_and(|parent| !resolved[parent]) {
                    continue;
                }
                resolved[index] = true;
                evaluation_order.push(index);
                progress = true;
            }
            if !progress {
                return Err("skeleton hierarchy contains a cycle/unresolvable parent".to_owned());
            }
        }

        // Incremental pose solvers (IK, look-at, procedural appendages) mutate only a small
        // subtree at a time. Precompute the affected topological order once so hot paths never
        // rescan the full hierarchy or allocate descendant scratch per correction.
        let mut subtree_evaluation_order = vec![Vec::new(); joint_count];
        for &index in &evaluation_order {
            let mut cursor = Some(index);
            while let Some(ancestor) = cursor {
                subtree_evaluation_order[ancestor].push(index);
                cursor = parent_indices[ancestor];
            }
        }

        let bind_locals = skeleton
            .joints
            .iter()
            .map(|joint| JointLocalPose {
                translation: joint.position_ls,
                rotation: joint.rotation_ls,
                scale: Some(joint.scale_ls),
            })
            .collect::<Vec<_>>();

        let mut bind_globals = vec![Mat4::IDENTITY; joint_count];
        for &index in &evaluation_order {
            let local = validated_local_matrix(
                bind_locals[index],
                skeleton.joints[index].scale_ls,
                index,
                true,
            )?;
            bind_globals[index] = parent_indices[index]
                .map(|parent| bind_globals[parent] * local)
                .unwrap_or(local);
            if !affine_invertible(bind_globals[index]) {
                return Err(format!("bind global matrix is singular joint={index}"));
            }
        }

        let bind_global_inverse = bind_globals
            .iter()
            .copied()
            .map(|matrix| matrix.inverse())
            .collect::<Vec<_>>();
        let bind_joint_frames = bind_globals
            .iter()
            .copied()
            .map(|bind| source_to_model * bind)
            .collect::<Vec<_>>();

        // Keep the dense-tag fast path, but prebuild a map for native sparse tags.
        // Duplicate non-dense tags are deliberately ambiguous and rejected at bind time.
        let joint_tags = skeleton
            .joints
            .iter()
            .map(|joint| joint.tag)
            .collect::<Vec<_>>();
        let mut tag_to_joint = HashMap::with_capacity(joint_count);
        let mut ambiguous_tags = HashSet::new();
        for (index, tag) in joint_tags.iter().copied().enumerate() {
            if tag_to_joint.insert(tag, index).is_some() {
                ambiguous_tags.insert(tag);
            }
        }

        // A compiled bind pose is a hard invariant: deformation must be identity.
        const MAX_BIND_IDENTITY_ERROR: f32 = 1.0e-4;
        let identity = Mat4::IDENTITY.to_cols_array();
        for index in 0..joint_count {
            let palette = source_to_model
                * bind_globals[index]
                * bind_global_inverse[index]
                * model_to_source;
            let values = palette.to_cols_array();
            let error = values
                .iter()
                .zip(identity.iter())
                .map(|(actual, expected)| (actual - expected).abs())
                .fold(0.0_f32, f32::max);
            if !error.is_finite() || error > MAX_BIND_IDENTITY_ERROR {
                return Err(format!(
                    "compiled bind-pose palette is not identity joint={index} error={error:.8} limit={MAX_BIND_IDENTITY_ERROR}"
                ));
            }
        }

        let mut compatibility_key = 0xcbf2_9ce4_8422_2325_u64;
        for value in source_to_model.to_cols_array() {
            animation_fingerprint_mix(&mut compatibility_key, u64::from(value.to_bits()));
        }
        for (index, parent) in parent_indices.iter().copied().enumerate() {
            animation_fingerprint_mix(&mut compatibility_key, index as u64);
            animation_fingerprint_mix(
                &mut compatibility_key,
                parent.map(|value| value as u64 + 1).unwrap_or(0),
            );
            animation_fingerprint_mix(&mut compatibility_key, u64::from(joint_tags[index]));
            let bind = bind_locals[index];
            for value in bind.translation {
                animation_fingerprint_mix(&mut compatibility_key, u64::from(value.to_bits()));
            }
            for value in bind.rotation {
                animation_fingerprint_mix(&mut compatibility_key, u64::from(value.to_bits()));
            }
            for value in bind.scale.unwrap_or([1.0; 3]) {
                animation_fingerprint_mix(&mut compatibility_key, u64::from(value.to_bits()));
            }
        }

        Ok(Self {
            source_to_model,
            model_to_source,
            parent_indices,
            evaluation_order,
            subtree_evaluation_order,
            joint_tags,
            tag_to_joint,
            ambiguous_tags,
            bind_locals,
            bind_global_inverse,
            bind_joint_frames,
            compatibility_key,
        })
    }

    #[inline]
    pub fn joint_count(&self) -> usize {
        self.parent_indices.len()
    }

    /// Stable process-independent fingerprint of the compiled skeleton binding contract.
    /// Graph stores use it to prevent sharing clip bindings across incompatible skeletons.
    #[inline]
    pub fn compatibility_key(&self) -> u64 {
        self.compatibility_key
    }

    #[inline]
    pub fn bind_locals(&self) -> &[JointLocalPose] {
        &self.bind_locals
    }

    #[inline]
    pub fn bind_joint_frames(&self) -> &[Mat4] {
        &self.bind_joint_frames
    }

    fn resolve_joint_tag(&self, tag: u32) -> Result<usize, String> {
        let dense = tag as usize;
        if dense < self.joint_tags.len() && self.joint_tags[dense] == tag {
            return Ok(dense);
        }
        if self.ambiguous_tags.contains(&tag) {
            return Err(format!(
                "animation joint tag is ambiguous in skeleton tag={tag}"
            ));
        }
        self.tag_to_joint
            .get(&tag)
            .copied()
            .ok_or_else(|| format!("animation joint tag is absent from skeleton tag={tag}"))
    }

    /// Resolves every clip channel to a skeleton joint exactly once.
    pub fn bind_clip(&self, clip: &AnimationClip) -> Result<AnimationClipBinding, String> {
        clip.validate_structure()?;
        let mut seen = HashSet::with_capacity(clip.joint_tags.len());
        let mut clip_joint_to_skeleton = Vec::with_capacity(clip.joint_tags.len());
        for &tag in &clip.joint_tags {
            if !seen.insert(tag) {
                return Err(format!(
                    "animation clip contains duplicate joint tag tag={tag} clip='{}'",
                    clip.name
                ));
            }
            clip_joint_to_skeleton.push(
                self.resolve_joint_tag(tag)
                    .map_err(|error| format!("{error} clip='{}'", clip.name))?,
            );
        }

        let full_pose = clip_joint_to_skeleton.len() == self.joint_count()
            && clip_joint_to_skeleton
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len()
                == self.joint_count();

        Ok(AnimationClipBinding {
            clip_joint_to_skeleton,
            skeleton_joint_count: self.joint_count(),
            clip_joint_count: clip.joint_count(),
            full_pose,
        })
    }

    fn build_source_globals(
        &self,
        locals: &[JointLocalPose],
        out_globals: &mut Vec<Mat4>,
    ) -> Result<(), String> {
        if locals.len() != self.joint_count() {
            return Err(format!(
                "animation local pose count mismatch poses={} skeleton={}",
                locals.len(),
                self.joint_count()
            ));
        }

        out_globals.clear();
        out_globals.resize(self.joint_count(), Mat4::IDENTITY);
        for &index in &self.evaluation_order {
            let local = validated_local_matrix(
                locals[index],
                self.bind_locals[index].scale.unwrap_or([1.0, 1.0, 1.0]),
                index,
                false,
            )?;
            out_globals[index] = self.parent_indices[index]
                .map(|parent| out_globals[parent] * local)
                .unwrap_or(local);
            if !finite_matrix(out_globals[index]) {
                return Err(format!(
                    "animated global matrix contains non-finite value joint={index}"
                ));
            }
        }
        Ok(())
    }

    /// Builds a skinning palette without rebuilding bind hierarchy or inverse binds.
    pub fn build_skin_palette_from_local_pose(
        &self,
        locals: &[JointLocalPose],
        out_palette: &mut Vec<Mat4>,
    ) -> Result<(), String> {
        self.build_source_globals(locals, out_palette)?;
        for (index, animated_global) in out_palette.iter_mut().enumerate() {
            let palette = self.source_to_model
                * *animated_global
                * self.bind_global_inverse[index]
                * self.model_to_source;
            if !finite_matrix(palette) {
                return Err(format!(
                    "animated skin palette contains non-finite value joint={index}"
                ));
            }
            *animated_global = palette;
        }
        Ok(())
    }

    /// Builds absolute animated joint frames using the precompiled hierarchy order.
    pub fn build_model_joint_frames_from_local_pose(
        &self,
        locals: &[JointLocalPose],
        out_frames: &mut Vec<Mat4>,
    ) -> Result<(), String> {
        self.build_source_globals(locals, out_frames)?;
        for (index, frame) in out_frames.iter_mut().enumerate() {
            *frame = self.source_to_model * *frame;
            if !finite_matrix(*frame) {
                return Err(format!(
                    "animated joint frame contains non-finite value joint={index}"
                ));
            }
        }
        Ok(())
    }

    /// Refreshes one already-built model-space joint-frame subtree after local-pose mutation.
    ///
    /// This is the incremental counterpart to `build_model_joint_frames_from_local_pose`: callers
    /// first build a complete frame table, then procedural solvers may update a shoulder/elbow/etc.
    /// and propagate only that joint and its descendants. The compiled subtree order guarantees
    /// parent-before-child evaluation and preserves the exact full-FK transform convention.
    pub fn refresh_model_joint_frames_subtree_from_local_pose(
        &self,
        locals: &[JointLocalPose],
        out_frames: &mut [Mat4],
        root_joint: usize,
    ) -> Result<(), String> {
        if locals.len() != self.joint_count() {
            return Err(format!(
                "animation local pose count mismatch poses={} skeleton={}",
                locals.len(),
                self.joint_count()
            ));
        }
        if out_frames.len() != self.joint_count() {
            return Err(format!(
                "animation joint frame count mismatch frames={} skeleton={}",
                out_frames.len(),
                self.joint_count()
            ));
        }
        let order = self
            .subtree_evaluation_order
            .get(root_joint)
            .ok_or_else(|| {
                format!(
                    "animation subtree root outside joint table root={root_joint} skeleton={}",
                    self.joint_count()
                )
            })?;

        for &index in order {
            let local = validated_local_matrix(
                locals[index],
                self.bind_locals[index].scale.unwrap_or([1.0, 1.0, 1.0]),
                index,
                false,
            )?;
            let frame = self.parent_indices[index]
                .map(|parent| out_frames[parent] * local)
                .unwrap_or(self.source_to_model * local);
            if !finite_matrix(frame) {
                return Err(format!(
                    "animated joint frame contains non-finite value joint={index}"
                ));
            }
            out_frames[index] = frame;
        }
        Ok(())
    }
}

impl AnimationClip {
    /// Performs structural validation once before a clip enters a runtime binding.
    pub fn validate_structure(&self) -> Result<(), String> {
        let joint_count = self.joint_count();
        let frame_count = self.frame_count();
        if joint_count == 0 || frame_count == 0 {
            return Err(format!(
                "animation clip contains no sampled poses clip='{}'",
                self.name
            ));
        }
        if self.poses.len() != joint_count * frame_count {
            return Err(format!(
                "animation clip pose array is not frame/joint rectangular clip='{}' poses={} joints={} frames={}",
                self.name,
                self.poses.len(),
                joint_count,
                frame_count
            ));
        }
        if !self.duration_seconds.is_finite() || self.duration_seconds <= 0.0 {
            return Err(format!(
                "animation clip duration is invalid clip='{}' duration={}",
                self.name, self.duration_seconds
            ));
        }
        if !self.sample_rate_hz.is_finite() || self.sample_rate_hz <= 0.0 {
            return Err(format!(
                "animation clip sample rate is invalid clip='{}' sample_rate={}",
                self.name, self.sample_rate_hz
            ));
        }

        let mut seen = HashSet::with_capacity(self.joint_tags.len());
        for &tag in &self.joint_tags {
            if !seen.insert(tag) {
                return Err(format!(
                    "animation clip contains duplicate joint tag tag={tag} clip='{}'",
                    self.name
                ));
            }
        }

        self.validate_events()?;

        for (pose_index, pose) in self.poses.iter().copied().enumerate() {
            if pose.translation.iter().any(|value| !value.is_finite()) {
                return Err(format!(
                    "animation clip translation is non-finite clip='{}' pose={pose_index}",
                    self.name
                ));
            }
            let rotation = quat(pose.rotation);
            let len2 = rotation.length_squared();
            if !len2.is_finite() || len2 <= 1.0e-8 {
                return Err(format!(
                    "animation clip quaternion is invalid clip='{}' pose={pose_index} len2={len2}",
                    self.name
                ));
            }
            if pose
                .scale
                .is_some_and(|scale| scale.iter().any(|value| !value.is_finite()))
            {
                return Err(format!(
                    "animation clip scale is non-finite clip='{}' pose={pose_index}",
                    self.name
                ));
            }
        }
        Ok(())
    }

    #[inline]
    pub fn bind_to_skeleton(
        &self,
        skeleton: &AnimationSkeletonRuntime,
    ) -> Result<AnimationClipBinding, String> {
        skeleton.bind_clip(self)
    }

    /// Samples through a precompiled clip/skeleton binding.
    ///
    /// No joint tag search, duplicate-tag scan, hierarchy sort or inverse-bind work occurs in
    /// this frame path.
    pub fn sample_local_pose_bound(
        &self,
        time_seconds: f32,
        skeleton: &AnimationSkeletonRuntime,
        binding: &AnimationClipBinding,
        out: &mut Vec<JointLocalPose>,
    ) -> Result<(), String> {
        self.sample_local_pose_bound_impl(time_seconds, skeleton, binding, out, true)
    }

    /// Samples a clip without replacing joints absent from a partial binding with bind pose.
    ///
    /// Full-body presentation layers use this path so sparse authored clips can own only the
    /// joints they actually contain while the previously visible pose remains authoritative for
    /// every untracked joint. This prevents bind/default-pose flashes during an active animation.
    pub fn sample_local_pose_bound_preserve_untracked(
        &self,
        time_seconds: f32,
        skeleton: &AnimationSkeletonRuntime,
        binding: &AnimationClipBinding,
        out: &mut Vec<JointLocalPose>,
    ) -> Result<(), String> {
        self.sample_local_pose_bound_impl(time_seconds, skeleton, binding, out, false)
    }

    fn sample_local_pose_bound_impl(
        &self,
        time_seconds: f32,
        skeleton: &AnimationSkeletonRuntime,
        binding: &AnimationClipBinding,
        out: &mut Vec<JointLocalPose>,
        reset_untracked_to_bind: bool,
    ) -> Result<(), String> {
        if binding.skeleton_joint_count != skeleton.joint_count()
            || binding.clip_joint_count != self.joint_count()
            || binding.clip_joint_to_skeleton.len() != self.joint_count()
        {
            return Err(format!(
                "animation clip binding does not match clip/skeleton clip='{}' binding_clip_joints={} actual_clip_joints={} binding_skeleton_joints={} actual_skeleton_joints={}",
                self.name,
                binding.clip_joint_count,
                self.joint_count(),
                binding.skeleton_joint_count,
                skeleton.joint_count()
            ));
        }

        let frame_count = self.frame_count();
        if frame_count == 0 {
            return Err(format!(
                "animation clip contains no frames clip='{}'",
                self.name
            ));
        }

        let duration = self.duration_seconds.max(1.0e-6);
        let mut t = if time_seconds.is_finite() {
            time_seconds.max(0.0)
        } else {
            0.0
        };
        if self.looped {
            t = t.rem_euclid(duration);
        } else {
            t = t.min(duration);
        }

        let mut frame_position = t * self.sample_rate_hz.max(1.0e-6);
        if self.looped {
            frame_position = frame_position.rem_euclid(frame_count as f32);
        } else {
            frame_position = frame_position.min((frame_count - 1) as f32);
        }
        let base = frame_position.floor() as usize;
        let alpha = frame_position - base as f32;
        let frame0 = base.min(frame_count - 1);
        let frame1 = if self.looped {
            (frame0 + 1) % frame_count
        } else {
            (frame0 + 1).min(frame_count - 1)
        };

        if out.len() != skeleton.joint_count() || (reset_untracked_to_bind && !binding.full_pose) {
            out.clear();
            out.extend_from_slice(skeleton.bind_locals());
        }

        let clip_joint_count = self.joint_count();
        for clip_joint in 0..clip_joint_count {
            let a = self.poses[frame0 * clip_joint_count + clip_joint];
            let b = self.poses[frame1 * clip_joint_count + clip_joint];
            let translation = vec3(a.translation).lerp(vec3(b.translation), alpha);
            let qa = quat(a.rotation).normalize();
            let mut qb = quat(b.rotation).normalize();
            if qa.dot(qb) < 0.0 {
                qb = Quat::from_xyzw(-qb.x, -qb.y, -qb.z, -qb.w);
            }
            let rotation = qa.slerp(qb, alpha).normalize();
            let scale = match (a.scale, b.scale) {
                (Some(a), Some(b)) => Some(vec3_array(vec3(a).lerp(vec3(b), alpha))),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
            out[binding.clip_joint_to_skeleton[clip_joint]] = JointLocalPose {
                translation: vec3_array(translation),
                rotation: quat_array(rotation),
                scale,
            };
        }
        Ok(())
    }
}
