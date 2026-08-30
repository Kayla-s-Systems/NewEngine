// Native Abby braid secondary motion.
// Authored cloth and foreground collision data originate in TLOU2 PC source space;
// every position/axis is canonicalized through the YDD skin_source_to_model matrix.
include!("abby_braid_cloth_authored.rs");

const ABBY_BRAID_NATIVE_JOINT_COUNT: usize = 8;
const ABBY_BRAID_DYNAMIC_START: usize = 1;
const ABBY_BRAID_TELEPORT_RESET_DISTANCE: f32 = 0.85;
const ABBY_BRAID_TELEPORT_RESET_QUAT_DOT: f32 = 0.65;
const ABBY_BRAID_BACK_CLEARANCE: f32 = 0.010;

#[derive(Clone, Copy, Debug)]
struct AbbyBraidCollisionRig {
    attachment_joint: usize,
    head_joint: usize,
    head_base_joint: usize,
    upper_back_joint: usize,
    middle_back_joint: usize,
    lower_back_joint: usize,
    left_shoulder_joint: usize,
    right_shoulder_joint: usize,
}

#[derive(Clone, Copy, Debug)]
struct AbbyBraidCapsuleBinding {
    joint: usize,
    local_a: Vec3,
    local_b: Vec3,
    radius: f32,
}

#[derive(Clone, Copy, Debug)]
struct AbbyBraidOrientedBoxBinding {
    joint: usize,
    local_center: Vec3,
    local_axes: [Vec3; 3],
    half_extents: Vec3,
}

#[derive(Clone, Copy, Debug)]
struct AbbyBraidColliderBindings {
    capsules: [AbbyBraidCapsuleBinding; 8],
    boxes: [AbbyBraidOrientedBoxBinding; 1],
}

#[derive(Clone, Copy, Debug)]
struct AbbyBraidCapsule {
    a: Vec3,
    b: Vec3,
    radius: f32,
}

#[derive(Clone, Copy, Debug)]
struct AbbyBraidOrientedBox {
    center: Vec3,
    axes: [Vec3; 3],
    half_extents: Vec3,
}

#[derive(Clone, Copy, Debug)]
struct AbbyBraidColliderSet {
    capsules: [AbbyBraidCapsule; 8],
    boxes: [AbbyBraidOrientedBox; 1],
}

fn abby_braid_bind_inverse(
    joint: usize,
    label: &str,
    bind_joint_frames: &[Mat4],
) -> Result<Mat4, String> {
    let bind = bind_joint_frames.get(joint).copied().ok_or_else(|| {
        format!(
            "Abby braid collision bind joint outside frame table label={label} joint={joint} frames={}",
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
            "Abby braid collision bind frame is singular/non-finite label={label} joint={joint}"
        ));
    }
    Ok(inverse)
}

impl AbbyBraidColliderBindings {
    fn from_bind_frames(
        rig: AbbyBraidCollisionRig,
        bind_joint_frames: &[Mat4],
        source_to_model: Mat4,
    ) -> Result<Self, String> {
        let capsule = |joint: usize,
                       label: &str,
                       authored_a: Vec3,
                       authored_b: Vec3,
                       radius: f32|
         -> Result<AbbyBraidCapsuleBinding, String> {
            let inverse = abby_braid_bind_inverse(joint, label, bind_joint_frames)?;
            let authored_a = source_to_model.transform_point3(authored_a);
            let authored_b = source_to_model.transform_point3(authored_b);
            Ok(AbbyBraidCapsuleBinding {
                joint,
                local_a: inverse.transform_point3(authored_a),
                local_b: inverse.transform_point3(authored_b),
                radius,
            })
        };
        let oriented_box = |joint: usize,
                            label: &str,
                            authored_center: Vec3,
                            authored_axes: [Vec3; 3],
                            half_extents: Vec3|
         -> Result<AbbyBraidOrientedBoxBinding, String> {
            let inverse = abby_braid_bind_inverse(joint, label, bind_joint_frames)?;
            let authored_center = source_to_model.transform_point3(authored_center);
            let mut local_axes = [Vec3::ZERO; 3];
            for (index, axis) in authored_axes.into_iter().enumerate() {
                let axis = source_to_model.transform_vector3(axis).normalize_or_zero();
                let local = inverse.transform_vector3(axis).normalize_or_zero();
                if local.length_squared() <= 1.0e-8 {
                    return Err(format!(
                        "Abby braid collision OBB axis collapsed label={label} joint={joint} axis={index}"
                    ));
                }
                local_axes[index] = local;
            }
            Ok(AbbyBraidOrientedBoxBinding {
                joint,
                local_center: inverse.transform_point3(authored_center),
                local_axes,
                half_extents,
            })
        };

        // Authored bind-space geometry decoded from Abby's COLLISION_DATA_FOREGROUND
        // Havok 2017 TAG0. The bind remap is intentionally computed against the imported
        // Abby_rig rather than assuming its joints are byte-identical to abby-skel.pak:
        // imported_bind^-1 * authored_shape_bind, then animated_joint * local_shape.
        Ok(Self {
            capsules: [
                capsule(
                    rig.lower_back_joint,
                    "spineb",
                    Vec3::new(-0.047_532_082, 1.068_920_494, 0.025_299_225),
                    Vec3::new(0.047_532_082, 1.068_920_494, 0.025_299_225),
                    0.138_568_237,
                )?,
                capsule(
                    rig.middle_back_joint,
                    "spinec",
                    Vec3::new(-0.038_301_840, 1.213_728_309, 0.023_629_347),
                    Vec3::new(0.038_301_840, 1.213_728_309, 0.023_629_347),
                    0.142_756_164,
                )?,
                capsule(
                    rig.upper_back_joint,
                    "spined",
                    Vec3::new(-0.095_017_865, 1.361_141_443, -0.009_727_530),
                    Vec3::new(0.095_017_865, 1.361_141_443, -0.009_727_530),
                    0.113_832_325,
                )?,
                capsule(
                    rig.head_base_joint,
                    "heada",
                    Vec3::new(-0.000_000_007, 1.569_606_982, 0.019_181_884),
                    Vec3::new(-0.000_000_007, 1.470_282_592, 0.007_612_315),
                    0.064_351_842,
                )?,
                capsule(
                    rig.head_joint,
                    "headb-back",
                    Vec3::new(0.000_502_951, 1.568_786_517, 0.067_407_488),
                    Vec3::new(0.000_502_951, 1.668_742_761, 0.064_498_807),
                    0.069_300_607,
                )?,
                capsule(
                    rig.head_joint,
                    "headb-front",
                    Vec3::new(0.000_999_995, 1.658_753_717, 0.013_592_642),
                    Vec3::new(0.000_999_995, 1.574_617_299, 0.045_504_414),
                    0.077_224_247,
                )?,
                capsule(
                    rig.left_shoulder_joint,
                    "l_shoulder",
                    Vec3::new(0.170_042_539, 1.385_139_470, 0.000_210_913),
                    Vec3::new(0.315_735_316, 1.169_874_187, -0.007_461_836),
                    0.064_326_786,
                )?,
                capsule(
                    rig.right_shoulder_joint,
                    "r_shoulder",
                    Vec3::new(-0.310_326_162, 1.167_700_125, -0.002_860_395),
                    Vec3::new(-0.173_673_718, 1.390_299_964, -0.004_678_467),
                    0.063_753_903,
                )?,
            ],
            boxes: [oriented_box(
                rig.upper_back_joint,
                "spined-box",
                Vec3::new(0.0, 1.192_253_582, 0.052_072_950),
                [
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(0.0, -0.984_808_579, 0.173_644_132),
                    Vec3::new(0.0, -0.173_644_132, -0.984_808_579),
                ],
                Vec3::new(0.079_999_998, 0.275_014_699, 0.065_004_334),
            )?],
        })
    }

    fn from_joint_frames(&self, joint_frames: &[Mat4]) -> Result<AbbyBraidColliderSet, String> {
        let animated_frame = |joint: usize, label: &str| {
            joint_frames.get(joint).copied().ok_or_else(|| {
                format!(
                    "Abby braid collision-rig joint outside animated frame table label={label} joint={joint} frames={}",
                    joint_frames.len()
                )
            })
        };
        let mut capsules = [AbbyBraidCapsule {
            a: Vec3::ZERO,
            b: Vec3::ZERO,
            radius: 0.0,
        }; 8];
        for (index, binding) in self.capsules.iter().copied().enumerate() {
            let frame = animated_frame(binding.joint, "capsule")?;
            capsules[index] = AbbyBraidCapsule {
                a: frame.transform_point3(binding.local_a),
                b: frame.transform_point3(binding.local_b),
                radius: binding.radius,
            };
        }
        let mut boxes = [AbbyBraidOrientedBox {
            center: Vec3::ZERO,
            axes: [Vec3::ZERO; 3],
            half_extents: Vec3::ZERO,
        }; 1];
        for (index, binding) in self.boxes.iter().copied().enumerate() {
            let frame = animated_frame(binding.joint, "oriented-box")?;
            let mut axes = [Vec3::ZERO; 3];
            for (axis_index, local_axis) in binding.local_axes.into_iter().enumerate() {
                let axis = frame.transform_vector3(local_axis).normalize_or_zero();
                if axis.length_squared() <= 1.0e-8 {
                    return Err(format!(
                        "Abby braid animated OBB axis collapsed joint={} axis={axis_index}",
                        binding.joint
                    ));
                }
                axes[axis_index] = axis;
            }
            boxes[index] = AbbyBraidOrientedBox {
                center: frame.transform_point3(binding.local_center),
                axes,
                half_extents: binding.half_extents,
            };
        }
        Ok(AbbyBraidColliderSet { capsules, boxes })
    }
}

#[derive(Clone, Debug)]
struct AbbyBraidRuntime {
    rig: AbbyBraidCollisionRig,
    braid_joints: [usize; ABBY_BRAID_NATIVE_JOINT_COUNT],
    collider_bindings: AbbyBraidColliderBindings,
    cloth_attachment_local_points: [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
    bind_braid_points: [Vec3; ABBY_BRAID_NATIVE_JOINT_COUNT],
    bind_braid_frames: [Mat4; ABBY_BRAID_NATIVE_JOINT_COUNT],
    points: [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
    previous_points: [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
    previous_root_velocity_local: Vec3,
    last_root_position: Option<Vec3>,
    last_root_rotation: Option<Quat>,
    reset_pending: bool,
    initialized: bool,
}

impl AbbyBraidRuntime {
    fn new(
        rig: AbbyBraidCollisionRig,
        braid_joints: [usize; ABBY_BRAID_NATIVE_JOINT_COUNT],
        source_to_model: [f32; 16],
        bind_joint_frames: &[Mat4],
    ) -> Result<Self, String> {
        let source_to_model = Mat4::from_cols_array(&source_to_model);
        let attachment_bind = *bind_joint_frames.get(rig.attachment_joint).ok_or_else(|| {
            format!(
                "braid attachment bind joint outside frame table joint={} frames={}",
                rig.attachment_joint,
                bind_joint_frames.len()
            )
        })?;
        let attachment_bind_inverse = attachment_bind.inverse();
        if attachment_bind_inverse
            .to_cols_array()
            .iter()
            .any(|v| !v.is_finite())
        {
            return Err("braid attachment bind frame is singular/non-finite".to_owned());
        }
        let collider_bindings =
            AbbyBraidColliderBindings::from_bind_frames(rig, bind_joint_frames, source_to_model)?;
        let cloth_bind = ABBY_BRAID_CLOTH_BIND_PARTICLES
            .map(|p| source_to_model.transform_point3(Vec3::new(p[0], p[1], p[2])));
        let cloth_attachment_local_points =
            cloth_bind.map(|point| attachment_bind_inverse.transform_point3(point));
        let mut bind_braid_points = [Vec3::ZERO; ABBY_BRAID_NATIVE_JOINT_COUNT];
        let mut bind_braid_frames = [Mat4::IDENTITY; ABBY_BRAID_NATIVE_JOINT_COUNT];
        for (lane, joint) in braid_joints.iter().copied().enumerate() {
            let frame = *bind_joint_frames.get(joint).ok_or_else(|| {
                format!("native braid joint outside bind frame table lane={lane} joint={joint}")
            })?;
            bind_braid_frames[lane] = frame;
            bind_braid_points[lane] = frame.transform_point3(Vec3::ZERO);
        }
        Ok(Self {
            rig,
            braid_joints,
            collider_bindings,
            cloth_attachment_local_points,
            bind_braid_points,
            bind_braid_frames,
            points: cloth_bind,
            previous_points: cloth_bind,
            previous_root_velocity_local: Vec3::ZERO,
            last_root_position: None,
            last_root_rotation: None,
            reset_pending: true,
            initialized: false,
        })
    }

    fn reset(&mut self, guide: [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT], root_velocity_local: Vec3) {
        self.points = guide;
        self.previous_points = guide;
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
            .get(self.rig.attachment_joint)
            .ok_or_else(|| "native braid attachment frame missing".to_owned())?;
        let cloth_guide = self
            .cloth_attachment_local_points
            .map(|point| attachment.transform_point3(point));
        let mut native_guide = [Vec3::ZERO; ABBY_BRAID_NATIVE_JOINT_COUNT];
        for (lane, joint) in self.braid_joints.iter().copied().enumerate() {
            native_guide[lane] = joint_frames
                .get(joint)
                .copied()
                .ok_or_else(|| {
                    format!("native braid animated frame missing lane={lane} joint={joint}")
                })?
                .transform_point3(Vec3::ZERO);
        }
        let colliders = self.collider_bindings.from_joint_frames(joint_frames)?;
        let root_rotation = root_rotation.normalize_or_identity();
        if self
            .last_root_position
            .is_some_and(|p| (root_position - p).length() > ABBY_BRAID_TELEPORT_RESET_DISTANCE)
        {
            self.reset_pending = true;
        }
        if self.last_root_rotation.is_some_and(|q| {
            q.normalize_or_identity().dot(root_rotation).abs() < ABBY_BRAID_TELEPORT_RESET_QUAT_DOT
        }) {
            self.reset_pending = true;
        }
        self.last_root_position = Some(root_position);
        self.last_root_rotation = Some(root_rotation);

        if !self.initialized || self.reset_pending {
            self.reset(cloth_guide, root_velocity_local);
        } else if dt > 0.0 {
            let frame_dt = dt.clamp(1.0 / 240.0, 1.0 / 20.0);
            const SUBSTEPS: usize = 2;
            const ITERATIONS: usize = 5;
            let step_dt = frame_dt / SUBSTEPS as f32;
            let mut root_acceleration_local =
                (root_velocity_local - self.previous_root_velocity_local) / frame_dt.max(1.0e-5);
            let acceleration_len = root_acceleration_local.length();
            if acceleration_len > 22.0 {
                root_acceleration_local *= 22.0 / acceleration_len;
            }
            self.previous_root_velocity_local = root_velocity_local;
            let gravity = Vec3::new(
                0.0,
                9.81 * ABBY_BRAID_CLOTH_RAW_PARAMS[7] * step_dt * step_dt,
                0.0,
            );
            let inertial_base =
                -root_acceleration_local * (ABBY_BRAID_CLOTH_RAW_PARAMS[4] * step_dt * step_dt);
            let velocity_retention = (1.0 - ABBY_BRAID_CLOTH_RAW_PARAMS[14]).clamp(0.0, 1.0);
            let collision_margin =
                ABBY_BRAID_CLOTH_RAW_PARAMS[6].max(0.0) + ABBY_BRAID_BACK_CLEARANCE;
            for _ in 0..SUBSTEPS {
                for index in 0..ABBY_BRAID_CLOTH_PARTICLE_COUNT {
                    let mobility = ABBY_BRAID_CLOTH_SCALAR0[index].max(0.0);
                    if mobility <= 1.0e-8 {
                        self.points[index] = cloth_guide[index];
                        self.previous_points[index] = cloth_guide[index];
                        continue;
                    }
                    let current = self.points[index];
                    let velocity = (current - self.previous_points[index]) * velocity_retention;
                    self.previous_points[index] = current;
                    let inertia_weight = (ABBY_BRAID_CLOTH_SCALAR2[index]
                        / ABBY_BRAID_CLOTH_RAW_PARAMS[4].max(1.0e-6))
                    .clamp(0.0, 1.0);
                    self.points[index] =
                        current + velocity + gravity + inertial_base * inertia_weight;
                }
                for _ in 0..ITERATIONS {
                    pin_authored_cloth_particles(&mut self.points, &cloth_guide);
                    for &(a, b, rest, stiffness, _damping) in &ABBY_BRAID_CLOTH_EDGES {
                        solve_authored_cloth_edge(&mut self.points, a, b, rest, stiffness);
                    }
                    for &(indices, weights, geometry_scale, rest_scalar) in &ABBY_BRAID_CLOTH_BENDS
                    {
                        solve_authored_cloth_bend(
                            &mut self.points,
                            &cloth_guide,
                            indices,
                            weights,
                            geometry_scale,
                            rest_scalar,
                        );
                    }
                    for index in 0..ABBY_BRAID_CLOTH_PARTICLE_COUNT {
                        if ABBY_BRAID_CLOTH_SCALAR0[index] <= 1.0e-8 {
                            continue;
                        }
                        let follow = (ABBY_BRAID_CLOTH_SCALAR1[index]
                            * ABBY_BRAID_CLOTH_RAW_PARAMS[2])
                            .clamp(0.0, 1.0);
                        self.points[index] = self.points[index].lerp(cloth_guide[index], follow);
                        // Lower/middle/upper torso capsules are one-sided for the braid: the
                        // strand may leave the back, but never tunnel through to the chest side.
                        let back_normal = colliders.boxes[0].axes[2];
                        for (capsule_index, capsule) in
                            colliders.capsules.iter().copied().enumerate()
                        {
                            if capsule_index <= 2 {
                                project_behind_capsule(
                                    &mut self.points[index],
                                    capsule,
                                    back_normal,
                                    collision_margin,
                                );
                            } else {
                                project_out_of_capsule(
                                    &mut self.points[index],
                                    capsule.a,
                                    capsule.b,
                                    capsule.radius + collision_margin,
                                );
                            }
                        }
                        let mut torso = colliders.boxes[0];
                        torso.half_extents += Vec3::splat(collision_margin);
                        project_behind_oriented_box(&mut self.points[index], torso);
                    }
                    for &(a, b, rest, stiffness, _damping) in &ABBY_BRAID_CLOTH_EDGES {
                        solve_authored_cloth_edge(&mut self.points, a, b, rest, stiffness);
                    }
                    pin_authored_cloth_particles(&mut self.points, &cloth_guide);
                }
                for &(a, b, _rest, _stiffness, damping) in &ABBY_BRAID_CLOTH_EDGES {
                    damp_authored_cloth_edge_velocity(
                        &self.points,
                        &mut self.previous_points,
                        a,
                        b,
                        damping,
                    );
                }
                pin_authored_cloth_particles(&mut self.points, &cloth_guide);
            }
        }

        let guide_centerline = authored_cloth_centerline(&cloth_guide);
        let current_centerline = authored_cloth_centerline(&self.points);
        let mut desired = native_guide;
        for lane in ABBY_BRAID_DYNAMIC_START..ABBY_BRAID_NATIVE_JOINT_COUNT {
            let t = normalized_polyline_parameter(&self.bind_braid_points, lane);
            desired[lane] += sample_polyline_normalized(&current_centerline, t)
                - sample_polyline_normalized(&guide_centerline, t);
        }
        for lane in ABBY_BRAID_DYNAMIC_START..ABBY_BRAID_NATIVE_JOINT_COUNT {
            let joint = self.braid_joints[lane];
            let guide_direction = if lane + 1 < ABBY_BRAID_NATIVE_JOINT_COUNT {
                native_guide[lane + 1] - native_guide[lane]
            } else {
                native_guide[lane] - native_guide[lane - 1]
            }
            .normalize_or_zero();
            let current_direction = if lane + 1 < ABBY_BRAID_NATIVE_JOINT_COUNT {
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
            let base_frame = joint_frames[joint];
            let desired_frame = Mat4::from_translation(desired[lane])
                * Mat4::from_quat(bend)
                * Mat4::from_translation(-native_guide[lane])
                * base_frame;
            let bind_inverse = self.bind_braid_frames[lane].inverse();
            let deformation = desired_frame * bind_inverse;
            if deformation.to_cols_array().iter().any(|v| !v.is_finite()) {
                return Err(format!(
                    "native braid deformation became non-finite lane={lane} joint={joint}"
                ));
            }
            palette[joint] = deformation;
        }
        Ok(())
    }
}

fn resolve_native_braid_joint(skeleton: &ModelSkeletonMetadata, name: &str) -> Option<usize> {
    skeleton.joints.iter().position(|joint| joint.name == name)
}

fn prepare_native_braid_secondary_motion(
    parts: &[PlayerRuntimeModelPart],
    skeleton: &ModelSkeletonMetadata,
    authored: Option<&newengine_engine_runtime::gameplay::PlayerBraidSecondaryMotionRig>,
    source_to_model: [f32; 16],
    bind_joint_frames: &[Mat4],
) -> Result<Option<AbbyBraidRuntime>, String> {
    let Some(authored) = authored else {
        return Ok(None);
    };
    if authored.chain_joints.len() != ABBY_BRAID_NATIVE_JOINT_COUNT {
        return Err(format!(
            "native braid authored chain requires {} joints, got {}",
            ABBY_BRAID_NATIVE_JOINT_COUNT,
            authored.chain_joints.len()
        ));
    }

    let mut braid_joints = [0usize; ABBY_BRAID_NATIVE_JOINT_COUNT];
    for (lane, name) in authored.chain_joints.iter().enumerate() {
        braid_joints[lane] = resolve_native_braid_joint(skeleton, name).ok_or_else(|| {
            format!("native braid authored chain is partial: missing joint '{name}'")
        })?;
    }

    let has_braid_skin = parts
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
                        weight > 1.0e-5 && braid_joints.contains(&(joint as usize))
                    })
            })
        });
    if !has_braid_skin {
        return Ok(None);
    }

    let required = |name: &str| {
        resolve_native_braid_joint(skeleton, name)
            .ok_or_else(|| format!("native braid authored collision driver '{name}' is missing"))
    };
    let rig = AbbyBraidCollisionRig {
        attachment_joint: braid_joints[0],
        head_joint: required(&authored.head_joint)?,
        head_base_joint: required(&authored.head_base_joint)?,
        upper_back_joint: required(&authored.upper_back_joint)?,
        middle_back_joint: required(&authored.middle_back_joint)?,
        lower_back_joint: required(&authored.lower_back_joint)?,
        left_shoulder_joint: required(&authored.left_shoulder_joint)?,
        right_shoulder_joint: required(&authored.right_shoulder_joint)?,
    };
    let runtime = AbbyBraidRuntime::new(rig, braid_joints, source_to_model, bind_joint_frames)?;
    newengine_ulog_api::ulog::info!(
        "game-ready: native braid secondary motion ready joints={} particles=32 collision='authored rig drivers + authored source-space colliders' space='source -> skin_source_to_model -> animated model'",
        ABBY_BRAID_NATIVE_JOINT_COUNT
    );
    Ok(Some(runtime))
}

fn project_out_of_capsule(point: &mut Vec3, a: Vec3, b: Vec3, radius: f32) {
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

fn project_behind_capsule(
    point: &mut Vec3,
    capsule: AbbyBraidCapsule,
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

fn project_behind_oriented_box(point: &mut Vec3, oriented_box: AbbyBraidOrientedBox) {
    let delta = *point - oriented_box.center;
    let local = Vec3::new(
        delta.dot(oriented_box.axes[0]),
        delta.dot(oriented_box.axes[1]),
        delta.dot(oriented_box.axes[2]),
    );
    let e = oriented_box.half_extents;
    if local.x.abs() > e.x || local.y.abs() > e.y {
        return;
    }
    // axes[2] is the authored back direction. Keep braid on/back of this face and catch
    // a complete one-frame tunnel through the torso up to 20 cm beyond the front face.
    if local.z < e.z && local.z > -(e.z + 0.20) {
        *point += oriented_box.axes[2] * (e.z - local.z);
    }
}

fn pin_authored_cloth_particles(
    points: &mut [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
    guide: &[Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
) {
    for index in 0..ABBY_BRAID_CLOTH_PARTICLE_COUNT {
        if ABBY_BRAID_CLOTH_SCALAR0[index] <= 1.0e-8 {
            points[index] = guide[index];
        }
    }
}

fn solve_authored_cloth_edge(
    points: &mut [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
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
    let wa = ABBY_BRAID_CLOTH_SCALAR0[a].max(0.0);
    let wb = ABBY_BRAID_CLOTH_SCALAR0[b].max(0.0);
    let weight_sum = wa + wb;
    if weight_sum <= 1.0e-8 {
        return;
    }
    let stiffness =
        (authored_stiffness / ABBY_BRAID_CLOTH_RAW_PARAMS[9].max(1.0e-6)).clamp(0.0, 1.0);
    let correction = delta * (((length - rest) / length) * stiffness);
    points[a] += correction * (wa / weight_sum);
    points[b] -= correction * (wb / weight_sum);
}

fn damp_authored_cloth_edge_velocity(
    points: &[Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
    previous: &mut [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
    a: usize,
    b: usize,
    authored_damping: f32,
) {
    let axis = (points[b] - points[a]).normalize_or_zero();
    if axis.length_squared() <= 1.0e-8 {
        return;
    }
    let wa = ABBY_BRAID_CLOTH_SCALAR0[a].max(0.0);
    let wb = ABBY_BRAID_CLOTH_SCALAR0[b].max(0.0);
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

fn solve_authored_cloth_bend(
    points: &mut [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
    guide: &[Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
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
        denominator += ABBY_BRAID_CLOTH_SCALAR0[index].max(0.0) * weight * weight;
    }
    if denominator <= 1.0e-8 {
        return;
    }

    // Source records also carry a geometry scale and signed rest scalar. The imported glTF
    // deform chain is not the original ND cloth render path, so use those terms only as an
    // authored strength normalization while the exact source rest state comes from the same
    // four weighted bind particles. This keeps bind pose invariant and avoids inventing a
    // synthetic bend cone.
    let bend_material = ABBY_BRAID_CLOTH_RAW_PARAMS[13];
    let geometry_normalization =
        (bend_material / geometry_scale.max(bend_material)).clamp(0.0, 1.0);
    let rest_modulation = (1.0 + rest_scalar.abs() / 0.001).recip();
    let stiffness = (geometry_normalization * rest_modulation).clamp(0.0, 1.0);
    let error = (current - target) * stiffness;
    for lane in 0..4 {
        let index = indices[lane];
        let mobility = ABBY_BRAID_CLOTH_SCALAR0[index].max(0.0);
        if mobility <= 1.0e-8 {
            continue;
        }
        points[index] -= error * (mobility * weights[lane] / denominator);
    }
}

fn authored_cloth_centerline(
    points: &[Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
) -> [Vec3; ABBY_BRAID_CLOTH_ROW_COUNT] {
    ABBY_BRAID_CLOTH_CENTERLINE_PAIRS.map(|pair| (points[pair[0]] + points[pair[1]]) * 0.5)
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
    let mut lengths = [0.0f32; ABBY_BRAID_CLOTH_ROW_COUNT - 1];
    let mut total = 0.0f32;
    for index in 0..points.len() - 1 {
        let length = (points[index + 1] - points[index]).length();
        if index < lengths.len() {
            lengths[index] = length;
        }
        total += length;
    }
    if total <= 1.0e-8 {
        return points[0];
    }
    let target = t.clamp(0.0, 1.0) * total;
    let mut cursor = 0.0f32;
    for index in 0..points.len() - 1 {
        let length = lengths[index];
        if target <= cursor + length || index + 2 == points.len() {
            let local = if length <= 1.0e-8 {
                0.0
            } else {
                ((target - cursor) / length).clamp(0.0, 1.0)
            };
            return points[index].lerp(points[index + 1], local);
        }
        cursor += length;
    }
    *points.last().unwrap_or(&points[0])
}

#[cfg(test)]
mod braid_tests {
    use super::*;
    #[test]
    fn authored_cloth_topology_matches_source_contract() {
        assert_eq!(ABBY_BRAID_CLOTH_BIND_PARTICLES.len(), 32);
        assert_eq!(ABBY_BRAID_CLOTH_TRIANGLES.len(), 30);
        assert_eq!(ABBY_BRAID_CLOTH_EDGES.len(), 61);
        assert_eq!(ABBY_BRAID_CLOTH_BENDS.len(), 29);
        assert_eq!(ABBY_BRAID_CLOTH_ACTIVE_VERTEX_ORDER.len(), 32);
        assert_eq!(ABBY_BRAID_CLOTH_CENTERLINE_PAIRS.len(), 16);
    }

    #[test]
    fn source_to_model_rotation_places_authored_braid_on_canonical_back() {
        let m = Mat4::from_cols_array(&[
            -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]);
        let p = m.transform_point3(Vec3::new(0.02, 1.4, -0.11));
        assert!(p.z > 0.10);
    }
    #[test]
    fn one_sided_capsule_guard_cannot_exit_through_chest_side() {
        let capsule = AbbyBraidCapsule {
            a: Vec3::new(-0.1, 0.0, 0.0),
            b: Vec3::new(0.1, 0.0, 0.0),
            radius: 0.12,
        };
        let mut point = Vec3::new(0.0, 0.0, -0.15);
        project_behind_capsule(&mut point, capsule, Vec3::Z, 0.01);
        assert!(point.z >= 0.129);
    }
    #[test]
    fn torso_back_guard_catches_complete_tunnel() {
        let box_shape = AbbyBraidOrientedBox {
            center: Vec3::ZERO,
            axes: [Vec3::X, Vec3::Y, Vec3::Z],
            half_extents: Vec3::new(0.2, 0.3, 0.1),
        };
        let mut point = Vec3::new(0.0, 0.0, -0.18);
        project_behind_oriented_box(&mut point, box_shape);
        assert!((point.z - 0.1).abs() < 1.0e-6);
    }
}
