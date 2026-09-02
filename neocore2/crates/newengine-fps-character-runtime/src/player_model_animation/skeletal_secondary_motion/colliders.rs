#[derive(Clone, Copy, Debug)]
struct SecondaryMotionCapsuleBinding {
    joint: usize,
    mode: PlayerSecondaryMotionColliderMode,
    local_a: Vec3,
    local_b: Vec3,
    radius: f32,
}

#[derive(Clone, Copy, Debug)]
struct SecondaryMotionOrientedBoxBinding {
    joint: usize,
    mode: PlayerSecondaryMotionColliderMode,
    local_center: Vec3,
    local_axes: [Vec3; 3],
    half_extents: Vec3,
}

#[derive(Clone, Debug, Default)]
struct SecondaryMotionColliderBindings {
    capsules: Vec<SecondaryMotionCapsuleBinding>,
    boxes: Vec<SecondaryMotionOrientedBoxBinding>,
}

#[derive(Clone, Copy, Debug)]
struct SecondaryMotionCapsule {
    mode: PlayerSecondaryMotionColliderMode,
    a: Vec3,
    b: Vec3,
    radius: f32,
}

#[derive(Clone, Copy, Debug)]
struct SecondaryMotionOrientedBox {
    mode: PlayerSecondaryMotionColliderMode,
    center: Vec3,
    axes: [Vec3; 3],
    half_extents: Vec3,
}

#[derive(Clone, Debug, Default)]
struct SecondaryMotionColliderSet {
    capsules: Vec<SecondaryMotionCapsule>,
    boxes: Vec<SecondaryMotionOrientedBox>,
}

#[inline]
fn secondary_motion_vec3(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

#[inline]
fn secondary_motion_joint(skeleton: &ModelSkeletonMetadata, name: &str) -> Option<usize> {
    skeleton.joints.iter().position(|joint| joint.name == name)
}

fn secondary_motion_bind_inverse(
    joint: usize,
    label: &str,
    bind_joint_frames: &[Mat4],
) -> Result<Mat4, String> {
    let bind = bind_joint_frames.get(joint).copied().ok_or_else(|| {
        format!(
            "skeletal secondary-motion collider bind joint outside frame table label={label} joint={joint} frames={}",
            bind_joint_frames.len()
        )
    })?;
    let inverse = bind.inverse();
    if inverse
        .to_cols_array()
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(format!(
            "skeletal secondary-motion collider bind frame is singular/non-finite label={label} joint={joint}"
        ));
    }
    Ok(inverse)
}

impl SecondaryMotionColliderBindings {
    fn from_authored(
        authored: &PlayerSkeletalSecondaryMotionRig,
        skeleton: &ModelSkeletonMetadata,
        bind_joint_frames: &[Mat4],
        source_to_model: Mat4,
    ) -> Result<Self, String> {
        let mut capsules = Vec::with_capacity(authored.collision_capsules.len());
        for (index, capsule) in authored.collision_capsules.iter().enumerate() {
            let joint =
                secondary_motion_joint(skeleton, capsule.joint.trim()).ok_or_else(|| {
                    format!(
                        "skeletal secondary-motion capsule[{index}] joint '{}' is missing",
                        capsule.joint
                    )
                })?;
            let inverse = secondary_motion_bind_inverse(joint, "capsule", bind_joint_frames)?;
            let source_a =
                source_to_model.transform_point3(secondary_motion_vec3(capsule.source_a));
            let source_b =
                source_to_model.transform_point3(secondary_motion_vec3(capsule.source_b));
            capsules.push(SecondaryMotionCapsuleBinding {
                joint,
                mode: capsule.mode,
                local_a: inverse.transform_point3(source_a),
                local_b: inverse.transform_point3(source_b),
                radius: capsule.radius,
            });
        }

        let mut boxes = Vec::with_capacity(authored.collision_boxes.len());
        for (index, box_shape) in authored.collision_boxes.iter().enumerate() {
            let joint =
                secondary_motion_joint(skeleton, box_shape.joint.trim()).ok_or_else(|| {
                    format!(
                        "skeletal secondary-motion oriented_box[{index}] joint '{}' is missing",
                        box_shape.joint
                    )
                })?;
            let inverse = secondary_motion_bind_inverse(joint, "oriented-box", bind_joint_frames)?;
            let source_center =
                source_to_model.transform_point3(secondary_motion_vec3(box_shape.source_center));
            let mut local_axes = [Vec3::ZERO; 3];
            for (axis_index, authored_axis) in box_shape.source_axes.iter().copied().enumerate() {
                let axis = source_to_model
                    .transform_vector3(secondary_motion_vec3(authored_axis))
                    .normalize_or_zero();
                let local = inverse.transform_vector3(axis).normalize_or_zero();
                if local.length_squared() <= 1.0e-8 {
                    return Err(format!(
                        "skeletal secondary-motion oriented_box[{index}] axis collapsed joint={} axis={axis_index}",
                        box_shape.joint
                    ));
                }
                local_axes[axis_index] = local;
            }
            boxes.push(SecondaryMotionOrientedBoxBinding {
                joint,
                mode: box_shape.mode,
                local_center: inverse.transform_point3(source_center),
                local_axes,
                half_extents: secondary_motion_vec3(box_shape.half_extents),
            });
        }

        Ok(Self { capsules, boxes })
    }

    fn from_joint_frames(
        &self,
        joint_frames: &[Mat4],
    ) -> Result<SecondaryMotionColliderSet, String> {
        let mut capsules = Vec::with_capacity(self.capsules.len());
        for (index, binding) in self.capsules.iter().copied().enumerate() {
            let frame = joint_frames.get(binding.joint).copied().ok_or_else(|| {
                format!(
                    "skeletal secondary-motion capsule[{index}] joint outside animated frame table joint={} frames={}",
                    binding.joint,
                    joint_frames.len()
                )
            })?;
            capsules.push(SecondaryMotionCapsule {
                mode: binding.mode,
                a: frame.transform_point3(binding.local_a),
                b: frame.transform_point3(binding.local_b),
                radius: binding.radius,
            });
        }

        let mut boxes = Vec::with_capacity(self.boxes.len());
        for (index, binding) in self.boxes.iter().copied().enumerate() {
            let frame = joint_frames.get(binding.joint).copied().ok_or_else(|| {
                format!(
                    "skeletal secondary-motion oriented_box[{index}] joint outside animated frame table joint={} frames={}",
                    binding.joint,
                    joint_frames.len()
                )
            })?;
            let mut axes = [Vec3::ZERO; 3];
            for (axis_index, local_axis) in binding.local_axes.into_iter().enumerate() {
                let axis = frame.transform_vector3(local_axis).normalize_or_zero();
                if axis.length_squared() <= 1.0e-8 {
                    return Err(format!(
                        "skeletal secondary-motion animated oriented_box[{index}] axis collapsed joint={} axis={axis_index}",
                        binding.joint
                    ));
                }
                axes[axis_index] = axis;
            }
            boxes.push(SecondaryMotionOrientedBox {
                mode: binding.mode,
                center: frame.transform_point3(binding.local_center),
                axes,
                half_extents: binding.half_extents,
            });
        }
        Ok(SecondaryMotionColliderSet { capsules, boxes })
    }
}

