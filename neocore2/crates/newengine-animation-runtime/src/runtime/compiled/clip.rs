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
