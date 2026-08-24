use super::*;

use newengine_animation_runtime::{
    build_model_joint_frames_from_local_pose, build_skin_palette_from_local_pose, decode_ycd_body,
    AnimationClip, JointLocalPose,
};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient, ASSET_LIST_FILE_BODY_OUTPUT};
use newengine_math::{Mat4, Quat, Vec3};
use newengine_model_skeleton_api::ModelSkeletonMetadata;

include!("abby_braid_cloth_authored.rs");

#[derive(Clone, Debug)]
struct PlayerAnimationRuntimeClip {
    clip_ref: String,
    clip: AnimationClip,
}

const ABBY_BRAID_SOFT_BODY_JOINTS: usize = 18;
const ABBY_BRAID_OUTPUT_KINEMATIC_JOINTS: usize = 2;
const ABBY_NATIVE_BRAID_JOINT_NAMES: [&str; 8] = [
    "braid_offset",
    "braid_a",
    "braid_b",
    "braid_c",
    "braid_d",
    "braid_e",
    "braid_f",
    "braid_g",
];
// The authored cloth bridge has 18 output points while the native North Star braid
// skin uses braid_offset + braid_a..g. These indices preserve the original chain arc.
const ABBY_NATIVE_BRAID_OUTPUT_INDICES: [usize; 8] = [0, 1, 4, 7, 10, 13, 16, 17];
const ABBY_BRAID_TELEPORT_RESET_DISTANCE: f32 = 0.85;
const ABBY_BRAID_TELEPORT_RESET_QUAT_DOT: f32 = 0.65;
const ABBY_BRAID_BIND_POINTS: [[f32; 3]; ABBY_BRAID_SOFT_BODY_JOINTS] = [
    [-0.002999943, 1.656426733, -0.084000171],
    [-0.002999943, 1.641275483, -0.084000322],
    [-0.002999942, 1.610275472, -0.084000316],
    [-0.002999941, 1.582275481, -0.084000312],
    [-0.002999940, 1.556275584, -0.084000290],
    [-0.002999939, 1.529539563, -0.087057094],
    [-0.002999938, 1.505803813, -0.090585516],
    [-0.002999937, 1.478275443, -0.094000325],
    [-0.002999935, 1.451275505, -0.100000274],
    [-0.002999934, 1.424487763, -0.104257053],
    [-0.002999932, 1.398487795, -0.105513821],
    [-0.002999932, 1.373558875, -0.107884712],
    [-0.002999931, 1.348712936, -0.111831540],
    [-0.002999930, 1.326275459, -0.114000268],
    [-0.002999929, 1.303275283, -0.116000267],
    [-0.002999928, 1.280275313, -0.118000280],
    [-0.002999927, 1.259275336, -0.120000261],
    [-0.002999926, 1.235222121, -0.121601911],
];

/// Mapping from the canonical Abby deformation rig into the imported `Abby_rig`.
///
/// Binary evidence from `abby-skel.pak` is authoritative here:
/// `braid_offset.parent = headb`; `neck`, `spined`, `l_shoulder` and `r_shoulder`
/// are the body-side collision drivers. Bind-space comparison maps `headb` to the
/// imported `head`, `spined` to `DEF-spine.003`, and the original shoulder sockets
/// to `DEF-upper_arm.L/R` much more closely than to the medial clavicle controls.
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
    ) -> Result<Self, String> {
        let capsule = |joint: usize,
                       label: &str,
                       authored_a: Vec3,
                       authored_b: Vec3,
                       radius: f32|
         -> Result<AbbyBraidCapsuleBinding, String> {
            let inverse = abby_braid_bind_inverse(joint, label, bind_joint_frames)?;
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
            let mut local_axes = [Vec3::ZERO; 3];
            for (index, axis) in authored_axes.into_iter().enumerate() {
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
enum AbbyBraidPaletteTarget {
    Supplemental18,
    Native8 {
        joints: [usize; 8],
        bind_points: [Vec3; 8],
    },
}

#[derive(Clone, Debug)]
struct AbbyBraidSoftBodyRuntime {
    rig: AbbyBraidCollisionRig,
    collider_bindings: AbbyBraidColliderBindings,
    palette_target: AbbyBraidPaletteTarget,
    attachment_bind_inverse: Mat4,
    attachment_local_points: [Vec3; ABBY_BRAID_SOFT_BODY_JOINTS],
    cloth_attachment_local_points: [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
    points: [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
    previous_points: [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
    previous_root_velocity_local: Vec3,
    last_root_position: Option<Vec3>,
    last_root_rotation: Option<Quat>,
    reset_pending: bool,
    initialized: bool,
}

impl AbbyBraidSoftBodyRuntime {
    fn new(
        rig: AbbyBraidCollisionRig,
        bind_joint_frames: &[Mat4],
        palette_target: AbbyBraidPaletteTarget,
    ) -> Result<Self, String> {
        let attachment_bind = *bind_joint_frames.get(rig.attachment_joint).ok_or_else(|| {
            format!(
                "Abby braid attachment bind joint outside frame table joint={} frames={}",
                rig.attachment_joint,
                bind_joint_frames.len()
            )
        })?;
        let attachment_bind_inverse = attachment_bind.inverse();
        if attachment_bind_inverse
            .to_cols_array()
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err("Abby braid attachment bind frame is singular/non-finite".to_owned());
        }
        let palette_target = match palette_target {
            AbbyBraidPaletteTarget::Supplemental18 => AbbyBraidPaletteTarget::Supplemental18,
            AbbyBraidPaletteTarget::Native8 { joints, .. } => {
                let mut bind_points = [Vec3::ZERO; 8];
                for (index, joint) in joints.iter().copied().enumerate() {
                    let frame = *bind_joint_frames.get(joint).ok_or_else(|| {
                        format!(
                            "native Abby braid joint outside bind frame table joint={joint} frames={}",
                            bind_joint_frames.len()
                        )
                    })?;
                    bind_points[index] = frame.transform_point3(Vec3::ZERO);
                }
                AbbyBraidPaletteTarget::Native8 {
                    joints,
                    bind_points,
                }
            }
        };
        let collider_bindings =
            AbbyBraidColliderBindings::from_bind_frames(rig, bind_joint_frames)?;
        let bind = ABBY_BRAID_BIND_POINTS.map(|p| Vec3::new(p[0], p[1], p[2]));
        let attachment_local_points =
            bind.map(|point| attachment_bind_inverse.transform_point3(point));
        let cloth_bind = ABBY_BRAID_CLOTH_BIND_PARTICLES.map(|p| Vec3::new(p[0], p[1], p[2]));
        let cloth_attachment_local_points =
            cloth_bind.map(|point| attachment_bind_inverse.transform_point3(point));
        Ok(Self {
            rig,
            collider_bindings,
            palette_target,
            attachment_bind_inverse,
            attachment_local_points,
            cloth_attachment_local_points,
            points: cloth_bind,
            previous_points: cloth_bind,
            previous_root_velocity_local: Vec3::ZERO,
            last_root_position: None,
            last_root_rotation: None,
            reset_pending: true,
            initialized: false,
        })
    }

    fn mode_label(&self) -> &'static str {
        match self.palette_target {
            AbbyBraidPaletteTarget::Supplemental18 => "legacy-supplemental18",
            AbbyBraidPaletteTarget::Native8 { .. } => "native-joints8",
        }
    }

    fn supplemental_palette_joint_count(&self) -> usize {
        match self.palette_target {
            AbbyBraidPaletteTarget::Supplemental18 => ABBY_BRAID_SOFT_BODY_JOINTS,
            AbbyBraidPaletteTarget::Native8 { .. } => 0,
        }
    }

    fn append_bind_palette(&self, palette: &mut Vec<Mat4>) {
        if matches!(self.palette_target, AbbyBraidPaletteTarget::Supplemental18) {
            palette.extend(std::iter::repeat_n(
                Mat4::IDENTITY,
                ABBY_BRAID_SOFT_BODY_JOINTS,
            ));
        }
    }

    #[inline]
    fn request_reset(&mut self) {
        self.reset_pending = true;
    }

    fn reset(
        &mut self,
        cloth_guide: [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
        root_velocity_local: Vec3,
    ) {
        self.points = cloth_guide;
        self.previous_points = cloth_guide;
        self.previous_root_velocity_local = root_velocity_local;
        self.reset_pending = false;
        self.initialized = true;
    }

    fn guide_from_attachment(
        &self,
        joint_frames: &[Mat4],
    ) -> Result<
        (
            [Vec3; ABBY_BRAID_SOFT_BODY_JOINTS],
            [Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
            Mat4,
        ),
        String,
    > {
        let attachment = *joint_frames.get(self.rig.attachment_joint).ok_or_else(|| {
            format!(
                "Abby braid attachment joint outside animated frame table joint={} frames={}",
                self.rig.attachment_joint,
                joint_frames.len()
            )
        })?;
        Ok((
            self.attachment_local_points
                .map(|point| attachment.transform_point3(point)),
            self.cloth_attachment_local_points
                .map(|point| attachment.transform_point3(point)),
            attachment,
        ))
    }

    fn tick_and_append(
        &mut self,
        dt: f32,
        root_velocity_local: Vec3,
        root_position: Vec3,
        root_rotation: Quat,
        joint_frames: &[Mat4],
        palette: &mut Vec<Mat4>,
    ) -> Result<(), String> {
        let (output_guide, cloth_guide, attachment) = self.guide_from_attachment(joint_frames)?;
        let colliders = self.collider_bindings.from_joint_frames(joint_frames)?;

        let root_rotation = root_rotation.normalize_or_identity();
        if let Some(previous) = self.last_root_position {
            if (root_position - previous).length() > ABBY_BRAID_TELEPORT_RESET_DISTANCE {
                self.request_reset();
            }
        }
        if let Some(previous) = self.last_root_rotation {
            if previous.normalize_or_identity().dot(root_rotation).abs()
                < ABBY_BRAID_TELEPORT_RESET_QUAT_DOT
            {
                self.request_reset();
            }
        }
        self.last_root_position = Some(root_position);
        self.last_root_rotation = Some(root_rotation);

        if !self.initialized || self.reset_pending {
            self.reset(cloth_guide, root_velocity_local);
        } else if dt > 0.0 {
            // The source COLLISION_DATA_CLOTH is a 32-particle / 30-triangle ribbon, not an
            // 18-node chain. Simulate that authored topology first, then bridge its centerline
            // displacement into the 18 deform joints used by the imported glTF braid skin.
            let frame_dt = dt.clamp(1.0 / 240.0, 1.0 / 20.0);
            const ITERATIONS: usize = 5;
            // Swept collision handles true tunneling, while adaptive substeps keep the
            // authored ribbon stable during sprint/low-FPS frames without paying the
            // worst-case cost while Abby is idle.
            let root_speed = root_velocity_local.length();
            let substeps = if frame_dt > 1.0 / 35.0 || root_speed > 4.0 {
                4usize
            } else if frame_dt > 1.0 / 50.0 || root_speed > 2.0 {
                3usize
            } else {
                2usize
            };
            let step_dt = frame_dt / substeps as f32;
            let mut root_acceleration_local =
                (root_velocity_local - self.previous_root_velocity_local) / frame_dt.max(1.0e-5);
            let acceleration_len = root_acceleration_local.length();
            if acceleration_len > 22.0 {
                root_acceleration_local *= 22.0 / acceleration_len;
            }
            self.previous_root_velocity_local = root_velocity_local;

            // Raw source slots are preserved in abby_braid_cloth_authored.rs. These runtime
            // interpretations are intentionally isolated here rather than baked into the data:
            // [2] guide response, [4] inertial coupling, [6] collision skin, [7] gravity scale,
            // [9] edge stiffness reference, [13] bend stiffness, [14] damping reference.
            let gravity = Vec3::new(
                0.0,
                9.81 * ABBY_BRAID_CLOTH_RAW_PARAMS[7] * step_dt * step_dt,
                0.0,
            );
            let inertial_base =
                -root_acceleration_local * (ABBY_BRAID_CLOTH_RAW_PARAMS[4] * step_dt * step_dt);
            let velocity_retention = (1.0 - ABBY_BRAID_CLOTH_RAW_PARAMS[14]).clamp(0.0, 1.0);
            let collision_margin = ABBY_BRAID_CLOTH_RAW_PARAMS[6].max(0.0);

            for _ in 0..substeps {
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

                        resolve_braid_particle_collisions(
                            &mut self.points[index],
                            &mut self.previous_points[index],
                            &colliders,
                            collision_margin,
                        );
                    }
                    // Projection can perturb structural lengths. One authored-edge pass restores
                    // the ribbon, then collision is solved again because the length correction
                    // itself can otherwise pull a particle back through Abby's back.
                    for &(a, b, rest, stiffness, _damping) in &ABBY_BRAID_CLOTH_EDGES {
                        solve_authored_cloth_edge(&mut self.points, a, b, rest, stiffness);
                    }
                    for index in 0..ABBY_BRAID_CLOTH_PARTICLE_COUNT {
                        if ABBY_BRAID_CLOTH_SCALAR0[index] <= 1.0e-8 {
                            continue;
                        }
                        resolve_braid_particle_collisions(
                            &mut self.points[index],
                            &mut self.previous_points[index],
                            &colliders,
                            collision_margin,
                        );
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

        let deformed_output =
            bridge_authored_cloth_to_braid_joints(&output_guide, &cloth_guide, &self.points);
        let attachment_delta = attachment * self.attachment_bind_inverse;
        let (_, attachment_delta_rotation, _) = attachment_delta.to_scale_rotation_translation();
        let deformation_for = |index: usize, bind_point: Vec3| {
            let guide_direction = if index + 1 < ABBY_BRAID_SOFT_BODY_JOINTS {
                output_guide[index + 1] - output_guide[index]
            } else {
                output_guide[index] - output_guide[index - 1]
            }
            .normalize_or_zero();
            let current_direction = if index + 1 < ABBY_BRAID_SOFT_BODY_JOINTS {
                deformed_output[index + 1] - deformed_output[index]
            } else {
                deformed_output[index] - deformed_output[index - 1]
            }
            .normalize_or_zero();
            let bend = if guide_direction.length_squared() > 1.0e-8
                && current_direction.length_squared() > 1.0e-8
            {
                Quat::from_rotation_arc(guide_direction, current_direction)
            } else {
                Quat::IDENTITY
            };
            let rotation = (bend * attachment_delta_rotation).normalize_or_identity();
            Mat4::from_translation(deformed_output[index])
                * Mat4::from_quat(rotation)
                * Mat4::from_translation(-bind_point)
        };
        match &self.palette_target {
            AbbyBraidPaletteTarget::Supplemental18 => {
                let bind = ABBY_BRAID_BIND_POINTS.map(|p| Vec3::new(p[0], p[1], p[2]));
                for index in 0..ABBY_BRAID_SOFT_BODY_JOINTS {
                    palette.push(deformation_for(index, bind[index]));
                }
            }
            AbbyBraidPaletteTarget::Native8 {
                joints,
                bind_points,
            } => {
                for target_index in 0..joints.len() {
                    let output_index = ABBY_NATIVE_BRAID_OUTPUT_INDICES[target_index];
                    let joint = joints[target_index];
                    let palette_len = palette.len();
                    let slot = palette.get_mut(joint).ok_or_else(|| {
                        format!(
                            "native Abby braid palette joint outside palette joint={joint} palette={palette_len}"
                        )
                    })?;
                    *slot = deformation_for(output_index, bind_points[target_index]);
                }
            }
        }
        Ok(())
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

fn bridge_authored_cloth_to_braid_joints(
    output_guide: &[Vec3; ABBY_BRAID_SOFT_BODY_JOINTS],
    cloth_guide: &[Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
    cloth_points: &[Vec3; ABBY_BRAID_CLOTH_PARTICLE_COUNT],
) -> [Vec3; ABBY_BRAID_SOFT_BODY_JOINTS] {
    let guide_centerline = authored_cloth_centerline(cloth_guide);
    let current_centerline = authored_cloth_centerline(cloth_points);
    let bind_output = ABBY_BRAID_BIND_POINTS.map(|p| Vec3::new(p[0], p[1], p[2]));
    let mut output = *output_guide;
    for index in ABBY_BRAID_OUTPUT_KINEMATIC_JOINTS..ABBY_BRAID_SOFT_BODY_JOINTS {
        let t = normalized_polyline_parameter(&bind_output, index);
        let guide_sample = sample_polyline_normalized(&guide_centerline, t);
        let current_sample = sample_polyline_normalized(&current_centerline, t);
        output[index] += current_sample - guide_sample;
    }
    output
}

fn point_segment_closest(point: Vec3, a: Vec3, b: Vec3) -> (Vec3, f32) {
    let axis = b - a;
    let axis_len_sq = axis.length_squared();
    if axis_len_sq <= 1.0e-10 {
        return (a, 0.0);
    }
    let t = ((point - a).dot(axis) / axis_len_sq).clamp(0.0, 1.0);
    (a + axis * t, t)
}

fn closest_parameters_between_segments(p0: Vec3, p1: Vec3, q0: Vec3, q1: Vec3) -> (f32, f32) {
    let d1 = p1 - p0;
    let d2 = q1 - q0;
    let r = p0 - q0;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let eps = 1.0e-10;
    if a <= eps && e <= eps {
        return (0.0, 0.0);
    }
    if a <= eps {
        return (0.0, (d2.dot(r) / e.max(eps)).clamp(0.0, 1.0));
    }
    let c = d1.dot(r);
    if e <= eps {
        return ((-c / a).clamp(0.0, 1.0), 0.0);
    }
    let b = d1.dot(d2);
    let f = d2.dot(r);
    let denom = a * e - b * b;
    let mut s = if denom.abs() > eps {
        ((b * f - c * e) / denom).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut t = (b * s + f) / e;
    if t < 0.0 {
        t = 0.0;
        s = (-c / a).clamp(0.0, 1.0);
    } else if t > 1.0 {
        t = 1.0;
        s = ((b - c) / a).clamp(0.0, 1.0);
    }
    (s, t)
}

fn apply_braid_contact_velocity_response(point: Vec3, previous: &mut Vec3, normal: Vec3) {
    let normal = normal.normalize_or_zero();
    if normal.length_squared() <= 1.0e-8 {
        *previous = point;
        return;
    }
    let velocity = point - *previous;
    let normal_speed = velocity.dot(normal);
    let tangent = velocity - normal * normal_speed;
    let retained = tangent * 0.86 + normal * normal_speed.max(0.0) * 0.20;
    *previous = point - retained;
}

fn sweep_point_against_capsule(
    point: &mut Vec3,
    previous: Vec3,
    a: Vec3,
    b: Vec3,
    radius: f32,
) -> Option<Vec3> {
    let radius = radius.max(1.0e-5);
    let radius_sq = radius * radius;
    let (start_axis, _) = point_segment_closest(previous, a, b);
    if (previous - start_axis).length_squared() <= radius_sq {
        return None;
    }
    let end = *point;
    let (path_t, axis_t) = closest_parameters_between_segments(previous, end, a, b);
    let path_closest = previous.lerp(end, path_t);
    let axis_closest = a.lerp(b, axis_t);
    if (path_closest - axis_closest).length_squared() > radius_sq {
        return None;
    }
    let mut lo = 0.0f32;
    let mut hi = path_t.max(1.0e-6);
    for _ in 0..12 {
        let mid = (lo + hi) * 0.5;
        let candidate = previous.lerp(end, mid);
        let (axis_point, _) = point_segment_closest(candidate, a, b);
        if (candidate - axis_point).length_squared() <= radius_sq {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let hit = previous.lerp(end, hi);
    let (axis_point, _) = point_segment_closest(hit, a, b);
    let mut normal = (hit - axis_point).normalize_or_zero();
    if normal.length_squared() <= 1.0e-8 {
        normal = (previous - start_axis).normalize_or_zero();
    }
    if normal.length_squared() <= 1.0e-8 {
        normal = Vec3::new(0.0, 0.0, -1.0);
    }
    *point = axis_point + normal * (radius + 1.0e-4);
    Some(normal)
}

fn sweep_point_against_oriented_box(
    point: &mut Vec3,
    previous: Vec3,
    oriented_box: AbbyBraidOrientedBox,
) -> Option<Vec3> {
    let to_local = |world: Vec3| {
        let delta = world - oriented_box.center;
        Vec3::new(
            delta.dot(oriented_box.axes[0]),
            delta.dot(oriented_box.axes[1]),
            delta.dot(oriented_box.axes[2]),
        )
    };
    let start = to_local(previous);
    let end = to_local(*point);
    let ext = oriented_box.half_extents;
    if start.x.abs() <= ext.x && start.y.abs() <= ext.y && start.z.abs() <= ext.z {
        return None;
    }
    let delta = end - start;
    let starts = [start.x, start.y, start.z];
    let dirs = [delta.x, delta.y, delta.z];
    let exts = [ext.x, ext.y, ext.z];
    let mut t_enter = 0.0f32;
    let mut t_exit = 1.0f32;
    let mut enter_axis = 0usize;
    let mut enter_sign = 0.0f32;
    for axis in 0..3 {
        let origin = starts[axis];
        let direction = dirs[axis];
        let extent = exts[axis];
        if direction.abs() <= 1.0e-8 {
            if origin < -extent || origin > extent {
                return None;
            }
            continue;
        }
        let ta = (-extent - origin) / direction;
        let tb = (extent - origin) / direction;
        let (near, far, normal_sign) = if ta <= tb {
            (ta, tb, -1.0)
        } else {
            (tb, ta, 1.0)
        };
        if near > t_enter {
            t_enter = near;
            enter_axis = axis;
            enter_sign = normal_sign;
        }
        t_exit = t_exit.min(far);
        if t_enter > t_exit {
            return None;
        }
    }
    if !(0.0..=1.0).contains(&t_enter) || t_exit < 0.0 {
        return None;
    }
    let mut hit_local = start + delta * t_enter;
    match enter_axis {
        0 => hit_local.x += enter_sign * 1.0e-4,
        1 => hit_local.y += enter_sign * 1.0e-4,
        _ => hit_local.z += enter_sign * 1.0e-4,
    }
    *point = oriented_box.center
        + oriented_box.axes[0] * hit_local.x
        + oriented_box.axes[1] * hit_local.y
        + oriented_box.axes[2] * hit_local.z;
    Some(oriented_box.axes[enter_axis] * enter_sign)
}

fn project_out_of_oriented_box(point: &mut Vec3, oriented_box: AbbyBraidOrientedBox) {
    let delta = *point - oriented_box.center;
    let local = Vec3::new(
        delta.dot(oriented_box.axes[0]),
        delta.dot(oriented_box.axes[1]),
        delta.dot(oriented_box.axes[2]),
    );
    let extents = oriented_box.half_extents;
    if local.x.abs() >= extents.x || local.y.abs() >= extents.y || local.z.abs() >= extents.z {
        return;
    }
    let penetrations = [
        extents.x - local.x.abs(),
        extents.y - local.y.abs(),
        extents.z - local.z.abs(),
    ];
    let mut axis_index = 0usize;
    if penetrations[1] < penetrations[axis_index] {
        axis_index = 1;
    }
    if penetrations[2] < penetrations[axis_index] {
        axis_index = 2;
    }
    let coordinate = [local.x, local.y, local.z][axis_index];
    let sign = if coordinate < 0.0 { -1.0 } else { 1.0 };
    *point += oriented_box.axes[axis_index] * (penetrations[axis_index] * sign);
}

fn project_out_of_capsule(point: &mut Vec3, a: Vec3, b: Vec3, radius: f32) {
    let (closest, _) = point_segment_closest(*point, a, b);
    let delta = *point - closest;
    let distance = delta.length();
    if distance < radius {
        let normal = if distance > 1.0e-6 {
            delta / distance
        } else {
            Vec3::new(0.0, 0.0, -1.0)
        };
        *point = closest + normal * radius;
    }
}

fn resolve_braid_particle_collisions(
    point: &mut Vec3,
    previous: &mut Vec3,
    colliders: &AbbyBraidColliderSet,
    collision_margin: f32,
) {
    const CONTACT_SKIN: f32 = 0.0035;
    let margin = collision_margin.max(0.0) + CONTACT_SKIN;
    for capsule in colliders.capsules {
        let radius = capsule.radius + margin;
        if let Some(normal) =
            sweep_point_against_capsule(point, *previous, capsule.a, capsule.b, radius)
        {
            apply_braid_contact_velocity_response(*point, previous, normal);
        }
        let before = *point;
        project_out_of_capsule(point, capsule.a, capsule.b, radius);
        let correction = *point - before;
        if correction.length_squared() > 1.0e-12 {
            apply_braid_contact_velocity_response(*point, previous, correction);
        }
    }
    for mut oriented_box in colliders.boxes {
        oriented_box.half_extents += Vec3::splat(margin);
        if let Some(normal) = sweep_point_against_oriented_box(point, *previous, oriented_box) {
            apply_braid_contact_velocity_response(*point, previous, normal);
        }
        let before = *point;
        project_out_of_oriented_box(point, oriented_box);
        let correction = *point - before;
        if correction.length_squared() > 1.0e-12 {
            apply_braid_contact_velocity_response(*point, previous, correction);
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct PlayerAnimationRuntimeBinding {
    clips: [Option<PlayerAnimationRuntimeClip>; 8],
    active_state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    active_slot: usize,
    skeleton: ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    time_seconds: f32,
    /// Pose currently visible on the character. This is preserved when a new
    /// locomotion state interrupts an in-flight cross-fade.
    current_locals: Vec<JointLocalPose>,
    sampled_target_locals: Vec<JointLocalPose>,
    transition_from_locals: Vec<JointLocalPose>,
    palette_scratch: Vec<Mat4>,
    /// Absolute bind joint frames in baked model space. Current animated frames are derived as
    /// `skin_palette * bind_frame`, after all pose/follower corrections but before braid solve.
    bind_joint_frames: Vec<Mat4>,
    joint_frames_scratch: Vec<Mat4>,
    /// Mirrored North Star deform/helper branches must follow their primary joints.
    helper_mirror_pairs: Vec<(usize, usize)>,
    /// Imported Rigify control/face branches need the authored constraint order restored:
    /// deform body -> animated neck/head controls -> face/eyes deform branches.
    eye_contract: Option<AbbyEyeRuntimeContract>,
    head_follow: Option<DetachedHeadFollowRig>,
    braid_soft_body: Option<AbbyBraidSoftBodyRuntime>,
    equipped_rifle_basepose: Option<PlayerAnimationRuntimeClip>,
    equipped_rifle_reload: Option<PlayerAnimationRuntimeClip>,
    equipment_overlay_locals: Vec<JointLocalPose>,
    rifle_ik: Option<AbbyRifleIkRig>,
}

#[inline]
const fn locomotion_slot(
    state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
) -> usize {
    use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;
    match state {
        L::Idle => 0,
        L::Walk => 1,
        L::Run => 2,
        L::Sprint => 3,
        L::CrouchIdle => 4,
        L::CrouchWalk => 5,
        L::Jump => 6,
        L::Fall => 7,
    }
}

impl PlayerAnimationRuntimeBinding {
    pub(super) fn initial_palette(&self) -> Vec<Mat4> {
        let mut palette = self.palette_scratch.clone();
        if let Some(braid) = self.braid_soft_body.as_ref() {
            braid.append_bind_palette(&mut palette);
        }
        palette
    }

    pub(super) fn skeleton_joint_count(&self) -> usize {
        self.skeleton.joints.len()
    }

    pub(super) fn supplemental_palette_joint_count(&self) -> usize {
        self.braid_soft_body
            .as_ref()
            .map(AbbyBraidSoftBodyRuntime::supplemental_palette_joint_count)
            .unwrap_or(0)
    }

    pub(super) fn expected_palette_joints(&self) -> usize {
        self.skeleton_joint_count() + self.supplemental_palette_joint_count()
    }

    pub(super) fn clip_refs_csv(&self) -> String {
        self.clips
            .iter()
            .filter_map(|clip| clip.as_ref().map(|clip| clip.clip_ref.as_str()))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn resolve_slot(
        &self,
        state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    ) -> usize {
        use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;
        let candidates: &[usize] = match state {
            L::Idle => &[0],
            L::Walk => &[1, 0],
            L::Run => &[2, 1, 0],
            L::Sprint => &[3, 2, 1, 0],
            L::CrouchIdle => &[4, 0],
            L::CrouchWalk => &[5, 4, 1, 0],
            L::Jump => &[6, 2, 0],
            L::Fall => &[7, 6, 2, 0],
        };
        candidates
            .iter()
            .copied()
            .find(|slot| self.clips[*slot].is_some())
            .unwrap_or(0)
    }
}

const ABBY_RIFLE_BASEPOSE_REF: &str = "animations/characters/abby/moveset.ycd@fob-car-ride-abby-moveset-truck-rear-aim-00bw-aim--abby";
const ABBY_RIFLE_RELOAD_REF: &str = "animations/characters/abby/moveset.ycd@fob-car-ride-rifle-vepr-crouch-reload-rear-part";
/// ReadyHold weapon/chest calibration was recovered from frame 24 of this 51-frame authored aim
/// cycle. Sampling the same phase keeps authored arm rotation style while the standing weapon
/// placement itself remains governed by the anatomical ReadyHold contract.
const ABBY_RIFLE_READY_SAMPLE_PHASE: f32 = 0.48;
/// Rotation-only ready-hold overlay. Car-ride translations must never be copied into standing
/// locomotion: doing so changes shoulder/arm chain geometry and makes the runtime solve a seated pose.
const ABBY_RIFLE_READY_ROTATION_WEIGHTS: &[(&str, f32)] = &[
    ("spineb", 0.22),
    ("spinec", 0.38),
    ("spined", 0.52),
    ("l_clavicle", 0.78),
    ("r_clavicle", 0.78),
    ("l_shoulder", 0.92),
    ("r_shoulder", 0.92),
    ("l_elbow", 1.0),
    ("r_elbow", 1.0),
    ("l_wrist", 1.0),
    ("r_wrist", 1.0),
    ("l_palm", 1.0),
    ("r_palm", 1.0),
];
/// The original Abby rifle reload drives the complete upper-body manipulation. Translation
/// channels stay excluded because this source was authored in vehicle/crouch space; its local
/// joint rotations are the reusable semantic content. Right-hand IK later keeps the firing hand
/// on the receiver while the left arm remains intentionally free to manipulate the magazine.
const ABBY_RIFLE_RELOAD_ROTATION_WEIGHTS: &[(&str, f32)] = &[
    ("spineb", 0.28),
    ("spinec", 0.48),
    ("spined", 0.68),
    ("l_clavicle", 1.0),
    ("r_clavicle", 0.92),
    ("l_shoulder", 1.0),
    ("r_shoulder", 1.0),
    ("l_elbow", 1.0),
    ("r_elbow", 1.0),
    ("l_wrist", 1.0),
    ("r_wrist", 1.0),
    ("l_palm", 1.0),
    ("r_palm", 1.0),
];

#[inline]
fn blend_joint_rotation_only(dst: &mut JointLocalPose, src: &JointLocalPose, weight: f32) {
    let weight = if weight.is_finite() {
        weight.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let from = Quat::from_xyzw(
        dst.rotation[0],
        dst.rotation[1],
        dst.rotation[2],
        dst.rotation[3],
    )
    .normalize_or_identity();
    let mut to = Quat::from_xyzw(
        src.rotation[0],
        src.rotation[1],
        src.rotation[2],
        src.rotation[3],
    )
    .normalize_or_identity();
    if from.dot(to) < 0.0 {
        to = Quat::from_xyzw(-to.x, -to.y, -to.z, -to.w);
    }
    let rotation = from.slerp(to, weight).normalize_or_identity();
    dst.rotation = [rotation.x, rotation.y, rotation.z, rotation.w];
}

fn apply_equipped_rifle_rotation_overlay(
    clip: Option<&PlayerAnimationRuntimeClip>,
    skeleton: &ModelSkeletonMetadata,
    scratch: &mut Vec<JointLocalPose>,
    target: &mut [JointLocalPose],
    normalized_phase: f32,
    weights: &[(&str, f32)],
) -> Result<(), String> {
    let Some(clip) = clip else {
        return Ok(());
    };
    let phase = if normalized_phase.is_finite() {
        normalized_phase.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let sample_time = (clip.clip.duration_seconds * phase)
        .clamp(0.0, clip.clip.duration_seconds.max(0.0));
    clip.clip
        .sample_local_pose_for_skeleton(sample_time, skeleton, scratch)?;
    for (name, weight) in weights {
        let Some(index) = skeleton.joints.iter().position(|joint| joint.name == *name) else {
            continue;
        };
        if let (Some(dst), Some(src)) = (target.get_mut(index), scratch.get(index)) {
            blend_joint_rotation_only(dst, src, *weight);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct AbbyRifleIkRig {
    chest: usize,
    right_shoulder: usize,
    right_elbow: usize,
    right_wrist: usize,
    right_palm: usize,
    left_shoulder: usize,
    left_elbow: usize,
    left_wrist: usize,
    left_palm: usize,
}

fn build_abby_rifle_ik_rig(skeleton: &ModelSkeletonMetadata) -> Option<AbbyRifleIkRig> {
    let find = |name: &str| skeleton.joints.iter().position(|joint| joint.name == name);
    Some(AbbyRifleIkRig {
        chest: find("spined")?,
        right_shoulder: find("r_shoulder")?,
        right_elbow: find("r_elbow")?,
        right_wrist: find("r_wrist")?,
        right_palm: find("r_palm")?,
        left_shoulder: find("l_shoulder")?,
        left_elbow: find("l_elbow")?,
        left_wrist: find("l_wrist")?,
        left_palm: find("l_palm")?,
    })
}

fn rebuild_model_joint_frames(
    skeleton: &ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    pose: &[JointLocalPose],
    frames: &mut Vec<Mat4>,
) -> Result<(), String> {
    frames.clear();
    build_model_joint_frames_from_local_pose(skeleton, source_to_model, pose, frames)
}

fn rotate_pose_joint_toward(
    skeleton: &ModelSkeletonMetadata,
    pose: &mut [JointLocalPose],
    frames: &[Mat4],
    joint_index: usize,
    end_effector_index: usize,
    target: Vec3,
    correction_weight: f32,
) -> Result<(), String> {
    let joint_frame = *frames
        .get(joint_index)
        .ok_or_else(|| format!("rifle IK joint frame missing index={joint_index}"))?;
    let end_frame = *frames
        .get(end_effector_index)
        .ok_or_else(|| format!("rifle IK end frame missing index={end_effector_index}"))?;
    let joint_position = joint_frame.transform_point3(Vec3::ZERO);
    let end_position = end_frame.transform_point3(Vec3::ZERO);
    let to_end = end_position - joint_position;
    let to_target = target - joint_position;
    if !to_end.is_finite()
        || !to_target.is_finite()
        || to_end.length_squared() <= 1.0e-10
        || to_target.length_squared() <= 1.0e-10
    {
        return Ok(());
    }

    let full_delta =
        Quat::from_rotation_arc(to_end.normalize(), to_target.normalize()).normalize_or_identity();
    let correction_weight = if correction_weight.is_finite() {
        correction_weight.clamp(0.0, 1.0)
    } else {
        0.5
    };
    let delta = Quat::IDENTITY
        .slerp(full_delta, correction_weight)
        .normalize_or_identity();
    let (_, joint_global_rotation, _) = joint_frame.to_scale_rotation_translation();
    let parent_global_rotation = skeleton.joints[joint_index]
        .parent_index
        .and_then(|parent| frames.get(parent as usize).copied())
        .map(|frame| frame.to_scale_rotation_translation().1)
        .unwrap_or(Quat::IDENTITY);
    let desired_global = (delta * joint_global_rotation).normalize_or_identity();
    let local_rotation =
        (parent_global_rotation.inverse() * desired_global).normalize_or_identity();
    let local = pose
        .get_mut(joint_index)
        .ok_or_else(|| format!("rifle IK local pose missing index={joint_index}"))?;
    local.rotation = [
        local_rotation.x,
        local_rotation.y,
        local_rotation.z,
        local_rotation.w,
    ];
    Ok(())
}

fn solve_two_bone_arm_with_pole(
    skeleton: &ModelSkeletonMetadata,
    pose: &mut [JointLocalPose],
    frames: &mut Vec<Mat4>,
    source_to_model: [f32; 16],
    shoulder: usize,
    elbow: usize,
    palm: usize,
    target: Vec3,
    pole: Vec3,
) -> Result<(), String> {
    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    let shoulder_position = frames
        .get(shoulder)
        .copied()
        .ok_or("rifle IK shoulder frame missing")?
        .transform_point3(Vec3::ZERO);
    let elbow_position = frames
        .get(elbow)
        .copied()
        .ok_or("rifle IK elbow frame missing")?
        .transform_point3(Vec3::ZERO);
    let palm_position = frames
        .get(palm)
        .copied()
        .ok_or("rifle IK palm frame missing")?
        .transform_point3(Vec3::ZERO);
    let upper_len = (elbow_position - shoulder_position).length();
    let lower_len = (palm_position - elbow_position).length();
    let raw_to_target = target - shoulder_position;
    let raw_distance = raw_to_target.length();
    if !upper_len.is_finite()
        || !lower_len.is_finite()
        || !raw_distance.is_finite()
        || upper_len <= 1.0e-5
        || lower_len <= 1.0e-5
        || raw_distance <= 1.0e-5
    {
        return Ok(());
    }

    let direction = raw_to_target / raw_distance;
    let min_reach = (upper_len - lower_len).abs() + 1.0e-4;
    let max_reach = (upper_len + lower_len - 1.0e-4).max(min_reach);
    let distance = raw_distance.clamp(min_reach, max_reach);
    let reachable_target = shoulder_position + direction * distance;

    let pole_vector = pole - shoulder_position;
    let mut bend_direction = pole_vector - direction * pole_vector.dot(direction);
    if bend_direction.length_squared() <= 1.0e-8 {
        let current_bend = elbow_position - shoulder_position;
        bend_direction = current_bend - direction * current_bend.dot(direction);
    }
    bend_direction = bend_direction.normalize_or_zero();
    if bend_direction.length_squared() <= 1.0e-8 {
        return Ok(());
    }

    let along = ((upper_len * upper_len - lower_len * lower_len + distance * distance)
        / (2.0 * distance))
        .clamp(0.0, upper_len);
    let height = (upper_len * upper_len - along * along).max(0.0).sqrt();
    let desired_elbow = shoulder_position + direction * along + bend_direction * height;

    // First orient the upper arm into the preferred elbow plane, then close the forearm onto the
    // palm target. No free CCD iterations remain, so the elbow cannot flip to another plane.
    rotate_pose_joint_toward(skeleton, pose, frames, shoulder, elbow, desired_elbow, 1.0)?;
    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    rotate_pose_joint_toward(skeleton, pose, frames, elbow, palm, reachable_target, 1.0)?;
    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    Ok(())
}

fn orient_wrist_for_palm_basis(
    skeleton: &ModelSkeletonMetadata,
    pose: &mut [JointLocalPose],
    frames: &[Mat4],
    wrist: usize,
    palm: usize,
    desired_palm_global: Quat,
    max_correction_radians: f32,
) -> Result<(), String> {
    let current_wrist = frames
        .get(wrist)
        .copied()
        .ok_or("rifle wrist frame missing")?
        .to_scale_rotation_translation()
        .1
        .normalize_or_identity();
    let palm_local = pose
        .get(palm)
        .ok_or("rifle palm local pose missing")?
        .rotation;
    let palm_local = Quat::from_xyzw(palm_local[0], palm_local[1], palm_local[2], palm_local[3])
        .normalize_or_identity();
    let desired_wrist = (desired_palm_global * palm_local.inverse()).normalize_or_identity();
    let dot = current_wrist.dot(desired_wrist).abs().clamp(0.0, 1.0);
    let angle = 2.0 * dot.acos();
    let weight = if angle.is_finite() && angle > max_correction_radians.max(1.0e-4) {
        (max_correction_radians / angle).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let limited = current_wrist
        .slerp(desired_wrist, weight)
        .normalize_or_identity();
    set_pose_joint_global_rotation(skeleton, pose, frames, wrist, limited)
}

fn set_pose_joint_global_rotation(
    skeleton: &ModelSkeletonMetadata,
    pose: &mut [JointLocalPose],
    frames: &[Mat4],
    joint_index: usize,
    desired_global: Quat,
) -> Result<(), String> {
    let parent_global = skeleton.joints[joint_index]
        .parent_index
        .and_then(|parent| frames.get(parent as usize).copied())
        .map(|frame| frame.to_scale_rotation_translation().1)
        .unwrap_or(Quat::IDENTITY)
        .normalize_or_identity();
    let local_rotation = (parent_global.inverse() * desired_global).normalize_or_identity();
    let local = pose
        .get_mut(joint_index)
        .ok_or_else(|| format!("rifle ready local pose missing index={joint_index}"))?;
    local.rotation = [
        local_rotation.x,
        local_rotation.y,
        local_rotation.z,
        local_rotation.w,
    ];
    Ok(())
}

/// ReadyHold is stock/shoulder-anchored because the current native corpus has no standalone standing rifle
/// clip. The weapon is placed from `spined`; both arms solve to calibrated palm-center contacts.
/// left palm follows the canonical rifle `l_grip`; it never feeds back into weapon transform.
fn apply_equipped_rifle_support_ik(
    rig: Option<&AbbyRifleIkRig>,
    skeleton: &ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    pose: &mut Vec<JointLocalPose>,
    frames: &mut Vec<Mat4>,
    view_forward_model: Option<Vec3>,
    aim_alpha: f32,
    recoil_alpha: f32,
    support_left_hand: bool,
) -> Result<Option<f32>, String> {
    let Some(rig) = rig else {
        return Ok(None);
    };
    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    let chest = *frames
        .get(rig.chest)
        .ok_or("rifle ReadyHold chest frame is unavailable")?;
    let right_shoulder = *frames
        .get(rig.right_shoulder)
        .ok_or("rifle ReadyHold right shoulder frame is unavailable")?;
    let left_shoulder = *frames
        .get(rig.left_shoulder)
        .ok_or("rifle ReadyHold left shoulder frame is unavailable")?;
    let contract = crate::weapon_grip::rifle_ready_solve_contract_presented(
        chest,
        right_shoulder,
        left_shoulder,
        view_forward_model,
        aim_alpha,
        recoil_alpha,
    )
    .ok_or("rifle ReadyHold could not resolve anatomical solve contract")?;
    let right_target = crate::weapon_grip::rifle_ready_right_palm_position(contract.root);
    let left_target = crate::weapon_grip::rifle_ready_left_palm_position(contract.root);

    solve_two_bone_arm_with_pole(
        skeleton,
        pose,
        frames,
        source_to_model,
        rig.right_shoulder,
        rig.right_elbow,
        rig.right_palm,
        right_target,
        contract.right_elbow_pole,
    )?;
    if support_left_hand {
        solve_two_bone_arm_with_pole(
            skeleton,
            pose,
            frames,
            source_to_model,
            rig.left_shoulder,
            rig.left_elbow,
            rig.left_palm,
            left_target,
            contract.left_elbow_pole,
        )?;
    }

    // Wrist orientation is a separate constrained pass. The palm contact calibration supplies the
    // desired grip basis, while a maximum correction prevents the wrist from absorbing an entire
    // arm-plane mismatch as twist.
    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    orient_wrist_for_palm_basis(
        skeleton,
        pose,
        frames,
        rig.right_wrist,
        rig.right_palm,
        crate::weapon_grip::rifle_ready_right_palm_rotation(contract.root),
        35.0_f32.to_radians(),
    )?;
    if support_left_hand {
        rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
        orient_wrist_for_palm_basis(
            skeleton,
            pose,
            frames,
            rig.left_wrist,
            rig.left_palm,
            crate::weapon_grip::rifle_ready_left_palm_rotation(contract.root),
            40.0_f32.to_radians(),
        )?;
    }

    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    let right_error = (frames[rig.right_palm].transform_point3(Vec3::ZERO) - right_target).length();
    let left_error = if support_left_hand {
        (frames[rig.left_palm].transform_point3(Vec3::ZERO) - left_target).length()
    } else {
        0.0
    };
    let stock_error = (contract.stock_contact - contract.shoulder_pocket).length();
    let error = right_error.max(left_error).max(stock_error);
    if !error.is_finite() {
        return Err("rifle ReadyHold IK produced non-finite contact error".to_owned());
    }
    Ok(Some(error))
}

fn build_helper_mirror_pairs(skeleton: &ModelSkeletonMetadata) -> Vec<(usize, usize)> {
    use std::collections::HashMap;

    let by_name = skeleton
        .joints
        .iter()
        .enumerate()
        .map(|(index, joint)| (joint.name.as_str(), index))
        .collect::<HashMap<_, _>>();
    skeleton
        .joints
        .iter()
        .enumerate()
        .filter_map(|(helper_index, joint)| {
            let primary_name = joint.name.strip_suffix("_helper")?;
            let primary_index = *by_name.get(primary_name)?;
            (primary_index != helper_index).then_some((helper_index, primary_index))
        })
        .collect()
}

#[inline]
fn synchronize_helper_pose(pairs: &[(usize, usize)], pose: &mut [JointLocalPose]) {
    for &(helper_index, primary_index) in pairs {
        if helper_index < pose.len() && primary_index < pose.len() {
            pose[helper_index] = pose[primary_index];
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct AbbyEyeRuntimeContract {
    left: usize,
    right: usize,
    parent: usize,
}

fn build_abby_eye_runtime_contract(
    skeleton: &ModelSkeletonMetadata,
) -> Option<AbbyEyeRuntimeContract> {
    let left = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == "l_eyeball")?;
    let right = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == "r_eyeball")?;
    let parent = skeleton.joints.get(left)?.parent_index? as usize;
    if skeleton
        .joints
        .get(right)?
        .parent_index
        .map(|value| value as usize)
        != Some(parent)
        || skeleton.joints.get(parent)?.name != "headb"
    {
        return None;
    }
    Some(AbbyEyeRuntimeContract {
        left,
        right,
        parent,
    })
}

fn stabilize_abby_eye_locals(
    contract: Option<&AbbyEyeRuntimeContract>,
    skeleton: &ModelSkeletonMetadata,
    pose: &mut [JointLocalPose],
) -> Result<(), String> {
    let Some(contract) = contract else {
        return Ok(());
    };
    for index in [contract.left, contract.right] {
        let joint = skeleton
            .joints
            .get(index)
            .ok_or_else(|| format!("Abby eye joint outside skeleton index={index}"))?;
        let dst = pose
            .get_mut(index)
            .ok_or_else(|| format!("Abby eye joint outside sampled pose index={index}"))?;
        *dst = JointLocalPose {
            translation: joint.position_ls,
            rotation: joint.rotation_ls,
            scale: Some(joint.scale_ls),
        };
    }
    Ok(())
}

#[inline]
fn matrix_max_abs_delta(a: Mat4, b: Mat4) -> f32 {
    a.to_cols_array()
        .into_iter()
        .zip(b.to_cols_array())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max)
}

fn validate_abby_eye_palette(
    contract: Option<&AbbyEyeRuntimeContract>,
    palette: &[Mat4],
) -> Result<(), String> {
    let Some(contract) = contract else {
        return Ok(());
    };
    let parent = *palette
        .get(contract.parent)
        .ok_or_else(|| "Abby eye parent outside skin palette".to_owned())?;
    for (side, index) in [("left", contract.left), ("right", contract.right)] {
        let eye = *palette
            .get(index)
            .ok_or_else(|| format!("Abby {side} eye outside skin palette index={index}"))?;
        let drift = matrix_max_abs_delta(eye, parent);
        // With authored bind-local eyes, A_eye=A_parent*Lbind and B_eye=B_parent*Lbind,
        // therefore A_eye*inverse(B_eye) must reduce to the exact parent deformation.
        if !drift.is_finite() || drift > 5.0e-4 {
            return Err(format!(
                "Abby {side} eye palette drift violates animated_global*inverse_bind contract index={index} parent={} max_abs_delta={drift:.8}",
                contract.parent
            ));
        }
    }
    Ok(())
}

fn debug_dump_abby_eye_matrices(
    contract: Option<&AbbyEyeRuntimeContract>,
    bind_joint_frames: &[Mat4],
    current_locals: &[JointLocalPose],
    palette: &[Mat4],
    context: &str,
) {
    let Some(contract) = contract else {
        return;
    };
    if std::env::var_os("NORTHSTAR_DEBUG_ABBY_EYES").is_none() {
        return;
    }
    let Some(parent_bind_global) = bind_joint_frames.get(contract.parent).copied() else {
        return;
    };
    let Some(parent_palette) = palette.get(contract.parent).copied() else {
        return;
    };
    let parent_global = parent_palette * parent_bind_global;
    for (side, index) in [("left", contract.left), ("right", contract.right)] {
        let (Some(bind_global), Some(local), Some(palette_matrix)) = (
            bind_joint_frames.get(index).copied(),
            current_locals.get(index),
            palette.get(index).copied(),
        ) else {
            continue;
        };
        let scale = local.scale.unwrap_or([1.0, 1.0, 1.0]);
        let animated_local = Mat4::from_scale_rotation_translation(
            Vec3::new(scale[0], scale[1], scale[2]),
            Quat::from_xyzw(
                local.rotation[0],
                local.rotation[1],
                local.rotation[2],
                local.rotation[3],
            )
            .normalize_or_identity(),
            Vec3::new(
                local.translation[0],
                local.translation[1],
                local.translation[2],
            ),
        );
        let animated_global = palette_matrix * bind_global;
        newengine_ulog_api::ulog::info!(
            "ABBY_EYE_MATRIX context='{}' side={} joint={} parent={} bind_global={:?} parent_global={:?} animated_local={:?} animated_global={:?} palette_matrix={:?} parent_palette={:?} palette_parent_drift={:.8}",
            context,
            side,
            index,
            contract.parent,
            bind_global,
            parent_global,
            animated_local,
            animated_global,
            palette_matrix,
            parent_palette,
            matrix_max_abs_delta(palette_matrix, parent_palette),
        );
    }
}

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

fn blend_local_poses(
    from: &[JointLocalPose],
    to: &[JointLocalPose],
    alpha: f32,
    out: &mut Vec<JointLocalPose>,
) -> Result<(), String> {
    if from.len() != to.len() {
        return Err(format!(
            "animation transition pose count mismatch from={} to={}",
            from.len(),
            to.len()
        ));
    }
    let alpha = if alpha.is_finite() {
        alpha.clamp(0.0, 1.0)
    } else {
        1.0
    };
    out.clear();
    out.reserve(to.len());
    for (a, b) in from.iter().zip(to.iter()) {
        let translation = Vec3::new(a.translation[0], a.translation[1], a.translation[2]).lerp(
            Vec3::new(b.translation[0], b.translation[1], b.translation[2]),
            alpha,
        );
        let qa = Quat::from_xyzw(a.rotation[0], a.rotation[1], a.rotation[2], a.rotation[3])
            .normalize_or_identity();
        let mut qb = Quat::from_xyzw(b.rotation[0], b.rotation[1], b.rotation[2], b.rotation[3])
            .normalize_or_identity();
        if qa.dot(qb) < 0.0 {
            qb = Quat::from_xyzw(-qb.x, -qb.y, -qb.z, -qb.w);
        }
        let q = qa.slerp(qb, alpha).normalize_or_identity();
        let scale = match (a.scale, b.scale) {
            (Some(a), Some(b)) => {
                let scale = Vec3::new(a[0], a[1], a[2]).lerp(Vec3::new(b[0], b[1], b[2]), alpha);
                Some([scale.x, scale.y, scale.z])
            }
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        out.push(JointLocalPose {
            translation: [translation.x, translation.y, translation.z],
            rotation: [q.x, q.y, q.z, q.w],
            scale,
        });
    }
    Ok(())
}

fn split_animation_ref(reference: &str) -> Result<(String, Option<String>), String> {
    let normalized = reference.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err("empty animation reference".to_owned());
    }
    let (path, selector) = normalized
        .rsplit_once('@')
        .map(|(path, selector)| {
            let selector = selector.trim();
            (
                path.to_owned(),
                (!selector.is_empty()).then(|| selector.to_owned()),
            )
        })
        .unwrap_or_else(|| (normalized.clone(), None));
    if !path.to_ascii_lowercase().ends_with(".ycd") {
        return Err(format!(
            "player animation must reference .ycd asset: '{reference}'"
        ));
    }
    Ok((path, selector))
}

fn load_animation_clip(reference: &str) -> Result<AnimationClip, String> {
    let (path, selector) = split_animation_ref(reference)?;
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let payload = assets
        .decode_v1(&AssetDecodeRequest {
            logical_path: path.clone(),
            output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
            selector: serde_json::Value::Null,
        })
        .map_err(|error| {
            format!(
                "player animation asset decode failed ref='{reference}' path='{path}' err='{error}'"
            )
        })?;
    decode_ycd_body(&payload, selector.as_deref()).map_err(|error| {
        format!("player animation YCD decode failed ref='{reference}' err='{error}'")
    })
}

fn validate_animation_clip(
    clip_ref: &str,
    clip: &AnimationClip,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: &ModelSkeletonMetadata,
) -> Result<(), String> {
    if !clip.skeleton_ref.trim().is_empty()
        && !clip
            .skeleton_ref
            .eq_ignore_ascii_case(assignment.skeleton_source.as_deref().unwrap_or_default())
    {
        return Err(format!(
            "player animation skeleton ref mismatch clip='{}' assignment='{}'",
            clip.skeleton_ref,
            assignment.skeleton_source.as_deref().unwrap_or("<none>")
        ));
    }
    for (clip_index, &tag) in clip.joint_tags.iter().enumerate() {
        if clip.joint_tags[..clip_index].contains(&tag) {
            return Err(format!(
                "player animation contains duplicate skeleton tag ref='{}' tag={}",
                clip_ref, tag
            ));
        }
        let dense = tag as usize;
        let present = dense < skeleton.joints.len() && skeleton.joints[dense].tag == tag
            || skeleton.joints.iter().any(|joint| joint.tag == tag);
        if !present {
            return Err(format!(
                "player animation skeleton tag is absent ref='{}' clip_index={} tag={} skeleton_joints={}",
                clip_ref,
                clip_index,
                tag,
                skeleton.joints.len()
            ));
        }
    }
    Ok(())
}

fn load_runtime_animation_clip(
    reference: &str,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: &ModelSkeletonMetadata,
) -> Result<PlayerAnimationRuntimeClip, String> {
    let clip = load_animation_clip(reference)?;
    validate_animation_clip(reference, &clip, assignment, skeleton)?;
    Ok(PlayerAnimationRuntimeClip {
        clip_ref: reference.to_owned(),
        clip,
    })
}

fn find_required_braid_joint(
    skeleton: &ModelSkeletonMetadata,
    binary_name: &str,
    imported_candidates: &[&str],
) -> Result<usize, String> {
    imported_candidates
        .iter()
        .find_map(|name| skeleton.joints.iter().position(|joint| joint.name == *name))
        .ok_or_else(|| {
            format!(
                "Abby braid binary driver '{binary_name}' has no imported-rig mapping candidates={imported_candidates:?}"
            )
        })
}

fn prepare_abby_braid_soft_body(
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    parts: &[PlayerRuntimeModelPart],
    skeleton: &ModelSkeletonMetadata,
    bind_joint_frames: &[Mat4],
) -> Result<Option<AbbyBraidSoftBodyRuntime>, String> {
    let skeleton_joint_count = skeleton.joints.len();
    let has_supplemental_skin = parts.iter().any(|part| {
        part.skin.as_ref().is_some_and(|skin| {
            skin.vertices.iter().any(|vertex| {
                vertex
                    .joints
                    .iter()
                    .chain(vertex.joints_extra.iter())
                    .zip(vertex.weights.iter().chain(vertex.weights_extra.iter()))
                    .any(|(&joint, &weight)| weight > 0.0 && joint as usize >= skeleton_joint_count)
            })
        })
    });

    let mut native_braid_joints = [0usize; ABBY_NATIVE_BRAID_JOINT_NAMES.len()];
    let mut has_native_joint_chain = true;
    for (index, name) in ABBY_NATIVE_BRAID_JOINT_NAMES.iter().copied().enumerate() {
        let Some(joint) = skeleton
            .joints
            .iter()
            .position(|candidate| candidate.name == name)
        else {
            has_native_joint_chain = false;
            break;
        };
        native_braid_joints[index] = joint;
    }
    let has_native_braid_skin = has_native_joint_chain
        && parts.iter().any(|part| {
            part.skin.as_ref().is_some_and(|skin| {
                skin.vertices.iter().any(|vertex| {
                    vertex
                        .joints
                        .iter()
                        .chain(vertex.joints_extra.iter())
                        .zip(vertex.weights.iter().chain(vertex.weights_extra.iter()))
                        .any(|(&joint, &weight)| {
                            weight > 0.0 && native_braid_joints.contains(&(joint as usize))
                        })
                })
            })
        });
    if !has_native_braid_skin && !has_supplemental_skin {
        return Ok(None);
    }

    let normalized = assignment
        .source
        .trim()
        .replace('\\', "/")
        .to_ascii_lowercase();
    if !normalized.contains("/characters/abby/") && !normalized.contains("@abby") {
        return Err(format!(
            "player braid soft-body is only authored for Abby source='{}'",
            assignment.source
        ));
    }

    // Native JOINT_HIERARCHY names are authoritative. Legacy Rigify candidates remain only
    // so archived 709 assets can still be inspected; production Abby resolves the first name.
    let attachment_joint = find_required_braid_joint(
        skeleton,
        "headb (parent of braid_offset)",
        &[
            "headb",
            "DEF-spine.006",
            skeleton.anchors.head.as_str(),
            "head",
        ],
    )?;
    let rig = AbbyBraidCollisionRig {
        attachment_joint,
        head_joint: attachment_joint,
        head_base_joint: find_required_braid_joint(
            skeleton,
            "heada",
            &["heada", "DEF-spine.005", "neck", "DEF-spine.004"],
        )?,
        upper_back_joint: find_required_braid_joint(
            skeleton,
            "spined",
            &["spined", "DEF-spine.003", "spine_fk.003", "DEF-spine.004"],
        )?,
        middle_back_joint: find_required_braid_joint(
            skeleton,
            "spinec",
            &["spinec", "DEF-spine.002", "spine_fk.002", "DEF-spine.003"],
        )?,
        lower_back_joint: find_required_braid_joint(
            skeleton,
            "spineb",
            &["spineb", "DEF-spine.001", "spine_fk.001", "DEF-spine.002"],
        )?,
        left_shoulder_joint: find_required_braid_joint(
            skeleton,
            "l_shoulder",
            &[
                "l_shoulder",
                "DEF-upper_arm.L",
                "ORG-upper_arm.L",
                "upper_arm.L",
            ],
        )?,
        right_shoulder_joint: find_required_braid_joint(
            skeleton,
            "r_shoulder",
            &[
                "r_shoulder",
                "DEF-upper_arm.R",
                "ORG-upper_arm.R",
                "upper_arm.R",
            ],
        )?,
    };
    let palette_target = if has_native_braid_skin {
        AbbyBraidPaletteTarget::Native8 {
            joints: native_braid_joints,
            bind_points: [Vec3::ZERO; 8],
        }
    } else {
        AbbyBraidPaletteTarget::Supplemental18
    };
    Ok(Some(AbbyBraidSoftBodyRuntime::new(
        rig,
        bind_joint_frames,
        palette_target,
    )?))
}

pub(super) fn prepare_player_animation_binding(
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    parts: &[PlayerRuntimeModelPart],
    skeleton: Option<&ModelSkeletonMetadata>,
) -> Result<Option<PlayerAnimationRuntimeBinding>, String> {
    use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;

    let skinned_parts = parts
        .iter()
        .filter_map(|part| part.skin.as_ref())
        .collect::<Vec<_>>();
    if skinned_parts.is_empty() {
        return Ok(None);
    }
    let skeleton = skeleton
        .ok_or_else(|| "skinned player model requires authored skeleton metadata".to_owned())?;
    let source_to_model = skinned_parts[0].source_to_model;
    for (part_index, skin) in skinned_parts.iter().enumerate() {
        if skin.source_to_model != source_to_model {
            return Err(format!(
                "skinned player model source-space transform mismatch part={part_index}"
            ));
        }
    }

    let Some(idle_ref) = assignment.idle_animation.as_deref() else {
        return Ok(None);
    };
    let mut clips: [Option<PlayerAnimationRuntimeClip>; 8] =
        [None, None, None, None, None, None, None, None];
    clips[locomotion_slot(L::Idle)] =
        Some(load_runtime_animation_clip(idle_ref, assignment, skeleton)?);

    for (state, reference) in [
        (L::Walk, assignment.walk_animation.as_deref()),
        (L::Run, assignment.run_animation.as_deref()),
        (L::Sprint, assignment.sprint_animation.as_deref()),
        (L::CrouchIdle, assignment.crouch_idle_animation.as_deref()),
        (L::CrouchWalk, assignment.crouch_walk_animation.as_deref()),
        (L::Jump, assignment.jump_animation.as_deref()),
        (L::Fall, assignment.fall_animation.as_deref()),
    ] {
        if let Some(reference) = reference {
            clips[locomotion_slot(state)] = Some(load_runtime_animation_clip(
                reference, assignment, skeleton,
            )?);
        }
    }

    let idle = clips[locomotion_slot(L::Idle)]
        .as_ref()
        .expect("idle clip was inserted above");
    let helper_mirror_pairs = build_helper_mirror_pairs(skeleton);
    let head_follow = build_detached_head_follow(skeleton);
    let eye_contract = build_abby_eye_runtime_contract(skeleton);
    let bind_locals = skeleton
        .joints
        .iter()
        .map(|joint| JointLocalPose {
            translation: joint.position_ls,
            rotation: joint.rotation_ls,
            scale: Some(joint.scale_ls),
        })
        .collect::<Vec<_>>();
    let mut bind_joint_frames = Vec::with_capacity(skeleton.joints.len());
    build_model_joint_frames_from_local_pose(
        skeleton,
        source_to_model,
        &bind_locals,
        &mut bind_joint_frames,
    )?;
    let mut current_locals = Vec::with_capacity(skeleton.joints.len());
    idle.clip
        .sample_local_pose_for_skeleton(0.0, skeleton, &mut current_locals)?;
    synchronize_helper_pose(&helper_mirror_pairs, &mut current_locals);
    stabilize_abby_eye_locals(eye_contract.as_ref(), skeleton, &mut current_locals)?;
    let mut palette_scratch = Vec::with_capacity(skeleton.joints.len());
    build_skin_palette_from_local_pose(
        skeleton,
        source_to_model,
        &current_locals,
        &mut palette_scratch,
    )?;
    apply_detached_head_follow_palette(head_follow.as_ref(), &mut palette_scratch)?;
    validate_abby_eye_palette(eye_contract.as_ref(), &palette_scratch)?;
    debug_dump_abby_eye_matrices(
        eye_contract.as_ref(),
        &bind_joint_frames,
        &current_locals,
        &palette_scratch,
        "initial",
    );
    let braid_soft_body =
        prepare_abby_braid_soft_body(assignment, parts, skeleton, &bind_joint_frames)?;
    let normalized_assignment_source = assignment
        .source
        .trim()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let is_abby = normalized_assignment_source.contains("/characters/abby/")
        || normalized_assignment_source.contains("@abby");
    let equipped_rifle_basepose = if is_abby {
        Some(load_runtime_animation_clip(
            ABBY_RIFLE_BASEPOSE_REF,
            assignment,
            skeleton,
        )?)
    } else {
        None
    };
    let equipped_rifle_reload = if is_abby {
        Some(load_runtime_animation_clip(
            ABBY_RIFLE_RELOAD_REF,
            assignment,
            skeleton,
        )?)
    } else {
        None
    };
    let rifle_ik = build_abby_rifle_ik_rig(skeleton);
    if let Some(braid) = braid_soft_body.as_ref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: Abby braid soft-body ready mode='{}' particles={} body_collision='8 authored capsules + torso OBB + swept CCD' palette_policy='native joints stay inside skeleton palette'",
            braid.mode_label(),
            ABBY_BRAID_CLOTH_PARTICLE_COUNT,
        );
    }
    let joint_frames_scratch = Vec::with_capacity(skeleton.joints.len());
    let sampled_target_locals = current_locals.clone();
    let transition_from_locals = current_locals.clone();
    if !helper_mirror_pairs.is_empty() {
        newengine_ulog_api::ulog::info!(
            "game-ready: mirrored North Star helper rig channels={} policy='primary local pose -> *_helper deform branch before skin palette'",
            helper_mirror_pairs.len()
        );
    }
    if let Some(rig) = head_follow.as_ref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: restored Abby canonical head-space headb_driver={} control_followers={} face_followers={} policy='original headb/DEF-spine.006 deformation -> detached controls + face/eyes; scalp/hair/braid share one authority'",
            rig.headb_driver,
            rig.control_followers.len(),
            rig.face_followers.len(),
        );
    }
    if let Some(eyes) = eye_contract.as_ref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: Abby native eye contract left={} right={} parent={} policy='body locomotion keeps authored eye-local bind; eye palette must equal headb deformation until EyeLookController owns eye-local rotation'",
            eyes.left,
            eyes.right,
            eyes.parent,
        );
    }

    Ok(Some(PlayerAnimationRuntimeBinding {
        clips,
        active_state: L::Idle,
        active_slot: locomotion_slot(L::Idle),
        skeleton: skeleton.clone(),
        source_to_model,
        time_seconds: 0.0,
        current_locals,
        sampled_target_locals,
        transition_from_locals,
        palette_scratch,
        bind_joint_frames,
        joint_frames_scratch,
        helper_mirror_pairs,
        eye_contract,
        head_follow,
        braid_soft_body,
        equipped_rifle_basepose,
        equipped_rifle_reload,
        equipment_overlay_locals: bind_locals,
        rifle_ik,
    }))
}

/// Current gameplay view direction converted into avatar/model-local space. Full-body first
/// person and explicit third-person aim use this for both rendered rifle and arm IK, so the weapon
/// and visible hands cannot diverge from the gameplay view axis.
pub(crate) fn player_rifle_view_forward_model(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<Vec3> {
    let visual_root = world
        .get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)?
        .visual_root
        .filter(|entity| world.exists(*entity))?;
    let (_, visual_rotation) =
        newengine_transform::read_entity_world_pose_local_chain(world, visual_root)?;

    let active_camera = world
        .resource::<newengine_scene::SceneState>()
        .and_then(|state| state.active_camera.or(state.root));
    let camera_rot_offset = active_camera
        .and_then(|camera| world.get::<newengine_sim::FollowTargetCameraController>(camera))
        .filter(|controller| controller.target == player)
        .map(|controller| controller.rot_offset)
        .unwrap_or(Quat::IDENTITY)
        .normalize_or_identity();
    let view_rotation = world
        .get::<newengine_sim::CharacterMotor>(player)
        .map(|motor| {
            (Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0) * camera_rot_offset)
                .normalize_or_identity()
        })
        .or_else(|| {
            active_camera
                .and_then(|camera| world.get::<newengine_sim::CameraRigComp>(camera))
                .map(|rig| rig.0.rotation.normalize_or_identity())
        })?;
    let forward_ws = (view_rotation * -Vec3::Z).normalize_or_zero();
    let forward_model = visual_rotation.normalize_or_identity().inverse() * forward_ws;
    (forward_model.is_finite() && forward_model.length_squared() > 1.0e-8)
        .then_some(forward_model.normalize())
}

fn player_prop_frame(
    world: &newengine_ecs::World,
    player: EntityId,
    candidates: &[&str],
) -> Option<Mat4> {
    let binding = world.get::<PlayerAnimationRuntimeBinding>(player)?;
    for candidate in candidates {
        let Some(index) = binding
            .skeleton
            .joints
            .iter()
            .position(|joint| joint.name == *candidate)
        else {
            continue;
        };
        if let Some(frame) = binding.joint_frames_scratch.get(index).copied() {
            return Some(frame);
        }
        if let Some(frame) = binding.bind_joint_frames.get(index).copied() {
            return Some(frame);
        }
    }
    None
}

const MAX_PROP_SOCKET_TO_HAND_DISTANCE: f32 = 0.12;

fn stable_hand_grip_frame(
    world: &newengine_ecs::World,
    player: EntityId,
    prop_candidates: &[&str],
    physical_candidates: &[&str],
) -> Option<Mat4> {
    let physical = player_prop_frame(world, player, physical_candidates)?;
    let Some(prop) = player_prop_frame(world, player, prop_candidates) else {
        return Some(physical);
    };
    let prop_position = prop.transform_point3(Vec3::ZERO);
    let physical_position = physical.transform_point3(Vec3::ZERO);
    let delta = prop_position - physical_position;
    if delta.is_finite() && delta.length_squared() <= MAX_PROP_SOCKET_TO_HAND_DISTANCE.powi(2) {
        Some(prop)
    } else {
        // Naughty Dog prop-attachment joints can be animation/constraint targets rather than
        // literal palm centers. A stale target may move far away from the hand; never drag an
        // equipped weapon there. Fall back to the animated palm/wrist frame.
        Some(physical)
    }
}

/// Physical right-hand master frame for held weapons. Constraint/prop targets are forbidden.
pub(crate) fn player_right_hand_weapon_frame(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<Mat4> {
    player_prop_frame(
        world,
        player,
        &["r_palm", "r_wrist", "DEF-hand.R", "hand.R"],
    )
}

/// Physical left-hand frame used for support diagnostics. Weapon transform never depends on it.
pub(crate) fn player_left_hand_weapon_frame(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<Mat4> {
    player_prop_frame(
        world,
        player,
        &["l_palm", "l_wrist", "DEF-hand.L", "hand.L"],
    )
}

/// Anatomical frames used by third-person rifle ReadyHold. The solve contract deliberately needs
/// both shoulders: Naughty Dog `spined` axes are not body-forward/body-up, so a stable body frame
/// is reconstructed from the shoulder line instead of trusting the spine joint basis.
pub(crate) fn player_rifle_ready_body_frames(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<(Mat4, Mat4, Mat4)> {
    let chest = player_prop_frame(
        world,
        player,
        &["spined", "DEF-spine.003", "spine_fk.003", "DEF-spine.004"],
    )?;
    let right_shoulder = player_prop_frame(
        world,
        player,
        &["r_shoulder", "DEF-upper_arm.R", "upper_arm.R"],
    )?;
    let left_shoulder = player_prop_frame(
        world,
        player,
        &["l_shoulder", "DEF-upper_arm.L", "upper_arm.L"],
    )?;
    Some((chest, right_shoulder, left_shoulder))
}

/// Stable right-hand weapon grip in player-model local space.
pub(crate) fn player_right_hand_prop_frame(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<Mat4> {
    stable_hand_grip_frame(
        world,
        player,
        &["r_hand_prop_attachment", "r_hand_prop"],
        &["r_palm", "r_wrist", "DEF-hand.R", "hand.R"],
    )
}

pub(crate) fn publish_player_first_person_camera_anchors(world: &mut newengine_ecs::World) {
    const EYE_FORWARD_CLEARANCE_M: f32 = 0.055;
    let players = world
        .query::<PlayerAnimationRuntimeBinding>()
        .map(|(player, _)| player)
        .collect::<Vec<_>>();

    for player in players {
        let eye_center_model = {
            let Some(binding) = world.get::<PlayerAnimationRuntimeBinding>(player) else {
                continue;
            };
            if let Some(eyes) = binding.eye_contract.as_ref() {
                let frame_at = |index: usize| {
                    binding
                        .joint_frames_scratch
                        .get(index)
                        .copied()
                        .or_else(|| binding.bind_joint_frames.get(index).copied())
                };
                match (frame_at(eyes.left), frame_at(eyes.right)) {
                    (Some(left), Some(right)) => {
                        let left = left.transform_point3(Vec3::ZERO);
                        let right = right.transform_point3(Vec3::ZERO);
                        ((left + right) * 0.5)
                            .is_finite()
                            .then_some((left + right) * 0.5)
                    }
                    _ => None,
                }
            } else {
                let anchor = binding.skeleton.anchors.eye.as_str();
                let frame = binding
                    .skeleton
                    .joints
                    .iter()
                    .position(|joint| joint.name == anchor)
                    .and_then(|index| {
                        binding
                            .joint_frames_scratch
                            .get(index)
                            .copied()
                            .or_else(|| binding.bind_joint_frames.get(index).copied())
                    });
                frame
                    .map(|frame| frame.transform_point3(Vec3::ZERO))
                    .filter(|position| position.is_finite())
            }
        };
        let Some(eye_center_model) = eye_center_model else {
            continue;
        };
        let Some(visual_root) = world
            .get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
            .and_then(|binding| binding.visual_root)
            .filter(|entity| world.exists(*entity))
        else {
            continue;
        };
        let Some((visual_position, visual_rotation)) =
            newengine_transform::read_entity_world_pose_local_chain(world, visual_root)
        else {
            continue;
        };
        let eye_center_ws =
            visual_position + visual_rotation.normalize_or_identity() * eye_center_model;
        if !eye_center_ws.is_finite() {
            continue;
        }
        let _ = world.insert(
            player,
            newengine_engine_runtime::gameplay::PlayerFirstPersonCameraAnchor {
                eye_center_ws,
                forward_clearance: EYE_FORWARD_CLEARANCE_M,
            },
        );
    }
}

pub(crate) fn tick_player_skin_animation(world: &mut newengine_ecs::World, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let players = world
        .query::<PlayerAnimationRuntimeBinding>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();

    for player in players {
        let animation_state = world
            .get::<newengine_engine_runtime::gameplay::PlayerAnimationState>(player)
            .copied()
            .unwrap_or_default();
        let world_velocity = world
            .get::<newengine_sim::Velocity>(player)
            .copied()
            .unwrap_or_default()
            .0;
        let root_transform = world.get::<Transform>(player).copied().unwrap_or_default();
        let root_velocity_local = root_transform.rotation.inverse() * world_velocity;
        let root_position = root_transform.position;
        let root_rotation = root_transform.rotation;
        let rifle_aim_alpha = super::equipment_visual::equipped_rifle_aim_alpha(world, player);
        let rifle_recoil_alpha = super::equipment_visual::equipped_rifle_recoil_alpha(world, player);
        let first_person_active = world
            .resource::<newengine_engine_runtime::gameplay::PlayerViewState>()
            .copied()
            .unwrap_or_default()
            .first_person_active;
        let rifle_view_forward_model = if first_person_active || rifle_aim_alpha > 0.001 {
            player_rifle_view_forward_model(world, player)
        } else {
            None
        };
        let has_equipped_rifle = world
            .get::<newengine_engine_runtime::gameplay::EquippedWeaponBinding>(player)
            .and_then(|binding| {
                world
                    .resource::<newengine_engine_runtime::gameplay::ItemCatalog>()
                    .and_then(|catalog| catalog.get(binding.item))
            })
            .and_then(|definition| definition.world.model_ref.as_deref())
            .is_some_and(|model_ref| {
                model_ref.eq_ignore_ascii_case(crate::weapon_grip::RIFLE_MODEL_REF)
            });
        let rifle_reload_progress = if has_equipped_rifle {
            world
                .get::<newengine_engine_runtime::gameplay::PlayerWeaponState>(player)
                .and_then(|state| {
                    (state.reload_remaining > 0.0).then(|| {
                        let duration = world
                            .get::<newengine_engine_runtime::gameplay::HitscanWeaponTuning>(player)
                            .map(|tuning| tuning.sanitized().reload_duration)
                            .filter(|duration| *duration > 1.0e-4)
                            .unwrap_or(2.0);
                        (1.0 - state.reload_remaining / duration).clamp(0.0, 1.0)
                    })
                })
        } else {
            None
        };
        let (palette, clip_ref, active_state) = {
            let Some(binding) = world.get_mut::<PlayerAnimationRuntimeBinding>(player) else {
                continue;
            };
            let desired_slot = binding.resolve_slot(animation_state.locomotion);
            let state_changed = binding.active_state != animation_state.locomotion;
            let slot_changed = binding.active_slot != desired_slot;
            let transitioned = state_changed || slot_changed;
            if slot_changed {
                // Cross-fade from the pose that was actually visible, not merely from
                // the previous clip. This keeps hands/forearms continuous even if the
                // player changes locomotion state again before the prior fade finishes.
                binding
                    .transition_from_locals
                    .clone_from(&binding.current_locals);
                binding.active_slot = desired_slot;
                binding.time_seconds = 0.0;
            }
            if state_changed {
                // A semantic transition is not necessarily a clip transition. Fall can
                // intentionally resolve to the active Jump slot when no authored fall
                // clip exists. Preserve playback time in that case so the airborne
                // phase continues through the apex instead of restarting the jump.
                binding.active_state = animation_state.locomotion;
            }
            if !slot_changed {
                let playback_rate = match animation_state.locomotion {
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Walk => {
                        (animation_state.normalized_speed / 0.40).clamp(0.65, 1.45)
                    }
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Run => {
                        (animation_state.normalized_speed / 0.85).clamp(0.75, 1.45)
                    }
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Sprint => {
                        animation_state.normalized_speed.clamp(1.0, 1.65)
                    }
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::CrouchWalk => {
                        // Authored crouch speed is ~1.0 m/s while normalized_speed is expressed
                        // against the 3.0 m/s run speed. Keep foot cadence centered at 1x at
                        // full crouch speed and only stretch modestly near the movement threshold.
                        (animation_state.normalized_speed / 0.333_333_34).clamp(0.70, 1.25)
                    }
                    _ => 1.0,
                };
                binding.time_seconds += dt * playback_rate;
            }

            let active_slot = binding.active_slot;
            let active_state = binding.active_state;
            let active_clip = binding.clips[active_slot]
                .as_ref()
                .expect("resolved player animation slot must contain a clip");
            let clip_ref = active_clip.clip_ref.clone();
            if transitioned {
                newengine_ulog_api::ulog::info!(
                    "game-ready: player locomotion animation transition player={} state='{}' clip='{}' duration={:.3}s normalized_speed={:.3}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    active_clip.clip.duration_seconds,
                    animation_state.normalized_speed
                );
            }
            if let Err(error) = active_clip.clip.sample_local_pose_for_skeleton(
                binding.time_seconds,
                &binding.skeleton,
                &mut binding.sampled_target_locals,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: player animation sample failed player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }

            if has_equipped_rifle {
                let (overlay, phase, weights, overlay_ref) = if let Some(progress) = rifle_reload_progress {
                    (
                        binding.equipped_rifle_reload.as_ref(),
                        progress,
                        ABBY_RIFLE_RELOAD_ROTATION_WEIGHTS,
                        ABBY_RIFLE_RELOAD_REF,
                    )
                } else {
                    (
                        binding.equipped_rifle_basepose.as_ref(),
                        ABBY_RIFLE_READY_SAMPLE_PHASE,
                        ABBY_RIFLE_READY_ROTATION_WEIGHTS,
                        ABBY_RIFLE_BASEPOSE_REF,
                    )
                };
                if let Err(error) = apply_equipped_rifle_rotation_overlay(
                    overlay,
                    &binding.skeleton,
                    &mut binding.equipment_overlay_locals,
                    &mut binding.sampled_target_locals,
                    phase,
                    weights,
                ) {
                    newengine_ulog_api::ulog::warn!(
                        "game-ready: Abby rifle upper-body overlay failed player={} ref='{}' phase={:.3}: {}",
                        player.stable_u64(),
                        overlay_ref,
                        phase,
                        error,
                    );
                }
            }
            synchronize_helper_pose(
                &binding.helper_mirror_pairs,
                &mut binding.sampled_target_locals,
            );

            let alpha = if state_changed && !slot_changed {
                // Same-slot semantic continuation (notably Jump -> Fall fallback) must
                // not re-enter a cross-fade against stale transition_from_locals.
                1.0
            } else {
                animation_state.transition_alpha.clamp(0.0, 1.0)
            };
            if alpha < 1.0 {
                if let Err(error) = blend_local_poses(
                    &binding.transition_from_locals,
                    &binding.sampled_target_locals,
                    alpha,
                    &mut binding.current_locals,
                ) {
                    newengine_ulog_api::ulog::warn!(
                        "game-ready: player animation transition failed player={} state='{}' clip='{}': {}",
                        player.stable_u64(),
                        active_state.clip_hint(),
                        clip_ref,
                        error
                    );
                    continue;
                }
            } else {
                binding
                    .current_locals
                    .clone_from(&binding.sampled_target_locals);
            }

            if has_equipped_rifle {
                match apply_equipped_rifle_support_ik(
                    binding.rifle_ik.as_ref(),
                    &binding.skeleton,
                    binding.source_to_model,
                    &mut binding.current_locals,
                    &mut binding.joint_frames_scratch,
                    rifle_view_forward_model,
                    rifle_aim_alpha,
                    rifle_recoil_alpha,
                    rifle_reload_progress.is_none(),
                ) {
                    Ok(Some(error)) if error > 0.025 => {
                        newengine_ulog_api::ulog::warn!(
                            "game-ready: Abby rifle support IK residual player={} error_m={:.5}",
                            player.stable_u64(),
                            error,
                        );
                    }
                    Ok(_) => {}
                    Err(error) => {
                        newengine_ulog_api::ulog::warn!(
                            "game-ready: Abby rifle support IK failed player={}: {}",
                            player.stable_u64(),
                            error,
                        );
                    }
                }
            }
            synchronize_helper_pose(&binding.helper_mirror_pairs, &mut binding.current_locals);
            if let Err(error) = stabilize_abby_eye_locals(
                binding.eye_contract.as_ref(),
                &binding.skeleton,
                &mut binding.current_locals,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: Abby eye-local stabilization failed player={} clip='{}': {}",
                    player.stable_u64(),
                    clip_ref,
                    error
                );
                continue;
            }

            if let Err(error) = build_skin_palette_from_local_pose(
                &binding.skeleton,
                binding.source_to_model,
                &binding.current_locals,
                &mut binding.palette_scratch,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: player skin palette update failed player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }
            if let Err(error) = apply_detached_head_follow_palette(
                binding.head_follow.as_ref(),
                &mut binding.palette_scratch,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: detached face/head follow projection failed player={} clip='{}': {}",
                    player.stable_u64(),
                    clip_ref,
                    error
                );
                continue;
            }
            if let Err(error) =
                validate_abby_eye_palette(binding.eye_contract.as_ref(), &binding.palette_scratch)
            {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: Abby eye palette rejected player={} clip='{}': {}",
                    player.stable_u64(),
                    clip_ref,
                    error
                );
                continue;
            }
            if transitioned {
                debug_dump_abby_eye_matrices(
                    binding.eye_contract.as_ref(),
                    &binding.bind_joint_frames,
                    &binding.current_locals,
                    &binding.palette_scratch,
                    &format!("transition:{clip_ref}"),
                );
            }
            binding.joint_frames_scratch.clear();
            binding
                .joint_frames_scratch
                .reserve(binding.skeleton.joints.len());
            for index in 0..binding.skeleton.joints.len() {
                // Skin palette is a deformation matrix. Multiplying it by the authored bind
                // frame reconstructs the absolute current-frame joint transform after all
                // animation/head-follow corrections: P * (S*B) = S*A.
                let frame = binding.palette_scratch[index] * binding.bind_joint_frames[index];
                binding.joint_frames_scratch.push(frame);
            }
            let (braid_soft_body, joint_frames_scratch, palette_scratch) = (
                &mut binding.braid_soft_body,
                &binding.joint_frames_scratch,
                &mut binding.palette_scratch,
            );
            if let Some(braid) = braid_soft_body.as_mut() {
                if let Err(error) = braid.tick_and_append(
                    dt,
                    root_velocity_local,
                    root_position,
                    root_rotation,
                    joint_frames_scratch,
                    palette_scratch,
                ) {
                    newengine_ulog_api::ulog::warn!(
                        "game-ready: Abby braid soft-body update failed player={} clip='{}': {}",
                        player.stable_u64(),
                        clip_ref,
                        error
                    );
                    continue;
                }
            }
            let expected_palette_joints = binding.skeleton.joints.len()
                + binding
                    .braid_soft_body
                    .as_ref()
                    .map(AbbyBraidSoftBodyRuntime::supplemental_palette_joint_count)
                    .unwrap_or(0);
            if let Err(error) = super::validation::validate_player_palette(
                &binding.palette_scratch,
                expected_palette_joints,
                &format!("animated clip {clip_ref}"),
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: unstable player skin palette rejected player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }
            (binding.palette_scratch.clone(), clip_ref, active_state)
        };

        if let Some(pose) =
            world.get_mut::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
        {
            pose.palette = palette;
            pose.revision = pose.revision.saturating_add(1).max(1);
        } else {
            let _ = world.insert(
                player,
                newengine_engine_runtime::gameplay::PlayerSkinPose {
                    palette,
                    revision: 1,
                },
            );
        }
        if dt > 0.0
            && world
                .get::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
                .is_some_and(|pose| pose.revision == 2)
        {
            newengine_ulog_api::ulog::info!(
                "game-ready: first animated player palette committed player={} state='{}' clip='{}'",
                player.stable_u64(),
                active_state.clip_hint(),
                clip_ref
            );
        }
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;

    #[test]
    fn rifle_ready_pole_ik_converges_without_moving_stock_anchored_weapon() {
        use newengine_model_skeleton_api::{ModelSkeletonAnchors, ModelSkeletonJointMetadata};

        let names = [
            "root",
            "spined",
            "r_shoulder",
            "r_elbow",
            "r_wrist",
            "r_palm",
            "l_shoulder",
            "l_elbow",
            "l_wrist",
            "l_palm",
        ];
        let joint = |index: u32, parent_index: Option<u32>, position_ls: [f32; 3]| {
            ModelSkeletonJointMetadata {
                index,
                tag: index,
                name: names[index as usize].to_owned(),
                parent: parent_index.map(|parent| names[parent as usize].to_owned()),
                parent_index,
                position_ls,
                rotation_ls: [0.0, 0.0, 0.0, 1.0],
                scale_ls: [1.0, 1.0, 1.0],
                flags: Vec::new(),
            }
        };
        let skeleton = ModelSkeletonMetadata {
            source: "test".to_owned(),
            source_format: "test".to_owned(),
            container_magic: "TEST".to_owned(),
            byte_len: 0,
            content_hash: String::new(),
            decode_status: "ok".to_owned(),
            joints: vec![
                joint(0, None, [0.0, 0.0, 0.0]),
                joint(1, Some(0), [0.0, 1.285_745, 0.0]),
                joint(2, Some(1), [-0.17, 0.06, 0.0]),
                // Real Abby arm lengths are roughly 0.26 m upper arm and 0.25 m forearm/hand.
                joint(3, Some(2), [0.0, -0.26, 0.0]),
                joint(4, Some(3), [0.0, -0.24, 0.0]),
                joint(5, Some(4), [0.0, -0.015, 0.0]),
                joint(6, Some(1), [0.17, 0.06, 0.0]),
                joint(7, Some(6), [0.0, -0.26, 0.0]),
                joint(8, Some(7), [0.0, -0.24, 0.0]),
                joint(9, Some(8), [0.0, -0.015, 0.0]),
            ],
            anchors: ModelSkeletonAnchors {
                root: "root".to_owned(),
                hips: "root".to_owned(),
                head: "spined".to_owned(),
                left_hand: "l_palm".to_owned(),
                right_hand: "r_palm".to_owned(),
                left_foot: "root".to_owned(),
                right_foot: "root".to_owned(),
                eye: "spined".to_owned(),
                eye_height: 0.0,
            },
        };
        let mut pose = skeleton
            .joints
            .iter()
            .map(|joint| JointLocalPose {
                translation: joint.position_ls,
                rotation: joint.rotation_ls,
                scale: Some(joint.scale_ls),
            })
            .collect::<Vec<_>>();
        let rig = build_abby_rifle_ik_rig(&skeleton).expect("rifle IK rig");
        let source_to_model = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let mut frames = Vec::new();
        rebuild_model_joint_frames(&skeleton, source_to_model, &pose, &mut frames)
            .expect("initial frames");
        let contract_before = crate::weapon_grip::rifle_ready_solve_contract(
            frames[rig.chest],
            frames[rig.right_shoulder],
            frames[rig.left_shoulder],
        )
        .expect("ReadyHold solve contract");
        let root_before = contract_before.root;
        let right_target = crate::weapon_grip::rifle_ready_right_palm_position(root_before);
        let left_target = crate::weapon_grip::rifle_ready_left_palm_position(root_before);
        let initial_error = (
            (frames[rig.right_palm].transform_point3(Vec3::ZERO) - right_target).length(),
            (frames[rig.left_palm].transform_point3(Vec3::ZERO) - left_target).length(),
        );

        let final_error = apply_equipped_rifle_support_ik(
            Some(&rig),
            &skeleton,
            source_to_model,
            &mut pose,
            &mut frames,
            None,
            0.0,
            0.0,
            true,
        )
        .expect("bilateral ReadyHold IK")
        .expect("IK enabled");

        let final_right =
            (frames[rig.right_palm].transform_point3(Vec3::ZERO) - right_target).length();
        let final_left =
            (frames[rig.left_palm].transform_point3(Vec3::ZERO) - left_target).length();
        assert!(
            final_right < initial_error.0,
            "right initial={} final={final_right}",
            initial_error.0
        );
        assert!(
            final_left < initial_error.1,
            "left initial={} final={final_left}",
            initial_error.1
        );
        assert!(final_error < 0.035, "final={final_error}");

        let contract_after = crate::weapon_grip::rifle_ready_solve_contract(
            frames[rig.chest],
            frames[rig.right_shoulder],
            frames[rig.left_shoulder],
        )
        .expect("ReadyHold solve contract after IK");
        let root_after = contract_after.root;
        assert!((root_before.position - root_after.position).length() < 1.0e-6);
        assert!(root_before.rotation.dot(root_after.rotation).abs() > 0.999_999);
        assert!((contract_after.stock_contact - contract_after.shoulder_pocket).length() < 1.0e-6);
    }

    #[test]
    fn rifle_ready_overlay_is_rotation_only_upper_body_and_uses_stable_aim_phase() {
        let names = ABBY_RIFLE_READY_ROTATION_WEIGHTS
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        assert!(names.contains(&"spinec"));
        assert!(names.contains(&"spined"));
        assert!(names.contains(&"l_wrist"));
        assert!(names.contains(&"r_palm"));
        assert!(!names.contains(&"r_hand_prop_attachment"));
        assert!(!names.contains(&"l_hand_prop"));
        assert!(!names.contains(&"pelvis"));
        assert!(!names.contains(&"spinea"));
        assert!(!names.contains(&"headb"));
        assert!((ABBY_RIFLE_READY_SAMPLE_PHASE - 0.48).abs() < 1.0e-6);
        assert!(ABBY_RIFLE_BASEPOSE_REF.contains("rear-aim-00bw-aim"));
        assert!(!ABBY_RIFLE_BASEPOSE_REF.contains("reload"));
    }

    #[test]
    fn detached_control_and_face_share_the_same_canonical_headb_delta() {
        let rig = DetachedHeadFollowRig {
            headb_driver: 0,
            control_followers: vec![1],
            face_followers: vec![2],
        };
        let mut palette = vec![Mat4::IDENTITY; 3];
        palette[0] = Mat4::from_translation(Vec3::new(0.2, 0.1, -0.3));
        palette[1] = Mat4::from_translation(Vec3::new(0.0, 0.02, 0.0));
        palette[2] = Mat4::from_translation(Vec3::new(0.0, 0.03, 0.0));

        apply_detached_head_follow_palette(Some(&rig), &mut palette).expect("projection");

        let control = palette[1].transform_point3(Vec3::ZERO);
        assert!((control.x - 0.2).abs() < 1.0e-5);
        assert!((control.y - 0.12).abs() < 1.0e-5);
        assert!((control.z + 0.3).abs() < 1.0e-5);

        // The face gets headb + its own detached deformation only. It must not
        // receive the MCH control deformation a second time (old result y=0.15).
        let face = palette[2].transform_point3(Vec3::ZERO);
        assert!((face.x - 0.2).abs() < 1.0e-5);
        assert!((face.y - 0.13).abs() < 1.0e-5);
        assert!((face.z + 0.3).abs() < 1.0e-5);
    }

    #[test]
    fn native_abby_eye_palette_enforces_parent_deformation_invariant() {
        let contract = AbbyEyeRuntimeContract {
            parent: 0,
            left: 1,
            right: 2,
        };
        let head_delta = Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 1.0, 1.0),
            Quat::from_rotation_y(0.25),
            Vec3::new(0.2, 0.1, -0.3),
        );
        let mut palette = vec![head_delta, head_delta, head_delta];
        validate_abby_eye_palette(Some(&contract), &palette).expect("stable eyes");

        palette[contract.left] = Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 1.0, 1.0),
            Quat::from_rotation_x(0.08),
            Vec3::ZERO,
        ) * palette[contract.left];
        let error = validate_abby_eye_palette(Some(&contract), &palette)
            .expect_err("extra eye deformation must be rejected");
        assert!(error.contains("eye palette drift"));
    }

    fn test_braid_rig() -> AbbyBraidCollisionRig {
        AbbyBraidCollisionRig {
            attachment_joint: 0,
            head_joint: 1,
            head_base_joint: 2,
            upper_back_joint: 3,
            middle_back_joint: 6,
            lower_back_joint: 7,
            left_shoulder_joint: 4,
            right_shoulder_joint: 5,
        }
    }

    fn test_braid_bind_frames() -> Vec<Mat4> {
        vec![
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::new(0.0, 8.0, 0.0)),
            Mat4::from_translation(Vec3::new(0.0, 7.8, 0.0)),
            Mat4::from_translation(Vec3::new(0.0, 7.5, 0.0)),
            Mat4::from_translation(Vec3::new(-1.0, 7.5, 0.0)),
            Mat4::from_translation(Vec3::new(1.0, 7.5, 0.0)),
            Mat4::from_translation(Vec3::new(0.0, 7.2, 0.0)),
            Mat4::from_translation(Vec3::new(0.0, 6.9, 0.0)),
        ]
    }

    fn test_braid() -> AbbyBraidSoftBodyRuntime {
        AbbyBraidSoftBodyRuntime::new(
            test_braid_rig(),
            &test_braid_bind_frames(),
            AbbyBraidPaletteTarget::Supplemental18,
        )
        .expect("test braid")
    }

    #[test]
    fn native_abby_braid_keeps_palette_inside_skeleton_joint_count() {
        let frames = test_braid_bind_frames();
        let braid = AbbyBraidSoftBodyRuntime::new(
            test_braid_rig(),
            &frames,
            AbbyBraidPaletteTarget::Native8 {
                joints: [0, 1, 2, 3, 4, 5, 6, 7],
                bind_points: [Vec3::ZERO; 8],
            },
        )
        .expect("native braid");
        assert_eq!(braid.supplemental_palette_joint_count(), 0);
        assert_eq!(braid.mode_label(), "native-joints8");
        let mut palette = vec![Mat4::IDENTITY; frames.len()];
        braid.append_bind_palette(&mut palette);
        assert_eq!(palette.len(), frames.len());
    }

    #[test]
    fn abby_braid_soft_body_appends_eighteen_finite_joint_matrices() {
        let mut braid = test_braid();
        let attachment = Mat4::from_translation(Vec3::new(0.05, 0.02, -0.01));
        let mut joint_frames = test_braid_bind_frames();
        joint_frames[0] = attachment;
        let mut palette = vec![Mat4::IDENTITY; joint_frames.len()];
        braid
            .tick_and_append(
                1.0 / 60.0,
                Vec3::ZERO,
                Vec3::ZERO,
                Quat::IDENTITY,
                &joint_frames,
                &mut palette,
            )
            .expect("soft body");
        assert_eq!(
            palette.len(),
            joint_frames.len() + ABBY_BRAID_SOFT_BODY_JOINTS
        );
        assert!(palette
            .iter()
            .all(|matrix| { matrix.to_cols_array().iter().all(|value| value.is_finite()) }));
        let bind_root = Vec3::new(
            ABBY_BRAID_BIND_POINTS[0][0],
            ABBY_BRAID_BIND_POINTS[0][1],
            ABBY_BRAID_BIND_POINTS[0][2],
        );
        let deformed_root = palette[joint_frames.len()].transform_point3(bind_root);
        let expected_root = attachment.transform_point3(bind_root);
        assert!((deformed_root - expected_root).length() < 1.0e-4);
    }

    #[test]
    fn abby_braid_authored_cloth_preserves_edge_rest_lengths_under_gravity() {
        let mut braid = test_braid();
        let joint_frames = test_braid_bind_frames();
        let mut palette = vec![Mat4::IDENTITY; joint_frames.len()];
        braid
            .tick_and_append(
                1.0 / 60.0,
                Vec3::ZERO,
                Vec3::ZERO,
                Quat::IDENTITY,
                &joint_frames,
                &mut palette,
            )
            .expect("init");
        for _ in 0..120 {
            let mut palette = vec![Mat4::IDENTITY; joint_frames.len()];
            braid
                .tick_and_append(
                    1.0 / 60.0,
                    Vec3::ZERO,
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    &joint_frames,
                    &mut palette,
                )
                .expect("step");
        }
        for &(a, b, rest, _stiffness, _damping) in &ABBY_BRAID_CLOTH_EDGES {
            let actual = (braid.points[b] - braid.points[a]).length();
            assert!(
                (actual - rest).abs() < 0.006,
                "edge=({a},{b}) rest={rest} actual={actual}"
            );
        }
    }

    #[test]
    fn abby_braid_reset_snaps_all_particles_to_current_animated_guide() {
        let mut braid = test_braid();
        let mut joint_frames = test_braid_bind_frames();
        let mut palette = vec![Mat4::IDENTITY; joint_frames.len()];
        braid
            .tick_and_append(
                1.0 / 60.0,
                Vec3::ZERO,
                Vec3::ZERO,
                Quat::IDENTITY,
                &joint_frames,
                &mut palette,
            )
            .expect("init");
        braid.points[10] += Vec3::new(0.4, 0.2, -0.1);
        braid.previous_points[10] -= Vec3::new(0.2, 0.1, 0.0);
        braid.request_reset();
        joint_frames[0] = Mat4::from_translation(Vec3::new(0.25, 0.0, 0.0));
        let expected = braid.guide_from_attachment(&joint_frames).expect("guide").1;
        let mut palette = vec![Mat4::IDENTITY; joint_frames.len()];
        braid
            .tick_and_append(
                1.0 / 60.0,
                Vec3::ZERO,
                Vec3::ZERO,
                Quat::IDENTITY,
                &joint_frames,
                &mut palette,
            )
            .expect("reset");
        for index in 0..ABBY_BRAID_CLOTH_PARTICLE_COUNT {
            assert!((braid.points[index] - expected[index]).length() < 1.0e-6);
            assert!((braid.previous_points[index] - expected[index]).length() < 1.0e-6);
        }
    }

    #[test]
    fn abby_braid_authored_collider_bindings_follow_current_joint_delta() {
        let rig = test_braid_rig();
        let bind_frames = test_braid_bind_frames();
        let bindings = AbbyBraidColliderBindings::from_bind_frames(rig, &bind_frames)
            .expect("authored collider bindings");
        let bind_colliders = bindings
            .from_joint_frames(&bind_frames)
            .expect("bind colliders");
        let mut animated_frames = bind_frames.clone();
        let delta = Vec3::new(0.44, -0.17, 0.09);
        animated_frames[rig.left_shoulder_joint] =
            Mat4::from_translation(delta) * bind_frames[rig.left_shoulder_joint];
        let animated = bindings
            .from_joint_frames(&animated_frames)
            .expect("animated colliders");
        let left_shoulder_capsule = 6;
        assert!(
            (animated.capsules[left_shoulder_capsule].a
                - bind_colliders.capsules[left_shoulder_capsule].a
                - delta)
                .length()
                < 1.0e-5
        );
        assert!(
            (animated.capsules[left_shoulder_capsule].b
                - bind_colliders.capsules[left_shoulder_capsule].b
                - delta)
                .length()
                < 1.0e-5
        );
        assert!(
            (bind_colliders.capsules[left_shoulder_capsule].radius - 0.064_326_786).abs() < 1.0e-7
        );
    }

    #[test]
    fn authored_oriented_box_projects_to_nearest_face() {
        let box_shape = AbbyBraidOrientedBox {
            center: Vec3::ZERO,
            axes: [Vec3::X, Vec3::Y, Vec3::Z],
            half_extents: Vec3::new(1.0, 2.0, 3.0),
        };
        let mut point = Vec3::new(0.9, 0.0, 0.0);
        project_out_of_oriented_box(&mut point, box_shape);
        assert!((point.x - 1.0).abs() < 1.0e-6);
        assert!(point.y.abs() < 1.0e-6);
        assert!(point.z.abs() < 1.0e-6);
    }

    #[test]
    fn braid_capsule_sweep_blocks_fast_back_tunneling() {
        let mut point = Vec3::new(0.0, 0.0, 1.0);
        let previous = Vec3::new(0.0, 0.0, -1.0);
        let normal = sweep_point_against_capsule(
            &mut point,
            previous,
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            0.35,
        )
        .expect("swept contact");
        assert!(point.z < -0.34, "point tunneled through capsule: {point:?}");
        assert!(normal.z < -0.9, "wrong entry normal: {normal:?}");
    }

    #[test]
    fn braid_obb_sweep_blocks_fast_torso_tunneling() {
        let body = AbbyBraidOrientedBox {
            center: Vec3::ZERO,
            axes: [Vec3::X, Vec3::Y, Vec3::Z],
            half_extents: Vec3::new(0.5, 1.0, 0.25),
        };
        let mut point = Vec3::new(0.0, 0.0, 1.0);
        let previous = Vec3::new(0.0, 0.0, -1.0);
        let normal =
            sweep_point_against_oriented_box(&mut point, previous, body).expect("swept contact");
        assert!(point.z < -0.249, "point tunneled through OBB: {point:?}");
        assert!(normal.z < -0.9, "wrong entry normal: {normal:?}");
    }

    #[test]
    fn braid_contact_response_removes_inward_verlet_velocity() {
        let point = Vec3::new(0.0, 0.0, -0.35);
        let mut previous = Vec3::new(0.0, 0.0, -0.8);
        apply_braid_contact_velocity_response(point, &mut previous, Vec3::new(0.0, 0.0, -1.0));
        let velocity = point - previous;
        assert!(
            velocity.dot(Vec3::new(0.0, 0.0, -1.0)) >= -1.0e-6,
            "inward velocity survived: {velocity:?}"
        );
    }

    #[test]
    fn abby_braid_authored_cloth_topology_is_self_consistent() {
        use std::collections::BTreeMap;

        assert_eq!(ABBY_BRAID_CLOTH_BIND_PARTICLES.len(), 32);
        assert_eq!(ABBY_BRAID_CLOTH_TRIANGLES.len(), 30);
        assert_eq!(ABBY_BRAID_CLOTH_EDGES.len(), 61);
        assert_eq!(ABBY_BRAID_CLOTH_BENDS.len(), 29);
        assert_eq!(ABBY_BRAID_CLOTH_ACTIVE_VERTEX_ORDER.len(), 32);
        assert_eq!(ABBY_BRAID_CLOTH_CENTERLINE_PAIRS.len(), 16);

        let bind = ABBY_BRAID_CLOTH_BIND_PARTICLES.map(|p| Vec3::new(p[0], p[1], p[2]));
        for &(a, b, rest, _stiffness, _damping) in &ABBY_BRAID_CLOTH_EDGES {
            let measured = (bind[b] - bind[a]).length();
            assert!(
                (measured - rest).abs() < 1.0e-6,
                "edge=({a},{b}) authored={rest} measured={measured}"
            );
        }
        for &(indices, weights, _geometry_scale, _rest_scalar) in &ABBY_BRAID_CLOTH_BENDS {
            assert!(indices
                .iter()
                .all(|&index| index < ABBY_BRAID_CLOTH_PARTICLE_COUNT));
            assert!(weights.iter().copied().sum::<f32>().abs() < 2.0e-5);
        }

        let pinned = ABBY_BRAID_CLOTH_SCALAR0
            .iter()
            .enumerate()
            .filter_map(|(index, &value)| (value <= 1.0e-8).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(pinned, vec![0, 1, 2, 30]);

        let mut edge_use = BTreeMap::<(usize, usize), usize>::new();
        for triangle in ABBY_BRAID_CLOTH_TRIANGLES {
            for (a, b) in [
                (triangle[0], triangle[1]),
                (triangle[1], triangle[2]),
                (triangle[2], triangle[0]),
            ] {
                let key = if a < b { (a, b) } else { (b, a) };
                *edge_use.entry(key).or_default() += 1;
            }
        }
        assert_eq!(edge_use.len(), 61);
        assert_eq!(edge_use.values().filter(|&&count| count == 2).count(), 29);
    }

    #[test]
    fn abby_braid_cloth_centerline_bridge_is_bind_invariant() {
        let output_guide = ABBY_BRAID_BIND_POINTS.map(|p| Vec3::new(p[0], p[1], p[2]));
        let cloth_guide = ABBY_BRAID_CLOTH_BIND_PARTICLES.map(|p| Vec3::new(p[0], p[1], p[2]));
        let bridged =
            bridge_authored_cloth_to_braid_joints(&output_guide, &cloth_guide, &cloth_guide);
        for index in 0..ABBY_BRAID_SOFT_BODY_JOINTS {
            assert!((bridged[index] - output_guide[index]).length() < 1.0e-7);
        }
    }

    #[test]
    fn local_pose_crossfade_preserves_endpoints_and_shortest_quaternion_path() {
        let from = [JointLocalPose {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: Some([1.0, 1.0, 1.0]),
        }];
        let to = [JointLocalPose {
            translation: [2.0, 4.0, 6.0],
            // Same identity rotation with opposite quaternion sign.
            rotation: [0.0, 0.0, 0.0, -1.0],
            scale: Some([1.0, 1.0, 1.0]),
        }];
        let mut out = Vec::new();
        blend_local_poses(&from, &to, 0.5, &mut out).expect("blend");
        assert_eq!(out.len(), 1);
        assert!((out[0].translation[0] - 1.0).abs() <= 1.0e-6);
        assert!((out[0].translation[1] - 2.0).abs() <= 1.0e-6);
        assert!((out[0].translation[2] - 3.0).abs() <= 1.0e-6);
        assert!(out[0].rotation[0].abs() <= 1.0e-6);
        assert!(out[0].rotation[1].abs() <= 1.0e-6);
        assert!(out[0].rotation[2].abs() <= 1.0e-6);
        assert!((out[0].rotation[3].abs() - 1.0).abs() <= 1.0e-6);
    }
}
