#[derive(Clone, Debug)]
struct SkeletalSecondaryMotionRuntime {
    authored: PlayerSkeletalSecondaryMotionRig,
    attachment_joint: usize,
    chain_joints: Vec<usize>,
    collider_bindings: SecondaryMotionColliderBindings,
    attachment_local_points: Vec<Vec3>,
    bind_chain_frame_inverses: Vec<Mat4>,
    bind_chain_parameters: Vec<f32>,
    particle_guide_scratch: Vec<Vec3>,
    chain_guide_scratch: Vec<Vec3>,
    guide_centerline_scratch: Vec<Vec3>,
    current_centerline_scratch: Vec<Vec3>,
    desired_scratch: Vec<Vec3>,
    collider_scratch: SecondaryMotionColliderSet,
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

        let chain_capacity = chain_joints.len();
        let bind_chain_frame_inverses = bind_chain_frames
            .iter()
            .copied()
            .map(Mat4::inverse)
            .collect::<Vec<_>>();
        if bind_chain_frame_inverses.iter().any(|inverse| {
            inverse
                .to_cols_array()
                .iter()
                .any(|value| !value.is_finite())
        }) {
            return Err("skeletal secondary-motion bind chain contains singular frame".to_owned());
        }
        let bind_chain_parameters = (0..bind_chain_points.len())
            .map(|lane| normalized_polyline_parameter(&bind_chain_points, lane))
            .collect::<Vec<_>>();
        let centerline_capacity = authored.centerline_pairs.len();

        Ok(Self {
            authored: authored.clone(),
            attachment_joint,
            chain_joints,
            collider_bindings,
            attachment_local_points,
            bind_chain_frame_inverses,
            bind_chain_parameters,
            particle_guide_scratch: Vec::with_capacity(bind_particles.len()),
            chain_guide_scratch: Vec::with_capacity(chain_capacity),
            guide_centerline_scratch: Vec::with_capacity(centerline_capacity),
            current_centerline_scratch: Vec::with_capacity(centerline_capacity),
            desired_scratch: Vec::with_capacity(chain_capacity),
            collider_scratch: SecondaryMotionColliderSet::default(),
            points: bind_particles.clone(),
            previous_points: bind_particles,
            previous_root_velocity_local: Vec3::ZERO,
            last_root_position: None,
            last_root_rotation: None,
            reset_pending: true,
            initialized: false,
        })
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
        self.particle_guide_scratch.clear();
        self.particle_guide_scratch.extend(
            self.attachment_local_points
                .iter()
                .copied()
                .map(|point| attachment.transform_point3(point)),
        );
        self.chain_guide_scratch.clear();
        for (lane, joint) in self.chain_joints.iter().copied().enumerate() {
            self.chain_guide_scratch.push(
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
        self.collider_bindings.resolve_from_joint_frames_into(
            joint_frames,
            &mut self.collider_scratch,
        )?;
        let particle_guide = &self.particle_guide_scratch;
        let chain_guide = &self.chain_guide_scratch;
        let colliders = &self.collider_scratch;
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
            self.points
                .clone_from_slice(&self.particle_guide_scratch);
            self.previous_points
                .clone_from_slice(&self.particle_guide_scratch);
            self.previous_root_velocity_local = root_velocity_local;
            self.reset_pending = false;
            self.initialized = true;
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
                        particle_guide,
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
                            particle_guide,
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
                        particle_guide,
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
                pin_secondary_motion_particles(&mut self.points, particle_guide, &self.authored);
            }
        }

        secondary_motion_centerline_into(
            particle_guide,
            &self.authored,
            &mut self.guide_centerline_scratch,
        );
        secondary_motion_centerline_into(
            &self.points,
            &self.authored,
            &mut self.current_centerline_scratch,
        );
        self.desired_scratch.clear();
        self.desired_scratch.extend_from_slice(chain_guide);
        let guide_centerline = &self.guide_centerline_scratch;
        let current_centerline = &self.current_centerline_scratch;
        let desired = &mut self.desired_scratch;
        for (lane, desired_point) in desired
            .iter_mut()
            .enumerate()
            .take(self.chain_joints.len())
            .skip(self.authored.dynamic_start)
        {
            let t = self.bind_chain_parameters[lane];
            *desired_point += sample_polyline_normalized(current_centerline, t)
                - sample_polyline_normalized(guide_centerline, t);
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
            let deformation = desired_frame * self.bind_chain_frame_inverses[lane];
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

