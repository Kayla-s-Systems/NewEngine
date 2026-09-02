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
        let mut skeleton_joint_to_clip = vec![None; self.joint_count()];
        for (clip_joint, &tag) in clip.joint_tags.iter().enumerate() {
            if !seen.insert(tag) {
                return Err(format!(
                    "animation clip contains duplicate joint tag tag={tag} clip='{}'",
                    clip.name
                ));
            }
            let skeleton_joint = self
                .resolve_joint_tag(tag)
                .map_err(|error| format!("{error} clip='{}'", clip.name))?;
            if skeleton_joint_to_clip[skeleton_joint]
                .replace(clip_joint)
                .is_some()
            {
                return Err(format!(
                    "animation clip resolves multiple channels to one skeleton joint skeleton_joint={} clip='{}'",
                    skeleton_joint, clip.name
                ));
            }
            clip_joint_to_skeleton.push(skeleton_joint);
        }

        let full_pose = clip_joint_to_skeleton.len() == self.joint_count()
            && skeleton_joint_to_clip.iter().all(Option::is_some);

        Ok(AnimationClipBinding {
            clip_joint_to_skeleton,
            skeleton_joint_to_clip,
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
