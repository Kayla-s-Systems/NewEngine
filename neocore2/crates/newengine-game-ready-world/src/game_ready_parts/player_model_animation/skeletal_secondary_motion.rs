use newengine_engine_runtime::gameplay::{
    PlayerSecondaryMotionColliderMode, PlayerSkeletalSecondaryMotionRig,
};

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

#[derive(Clone, Debug)]
struct SkeletalSecondaryMotionRuntime {
    authored: PlayerSkeletalSecondaryMotionRig,
    attachment_joint: usize,
    chain_joints: Vec<usize>,
    collider_bindings: SecondaryMotionColliderBindings,
    attachment_local_points: Vec<Vec3>,
    bind_chain_points: Vec<Vec3>,
    bind_chain_frames: Vec<Mat4>,
    points: Vec<Vec3>,
    previous_points: Vec<Vec3>,
    previous_root_velocity_local: Vec3,
    last_root_position: Option<Vec3>,
    last_root_rotation: Option<Quat>,
    reset_pending: bool,
    initialized: bool,
}

impl SkeletalSecondaryMotionRuntime {
    fn new(
        authored: &PlayerSkeletalSecondaryMotionRig,
        chain_joints: Vec<usize>,
        skeleton: &ModelSkeletonMetadata,
        source_to_model: [f32; 16],
        bind_joint_frames: &[Mat4],
    ) -> Result<Self, String> {
        let attachment_joint = *chain_joints
            .first()
            .ok_or_else(|| "skeletal secondary-motion chain is empty".to_owned())?;
        let source_to_model = Mat4::from_cols_array(&source_to_model);
        let attachment_bind = *bind_joint_frames.get(attachment_joint).ok_or_else(|| {
            format!(
                "skeletal secondary-motion attachment bind joint outside frame table joint={attachment_joint} frames={}",
                bind_joint_frames.len()
            )
        })?;
        let attachment_bind_inverse = attachment_bind.inverse();
        if attachment_bind_inverse
            .to_cols_array()
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(
                "skeletal secondary-motion attachment bind frame is singular/non-finite".to_owned(),
            );
        }

        let collider_bindings = SecondaryMotionColliderBindings::from_authored(
            authored,
            skeleton,
            bind_joint_frames,
            source_to_model,
        )?;
        let bind_particles = authored
            .particles
            .iter()
            .map(|particle| {
                source_to_model.transform_point3(secondary_motion_vec3(particle.rest_position))
            })
            .collect::<Vec<_>>();
        let attachment_local_points = bind_particles
            .iter()
            .copied()
            .map(|point| attachment_bind_inverse.transform_point3(point))
            .collect::<Vec<_>>();

        let mut bind_chain_points = Vec::with_capacity(chain_joints.len());
        let mut bind_chain_frames = Vec::with_capacity(chain_joints.len());
        for (lane, joint) in chain_joints.iter().copied().enumerate() {
            let frame = *bind_joint_frames.get(joint).ok_or_else(|| {
                format!(
                    "skeletal secondary-motion chain joint outside bind frame table lane={lane} joint={joint}"
                )
            })?;
            bind_chain_frames.push(frame);
            bind_chain_points.push(frame.transform_point3(Vec3::ZERO));
        }

        Ok(Self {
            authored: authored.clone(),
            attachment_joint,
            chain_joints,
            collider_bindings,
            attachment_local_points,
            bind_chain_points,
            bind_chain_frames,
            points: bind_particles.clone(),
            previous_points: bind_particles,
            previous_root_velocity_local: Vec3::ZERO,
            last_root_position: None,
            last_root_rotation: None,
            reset_pending: true,
            initialized: false,
        })
    }

    fn reset(&mut self, guide: &[Vec3], root_velocity_local: Vec3) {
        self.points.clone_from_slice(guide);
        self.previous_points.clone_from_slice(guide);
        self.previous_root_velocity_local = root_velocity_local;
        self.reset_pending = false;
        self.initialized = true;
    }

    fn tick(
        &mut self,
        dt: f32,
        root_velocity_local: Vec3,
        root_position: Vec3,
        root_rotation: Quat,
        joint_frames: &[Mat4],
        palette: &mut [Mat4],
    ) -> Result<(), String> {
        let attachment = *joint_frames
            .get(self.attachment_joint)
            .ok_or_else(|| "skeletal secondary-motion attachment frame missing".to_owned())?;
        let particle_guide = self
            .attachment_local_points
            .iter()
            .copied()
            .map(|point| attachment.transform_point3(point))
            .collect::<Vec<_>>();
        let mut chain_guide = Vec::with_capacity(self.chain_joints.len());
        for (lane, joint) in self.chain_joints.iter().copied().enumerate() {
            chain_guide.push(
                joint_frames
                    .get(joint)
                    .copied()
                    .ok_or_else(|| {
                        format!(
                            "skeletal secondary-motion animated chain frame missing lane={lane} joint={joint}"
                        )
                    })?
                    .transform_point3(Vec3::ZERO),
            );
        }
        let colliders = self.collider_bindings.from_joint_frames(joint_frames)?;
        let one_sided_back_normal = colliders
            .boxes
            .iter()
            .find(|box_shape| box_shape.mode == PlayerSecondaryMotionColliderMode::OneSidedBack)
            .map(|box_shape| box_shape.axes[2]);
        if colliders
            .capsules
            .iter()
            .any(|capsule| capsule.mode == PlayerSecondaryMotionColliderMode::OneSidedBack)
            && one_sided_back_normal.is_none()
        {
            return Err(
                "skeletal secondary-motion one-sided capsule requires a one-sided oriented box to author the back normal"
                    .to_owned(),
            );
        }

        let tuning = &self.authored.tuning;
        let root_rotation = root_rotation.normalize_or_identity();
        if self.last_root_position.is_some_and(|position| {
            (root_position - position).length() > tuning.teleport_reset_distance
        }) {
            self.reset_pending = true;
        }
        if self.last_root_rotation.is_some_and(|rotation| {
            rotation.normalize_or_identity().dot(root_rotation).abs()
                < tuning.teleport_reset_quat_dot
        }) {
            self.reset_pending = true;
        }
        self.last_root_position = Some(root_position);
        self.last_root_rotation = Some(root_rotation);

        if !self.initialized || self.reset_pending {
            self.reset(&particle_guide, root_velocity_local);
        } else if dt > 0.0 {
            let frame_dt = dt.clamp(1.0 / 240.0, 1.0 / 20.0);
            let substeps = usize::from(tuning.solver_substeps.max(1));
            let iterations = usize::from(tuning.solver_iterations.max(1));
            let step_dt = frame_dt / substeps as f32;
            let mut root_acceleration_local =
                (root_velocity_local - self.previous_root_velocity_local) / frame_dt.max(1.0e-5);
            let acceleration_len = root_acceleration_local.length();
            if tuning.max_root_acceleration > 0.0 && acceleration_len > tuning.max_root_acceleration
            {
                root_acceleration_local *= tuning.max_root_acceleration / acceleration_len;
            }
            self.previous_root_velocity_local = root_velocity_local;
            let gravity = Vec3::new(0.0, 9.81 * tuning.gravity_scale * step_dt * step_dt, 0.0);
            let inertial_base =
                -root_acceleration_local * (tuning.inertia_scale * step_dt * step_dt);
            let inertia_reference = tuning.inertia_scale.abs().max(1.0e-6);
            let velocity_retention = (1.0 - tuning.velocity_damping).clamp(0.0, 1.0);
            let collision_margin =
                tuning.collision_margin.max(0.0) + tuning.back_clearance.max(0.0);

            for _ in 0..substeps {
                for (index, particle) in self.authored.particles.iter().enumerate() {
                    let mobility = particle.mobility.max(0.0);
                    if mobility <= 1.0e-8 {
                        self.points[index] = particle_guide[index];
                        self.previous_points[index] = particle_guide[index];
                        continue;
                    }
                    let current = self.points[index];
                    let velocity = (current - self.previous_points[index]) * velocity_retention;
                    self.previous_points[index] = current;
                    let inertia_weight = (particle.inertia / inertia_reference).clamp(0.0, 1.0);
                    self.points[index] =
                        current + velocity + gravity + inertial_base * inertia_weight;
                }

                for _ in 0..iterations {
                    pin_secondary_motion_particles(
                        &mut self.points,
                        &particle_guide,
                        &self.authored,
                    );
                    for edge in &self.authored.edges {
                        solve_secondary_motion_edge(
                            &mut self.points,
                            &self.authored,
                            edge.a,
                            edge.b,
                            edge.rest_length,
                            edge.stiffness,
                        );
                    }
                    for bend in &self.authored.bends {
                        solve_secondary_motion_bend(
                            &mut self.points,
                            &particle_guide,
                            &self.authored,
                            bend.indices,
                            bend.weights,
                            bend.geometry_scale,
                            bend.rest_scalar,
                        );
                    }

                    for (index, particle) in self.authored.particles.iter().enumerate() {
                        if particle.mobility <= 1.0e-8 {
                            continue;
                        }
                        let follow = (particle.follow * tuning.follow_scale).clamp(0.0, 1.0);
                        self.points[index] = self.points[index].lerp(particle_guide[index], follow);

                        for capsule in colliders.capsules.iter().copied() {
                            match capsule.mode {
                                PlayerSecondaryMotionColliderMode::Exterior => {
                                    project_out_of_secondary_motion_capsule(
                                        &mut self.points[index],
                                        capsule.a,
                                        capsule.b,
                                        capsule.radius + collision_margin,
                                    );
                                }
                                PlayerSecondaryMotionColliderMode::OneSidedBack => {
                                    project_behind_secondary_motion_capsule(
                                        &mut self.points[index],
                                        capsule,
                                        one_sided_back_normal
                                            .expect("validated one-sided back normal"),
                                        collision_margin,
                                    );
                                }
                            }
                        }
                        for mut box_shape in colliders.boxes.iter().copied() {
                            box_shape.half_extents += Vec3::splat(collision_margin);
                            match box_shape.mode {
                                PlayerSecondaryMotionColliderMode::Exterior => {
                                    project_out_of_secondary_motion_box(
                                        &mut self.points[index],
                                        box_shape,
                                    );
                                }
                                PlayerSecondaryMotionColliderMode::OneSidedBack => {
                                    project_behind_secondary_motion_box(
                                        &mut self.points[index],
                                        box_shape,
                                        tuning.tunnel_depth,
                                    );
                                }
                            }
                        }
                    }

                    for edge in &self.authored.edges {
                        solve_secondary_motion_edge(
                            &mut self.points,
                            &self.authored,
                            edge.a,
                            edge.b,
                            edge.rest_length,
                            edge.stiffness,
                        );
                    }
                    pin_secondary_motion_particles(
                        &mut self.points,
                        &particle_guide,
                        &self.authored,
                    );
                }

                for edge in &self.authored.edges {
                    damp_secondary_motion_edge_velocity(
                        &self.points,
                        &mut self.previous_points,
                        &self.authored,
                        edge.a,
                        edge.b,
                        edge.damping,
                    );
                }
                pin_secondary_motion_particles(&mut self.points, &particle_guide, &self.authored);
            }
        }

        let guide_centerline = secondary_motion_centerline(&particle_guide, &self.authored);
        let current_centerline = secondary_motion_centerline(&self.points, &self.authored);
        let mut desired = chain_guide.clone();
        for lane in self.authored.dynamic_start..self.chain_joints.len() {
            let t = normalized_polyline_parameter(&self.bind_chain_points, lane);
            desired[lane] += sample_polyline_normalized(&current_centerline, t)
                - sample_polyline_normalized(&guide_centerline, t);
        }
        for lane in self.authored.dynamic_start..self.chain_joints.len() {
            let joint = self.chain_joints[lane];
            let guide_direction = if lane + 1 < self.chain_joints.len() {
                chain_guide[lane + 1] - chain_guide[lane]
            } else {
                chain_guide[lane] - chain_guide[lane - 1]
            }
            .normalize_or_zero();
            let current_direction = if lane + 1 < self.chain_joints.len() {
                desired[lane + 1] - desired[lane]
            } else {
                desired[lane] - desired[lane - 1]
            }
            .normalize_or_zero();
            let bend = if guide_direction.length_squared() > 1.0e-8
                && current_direction.length_squared() > 1.0e-8
            {
                Quat::from_rotation_arc(guide_direction, current_direction)
            } else {
                Quat::IDENTITY
            };
            let base_frame = *joint_frames.get(joint).ok_or_else(|| {
                format!(
                    "skeletal secondary-motion chain palette joint outside animated frame table lane={lane} joint={joint}"
                )
            })?;
            let desired_frame = Mat4::from_translation(desired[lane])
                * Mat4::from_quat(bend)
                * Mat4::from_translation(-chain_guide[lane])
                * base_frame;
            let bind_inverse = self.bind_chain_frames[lane].inverse();
            let deformation = desired_frame * bind_inverse;
            if deformation
                .to_cols_array()
                .iter()
                .any(|value| !value.is_finite())
            {
                return Err(format!(
                    "skeletal secondary-motion deformation became non-finite lane={lane} joint={joint}"
                ));
            }
            let palette_len = palette.len();
            let target = palette.get_mut(joint).ok_or_else(|| {
                format!(
                    "skeletal secondary-motion chain palette joint outside skin palette lane={lane} joint={joint} palette={}",
                    palette_len
                )
            })?;
            *target = deformation;
        }
        Ok(())
    }
}

fn validate_skeletal_secondary_motion(
    authored: &PlayerSkeletalSecondaryMotionRig,
) -> Result<(), String> {
    if authored.chain_joints.len() < 2 {
        return Err("skeletal secondary-motion requires at least two chain joints".to_owned());
    }
    if authored.dynamic_start == 0 || authored.dynamic_start >= authored.chain_joints.len() {
        return Err(format!(
            "skeletal secondary-motion dynamic_start={} outside movable chain range 1..{}",
            authored.dynamic_start,
            authored.chain_joints.len()
        ));
    }
    if authored.particles.is_empty() {
        return Err("skeletal secondary-motion requires particles".to_owned());
    }
    if authored.centerline_pairs.len() < 2 {
        return Err("skeletal secondary-motion requires at least two centerline pairs".to_owned());
    }
    if authored.tuning.solver_substeps == 0 || authored.tuning.solver_iterations == 0 {
        return Err(
            "skeletal secondary-motion solver substeps/iterations must be non-zero".to_owned(),
        );
    }
    if authored.tuning.stretch_reference_stiffness <= 0.0
        || authored.tuning.bend_reference_stiffness <= 0.0
    {
        return Err(
            "skeletal secondary-motion reference stiffness values must be positive".to_owned(),
        );
    }

    let particle_count = authored.particles.len();
    for (index, particle) in authored.particles.iter().enumerate() {
        if particle
            .rest_position
            .iter()
            .chain([&particle.mobility, &particle.follow, &particle.inertia])
            .any(|value| !value.is_finite())
        {
            return Err(format!(
                "skeletal secondary-motion particle[{index}] contains non-finite data"
            ));
        }
    }
    for (index, edge) in authored.edges.iter().enumerate() {
        if edge.a >= particle_count || edge.b >= particle_count {
            return Err(format!(
                "skeletal secondary-motion edge[{index}] index outside particles a={} b={} particles={particle_count}",
                edge.a, edge.b
            ));
        }
        if edge.rest_length <= 0.0
            || !edge.rest_length.is_finite()
            || !edge.stiffness.is_finite()
            || !edge.damping.is_finite()
        {
            return Err(format!(
                "skeletal secondary-motion edge[{index}] contains invalid scalar data"
            ));
        }
    }
    for (index, bend) in authored.bends.iter().enumerate() {
        if bend
            .indices
            .iter()
            .any(|&particle| particle >= particle_count)
        {
            return Err(format!(
                "skeletal secondary-motion bend[{index}] index outside particles"
            ));
        }
        if bend.weights.iter().any(|value| !value.is_finite())
            || !bend.geometry_scale.is_finite()
            || !bend.rest_scalar.is_finite()
        {
            return Err(format!(
                "skeletal secondary-motion bend[{index}] contains non-finite data"
            ));
        }
    }
    for (index, pair) in authored.centerline_pairs.iter().enumerate() {
        if pair.iter().any(|&particle| particle >= particle_count) {
            return Err(format!(
                "skeletal secondary-motion centerline pair[{index}] index outside particles"
            ));
        }
    }
    for (index, capsule) in authored.collision_capsules.iter().enumerate() {
        if capsule.joint.trim().is_empty()
            || capsule.radius <= 0.0
            || !capsule.radius.is_finite()
            || capsule
                .source_a
                .iter()
                .chain(capsule.source_b.iter())
                .any(|value| !value.is_finite())
        {
            return Err(format!(
                "skeletal secondary-motion capsule[{index}] contains invalid data"
            ));
        }
    }
    for (index, box_shape) in authored.collision_boxes.iter().enumerate() {
        if box_shape.joint.trim().is_empty()
            || box_shape
                .half_extents
                .iter()
                .any(|value| *value <= 0.0 || !value.is_finite())
            || box_shape
                .source_center
                .iter()
                .chain(box_shape.source_axes.iter().flatten())
                .any(|value| !value.is_finite())
        {
            return Err(format!(
                "skeletal secondary-motion oriented_box[{index}] contains invalid data"
            ));
        }
    }
    let tuning_values = [
        authored.tuning.teleport_reset_distance,
        authored.tuning.teleport_reset_quat_dot,
        authored.tuning.back_clearance,
        authored.tuning.max_root_acceleration,
        authored.tuning.gravity_scale,
        authored.tuning.inertia_scale,
        authored.tuning.velocity_damping,
        authored.tuning.collision_margin,
        authored.tuning.follow_scale,
        authored.tuning.stretch_reference_stiffness,
        authored.tuning.bend_reference_stiffness,
        authored.tuning.tunnel_depth,
    ];
    if tuning_values.iter().any(|value| !value.is_finite()) {
        return Err("skeletal secondary-motion tuning contains non-finite data".to_owned());
    }
    if !(0.0..=1.0).contains(&authored.tuning.teleport_reset_quat_dot) {
        return Err(
            "skeletal secondary-motion teleport_reset_quat_dot must be in [0,1]".to_owned(),
        );
    }
    Ok(())
}

fn prepare_skeletal_secondary_motion(
    parts: &[PlayerRuntimeModelPart],
    skeleton: &ModelSkeletonMetadata,
    authored: Option<&PlayerSkeletalSecondaryMotionRig>,
    source_to_model: [f32; 16],
    bind_joint_frames: &[Mat4],
) -> Result<Option<SkeletalSecondaryMotionRuntime>, String> {
    let Some(authored) = authored else {
        return Ok(None);
    };
    validate_skeletal_secondary_motion(authored)?;

    let chain_joints = authored
        .chain_joints
        .iter()
        .enumerate()
        .map(|(lane, name)| {
            secondary_motion_joint(skeleton, name.trim()).ok_or_else(|| {
                format!(
                    "skeletal secondary-motion authored chain is partial lane={lane}: missing joint '{name}'"
                )
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let has_simulated_skin = parts
        .iter()
        .filter_map(|part| part.skin.as_ref())
        .any(|skin| {
            skin.vertices.iter().any(|vertex| {
                vertex
                    .joints
                    .iter()
                    .chain(vertex.joints_extra.iter())
                    .zip(vertex.weights.iter().chain(vertex.weights_extra.iter()))
                    .any(|(&joint, &weight)| {
                        weight > 1.0e-5 && chain_joints.contains(&(joint as usize))
                    })
            })
        });
    if !has_simulated_skin {
        return Ok(None);
    }

    let runtime = SkeletalSecondaryMotionRuntime::new(
        authored,
        chain_joints,
        skeleton,
        source_to_model,
        bind_joint_frames,
    )?;
    newengine_ulog_api::ulog::info!(
        "skeletal secondary motion ready joints={} particles={} edges={} bends={} capsules={} boxes={} source='project-authored definition'",
        authored.chain_joints.len(),
        authored.particles.len(),
        authored.edges.len(),
        authored.bends.len(),
        authored.collision_capsules.len(),
        authored.collision_boxes.len(),
    );
    Ok(Some(runtime))
}

fn project_out_of_secondary_motion_capsule(point: &mut Vec3, a: Vec3, b: Vec3, radius: f32) {
    let axis = b - a;
    let len2 = axis.length_squared();
    if len2 <= 1.0e-8 {
        return;
    }
    let t = ((*point - a).dot(axis) / len2).clamp(0.0, 1.0);
    let closest = a + axis * t;
    let delta = *point - closest;
    let distance = delta.length();
    if distance < radius {
        let normal = if distance > 1.0e-6 {
            delta / distance
        } else {
            Vec3::Z
        };
        *point = closest + normal * radius;
    }
}

fn project_behind_secondary_motion_capsule(
    point: &mut Vec3,
    capsule: SecondaryMotionCapsule,
    back_normal: Vec3,
    margin: f32,
) {
    let axis = capsule.b - capsule.a;
    let len2 = axis.length_squared();
    if len2 <= 1.0e-8 {
        return;
    }
    let t = ((*point - capsule.a).dot(axis) / len2).clamp(0.0, 1.0);
    let closest = capsule.a + axis * t;
    let back = back_normal.normalize_or_zero();
    if back.length_squared() <= 1.0e-8 {
        return;
    }
    let delta = *point - closest;
    let signed = delta.dot(back);
    let tangent = (delta - back * signed).length();
    let radius = capsule.radius + margin;
    if tangent < radius * 1.10 && signed < radius {
        *point += back * (radius - signed);
    }
}

fn project_behind_secondary_motion_box(
    point: &mut Vec3,
    box_shape: SecondaryMotionOrientedBox,
    tunnel_depth: f32,
) {
    let delta = *point - box_shape.center;
    let local = Vec3::new(
        delta.dot(box_shape.axes[0]),
        delta.dot(box_shape.axes[1]),
        delta.dot(box_shape.axes[2]),
    );
    let extents = box_shape.half_extents;
    if local.x.abs() > extents.x || local.y.abs() > extents.y {
        return;
    }
    if local.z < extents.z && local.z > -(extents.z + tunnel_depth.max(0.0)) {
        *point += box_shape.axes[2] * (extents.z - local.z);
    }
}

fn project_out_of_secondary_motion_box(point: &mut Vec3, box_shape: SecondaryMotionOrientedBox) {
    let delta = *point - box_shape.center;
    let local = Vec3::new(
        delta.dot(box_shape.axes[0]),
        delta.dot(box_shape.axes[1]),
        delta.dot(box_shape.axes[2]),
    );
    let extents = box_shape.half_extents;
    if local.x.abs() >= extents.x || local.y.abs() >= extents.y || local.z.abs() >= extents.z {
        return;
    }
    let distances = [
        extents.x - local.x.abs(),
        extents.y - local.y.abs(),
        extents.z - local.z.abs(),
    ];
    let axis = if distances[0] <= distances[1] && distances[0] <= distances[2] {
        0
    } else if distances[1] <= distances[2] {
        1
    } else {
        2
    };
    let component = match axis {
        0 => local.x,
        1 => local.y,
        _ => local.z,
    };
    let sign = if component >= 0.0 { 1.0 } else { -1.0 };
    *point += box_shape.axes[axis] * distances[axis] * sign;
}

fn pin_secondary_motion_particles(
    points: &mut [Vec3],
    guide: &[Vec3],
    authored: &PlayerSkeletalSecondaryMotionRig,
) {
    for (index, particle) in authored.particles.iter().enumerate() {
        if particle.mobility <= 1.0e-8 {
            points[index] = guide[index];
        }
    }
}

fn solve_secondary_motion_edge(
    points: &mut [Vec3],
    authored: &PlayerSkeletalSecondaryMotionRig,
    a: usize,
    b: usize,
    rest: f32,
    authored_stiffness: f32,
) {
    let delta = points[b] - points[a];
    let length = delta.length();
    if length <= 1.0e-6 || !length.is_finite() {
        return;
    }
    let wa = authored.particles[a].mobility.max(0.0);
    let wb = authored.particles[b].mobility.max(0.0);
    let weight_sum = wa + wb;
    if weight_sum <= 1.0e-8 {
        return;
    }
    let stiffness = (authored_stiffness / authored.tuning.stretch_reference_stiffness.max(1.0e-6))
        .clamp(0.0, 1.0);
    let correction = delta * (((length - rest) / length) * stiffness);
    points[a] += correction * (wa / weight_sum);
    points[b] -= correction * (wb / weight_sum);
}

fn damp_secondary_motion_edge_velocity(
    points: &[Vec3],
    previous: &mut [Vec3],
    authored: &PlayerSkeletalSecondaryMotionRig,
    a: usize,
    b: usize,
    authored_damping: f32,
) {
    let axis = (points[b] - points[a]).normalize_or_zero();
    if axis.length_squared() <= 1.0e-8 {
        return;
    }
    let wa = authored.particles[a].mobility.max(0.0);
    let wb = authored.particles[b].mobility.max(0.0);
    let weight_sum = wa + wb;
    if weight_sum <= 1.0e-8 {
        return;
    }
    let mut va = points[a] - previous[a];
    let mut vb = points[b] - previous[b];
    let relative = (vb - va).dot(axis);
    let damping = authored_damping.clamp(0.0, 1.0);
    va += axis * (relative * damping * wa / weight_sum);
    vb -= axis * (relative * damping * wb / weight_sum);
    previous[a] = points[a] - va;
    previous[b] = points[b] - vb;
}

fn solve_secondary_motion_bend(
    points: &mut [Vec3],
    guide: &[Vec3],
    authored: &PlayerSkeletalSecondaryMotionRig,
    indices: [usize; 4],
    weights: [f32; 4],
    geometry_scale: f32,
    rest_scalar: f32,
) {
    let mut current = Vec3::ZERO;
    let mut target = Vec3::ZERO;
    let mut denominator = 0.0f32;
    for lane in 0..4 {
        let index = indices[lane];
        let weight = weights[lane];
        current += points[index] * weight;
        target += guide[index] * weight;
        denominator += authored.particles[index].mobility.max(0.0) * weight * weight;
    }
    if denominator <= 1.0e-8 {
        return;
    }

    let bend_reference = authored.tuning.bend_reference_stiffness;
    let geometry_normalization =
        (bend_reference / geometry_scale.max(bend_reference)).clamp(0.0, 1.0);
    let rest_modulation = (1.0 + rest_scalar.abs() / 0.001).recip();
    let stiffness = (geometry_normalization * rest_modulation).clamp(0.0, 1.0);
    let error = (current - target) * stiffness;
    for lane in 0..4 {
        let index = indices[lane];
        let mobility = authored.particles[index].mobility.max(0.0);
        if mobility <= 1.0e-8 {
            continue;
        }
        points[index] -= error * (mobility * weights[lane] / denominator);
    }
}

fn secondary_motion_centerline(
    points: &[Vec3],
    authored: &PlayerSkeletalSecondaryMotionRig,
) -> Vec<Vec3> {
    authored
        .centerline_pairs
        .iter()
        .map(|pair| (points[pair[0]] + points[pair[1]]) * 0.5)
        .collect()
}

fn normalized_polyline_parameter(points: &[Vec3], index: usize) -> f32 {
    if points.len() <= 1 || index == 0 {
        return 0.0;
    }
    let mut total = 0.0f32;
    let mut prefix = 0.0f32;
    for segment in 0..points.len() - 1 {
        let length = (points[segment + 1] - points[segment]).length();
        total += length;
        if segment < index {
            prefix += length;
        }
    }
    if total <= 1.0e-8 {
        0.0
    } else {
        (prefix / total).clamp(0.0, 1.0)
    }
}

fn sample_polyline_normalized(points: &[Vec3], t: f32) -> Vec3 {
    if points.is_empty() {
        return Vec3::ZERO;
    }
    if points.len() == 1 {
        return points[0];
    }
    let mut total = 0.0f32;
    for segment in 0..points.len() - 1 {
        total += (points[segment + 1] - points[segment]).length();
    }
    if total <= 1.0e-8 {
        return points[0];
    }
    let target = t.clamp(0.0, 1.0) * total;
    let mut cursor = 0.0f32;
    for segment in 0..points.len() - 1 {
        let length = (points[segment + 1] - points[segment]).length();
        if target <= cursor + length || segment + 2 == points.len() {
            let local = if length <= 1.0e-8 {
                0.0
            } else {
                ((target - cursor) / length).clamp(0.0, 1.0)
            };
            return points[segment].lerp(points[segment + 1], local);
        }
        cursor += length;
    }
    *points.last().unwrap_or(&points[0])
}

#[cfg(test)]
mod skeletal_secondary_motion_tests {
    use super::*;

    #[test]
    fn polyline_sampling_is_topology_agnostic() {
        let points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
        ];
        let sampled = sample_polyline_normalized(&points, 0.5);
        assert!((sampled.y - 1.5).abs() < 1.0e-5);
    }

    #[test]
    fn exterior_capsule_projection_respects_authored_radius() {
        let mut point = Vec3::new(0.0, 0.0, 0.1);
        project_out_of_secondary_motion_capsule(
            &mut point,
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            0.5,
        );
        assert!((point.length() - 0.5).abs() < 1.0e-5);
    }
}
