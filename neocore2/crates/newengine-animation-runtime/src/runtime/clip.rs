#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointLocalPose {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    /// Authored local scale. `None` is the legacy YCD v1 representation and
    /// resolves to the skeleton bind scale when building matrices.
    pub scale: Option<[f32; 3]>,
}

impl JointLocalPose {
    #[inline]
    pub fn matrix(self, fallback_scale: [f32; 3]) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            vec3(self.scale.unwrap_or(fallback_scale)),
            quat(self.rotation).normalize(),
            vec3(self.translation),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimationClip {
    pub name: String,
    pub skeleton_ref: String,
    pub source: String,
    pub duration_seconds: f32,
    pub sample_rate_hz: f32,
    pub looped: bool,
    pub joint_tags: Vec<u32>,
    /// Frame-major local poses: `frame * joint_count + joint_index`.
    pub poses: Vec<JointLocalPose>,
}

impl AnimationClip {
    #[inline]
    pub fn joint_count(&self) -> usize {
        self.joint_tags.len()
    }

    #[inline]
    pub fn frame_count(&self) -> usize {
        let joints = self.joint_count();
        if joints == 0 {
            0
        } else {
            self.poses.len() / joints
        }
    }

    pub fn sample_local_pose(
        &self,
        time_seconds: f32,
        out: &mut Vec<JointLocalPose>,
    ) -> Result<(), String> {
        let joint_count = self.joint_count();
        let frame_count = self.frame_count();
        if joint_count == 0 || frame_count == 0 {
            return Err("animation clip contains no sampled poses".to_owned());
        }
        if self.poses.len() != joint_count * frame_count {
            return Err("animation clip pose array is not frame/joint rectangular".to_owned());
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
        let frame_position = t * self.sample_rate_hz.max(1.0e-6);
        let base = frame_position.floor() as usize;
        let alpha = frame_position - base as f32;
        let frame0 = base.min(frame_count - 1);
        let frame1 = if self.looped {
            (frame0 + 1) % frame_count
        } else {
            (frame0 + 1).min(frame_count - 1)
        };
        out.clear();
        out.reserve(joint_count);
        for joint in 0..joint_count {
            let a = self.poses[frame0 * joint_count + joint];
            let b = self.poses[frame1 * joint_count + joint];
            let translation = vec3(a.translation).lerp(vec3(b.translation), alpha);
            let mut qa = quat(a.rotation).normalize();
            let mut qb = quat(b.rotation).normalize();
            if qa.dot(qb) < 0.0 {
                qb = Quat::from_xyzw(-qb.x, -qb.y, -qb.z, -qb.w);
            }
            qa = qa.slerp(qb, alpha).normalize();
            let scale = match (a.scale, b.scale) {
                (Some(a), Some(b)) => Some(vec3_array(vec3(a).lerp(vec3(b), alpha))),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
            out.push(JointLocalPose {
                translation: vec3_array(translation),
                rotation: quat_array(qa),
                scale,
            });
        }
        Ok(())
    }

    /// Samples this clip directly into a complete skeleton-local pose.
    ///
    /// YCD joint tags are authoritative addresses, not an implicit requirement that every
    /// clip carry every skeleton joint. Missing tags retain the authored bind pose. This keeps
    /// native North Star channel subsets compact while preserving legacy full-pose clips.
    pub fn sample_local_pose_for_skeleton(
        &self,
        time_seconds: f32,
        skeleton: &ModelSkeletonMetadata,
        out: &mut Vec<JointLocalPose>,
    ) -> Result<(), String> {
        let clip_joint_count = self.joint_count();
        let frame_count = self.frame_count();
        if clip_joint_count == 0 || frame_count == 0 {
            return Err("animation clip contains no sampled poses".to_owned());
        }
        if self.poses.len() != clip_joint_count * frame_count {
            return Err("animation clip pose array is not frame/joint rectangular".to_owned());
        }
        if skeleton.joints.is_empty() {
            return Err("animation sampling requires a non-empty skeleton".to_owned());
        }
        for (index, joint) in skeleton.joints.iter().enumerate() {
            if joint.index as usize != index {
                return Err(format!(
                    "skeleton joint indices must be dense index={} authored={}",
                    index, joint.index
                ));
            }
        }

        out.clear();
        out.reserve(skeleton.joints.len());
        out.extend(skeleton.joints.iter().map(|joint| JointLocalPose {
            translation: joint.position_ls,
            rotation: joint.rotation_ls,
            scale: Some(joint.scale_ls),
        }));

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
        let frame_position = t * self.sample_rate_hz.max(1.0e-6);
        let base = frame_position.floor() as usize;
        let alpha = frame_position - base as f32;
        let frame0 = base.min(frame_count - 1);
        let frame1 = if self.looped {
            (frame0 + 1) % frame_count
        } else {
            (frame0 + 1).min(frame_count - 1)
        };

        for clip_joint in 0..clip_joint_count {
            let tag = self.joint_tags[clip_joint];
            if self.joint_tags[..clip_joint].contains(&tag) {
                return Err(format!("animation clip contains duplicate joint tag {tag}"));
            }
            let dense = tag as usize;
            let skeleton_joint =
                if dense < skeleton.joints.len() && skeleton.joints[dense].tag == tag {
                    dense
                } else {
                    skeleton
                        .joints
                        .iter()
                        .position(|joint| joint.tag == tag)
                        .ok_or_else(|| {
                            format!(
                                "animation joint tag is absent from skeleton tag={} clip='{}'",
                                tag, self.name
                            )
                        })?
                };
            let a = self.poses[frame0 * clip_joint_count + clip_joint];
            let b = self.poses[frame1 * clip_joint_count + clip_joint];
            let translation = vec3(a.translation).lerp(vec3(b.translation), alpha);
            let mut qa = quat(a.rotation).normalize();
            let mut qb = quat(b.rotation).normalize();
            if qa.dot(qb) < 0.0 {
                qb = Quat::from_xyzw(-qb.x, -qb.y, -qb.z, -qb.w);
            }
            qa = qa.slerp(qb, alpha).normalize();
            let scale = match (a.scale, b.scale) {
                (Some(a), Some(b)) => Some(vec3_array(vec3(a).lerp(vec3(b), alpha))),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
            out[skeleton_joint] = JointLocalPose {
                translation: vec3_array(translation),
                rotation: quat_array(qa),
                scale,
            };
        }
        Ok(())
    }
}
