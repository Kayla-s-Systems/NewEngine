use super::*;

use newengine_animation_runtime::{
    build_skin_palette_from_local_pose, decode_ycd_body, AnimationClip, JointLocalPose,
};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient, ASSET_LIST_FILE_BODY_OUTPUT};
use newengine_math::{Mat4, Quat, Vec3};
use newengine_model_skeleton_api::ModelSkeletonMetadata;

#[derive(Clone, Debug)]
struct PlayerAnimationRuntimeClip {
    clip_ref: String,
    clip: AnimationClip,
}

const ABBY_BRAID_SOFT_BODY_JOINTS: usize = 18;
const ABBY_BRAID_PINNED_JOINTS: usize = 3;
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

#[derive(Clone, Debug)]
struct AbbyBraidSoftBodyRuntime {
    head_joint: usize,
    torso_joint: usize,
    points: [Vec3; ABBY_BRAID_SOFT_BODY_JOINTS],
    previous_points: [Vec3; ABBY_BRAID_SOFT_BODY_JOINTS],
    previous_root_velocity_local: Vec3,
    initialized: bool,
}

impl AbbyBraidSoftBodyRuntime {
    fn new(head_joint: usize, torso_joint: usize) -> Self {
        let points = ABBY_BRAID_BIND_POINTS.map(|p| Vec3::new(p[0], p[1], p[2]));
        Self {
            head_joint,
            torso_joint,
            points,
            previous_points: points,
            previous_root_velocity_local: Vec3::ZERO,
            initialized: false,
        }
    }

    fn append_bind_palette(&self, palette: &mut Vec<Mat4>) {
        palette.extend(std::iter::repeat_n(
            Mat4::IDENTITY,
            ABBY_BRAID_SOFT_BODY_JOINTS,
        ));
    }

    fn tick_and_append(
        &mut self,
        dt: f32,
        root_velocity_local: Vec3,
        palette: &mut Vec<Mat4>,
    ) -> Result<(), String> {
        let head = *palette.get(self.head_joint).ok_or_else(|| {
            format!(
                "Abby braid soft-body head joint outside skeletal palette joint={} palette={}",
                self.head_joint,
                palette.len()
            )
        })?;
        let torso = *palette.get(self.torso_joint).ok_or_else(|| {
            format!(
                "Abby braid soft-body torso joint outside skeletal palette joint={} palette={}",
                self.torso_joint,
                palette.len()
            )
        })?;
        let bind = ABBY_BRAID_BIND_POINTS.map(|p| Vec3::new(p[0], p[1], p[2]));
        let guide = bind.map(|point| head.transform_point3(point));
        if !self.initialized {
            self.points = guide;
            self.previous_points = guide;
            self.previous_root_velocity_local = root_velocity_local;
            self.initialized = true;
        } else {
            // A braid behaves closer to a heavy articulated rod than a loose rope.  Run
            // several small PBD substeps, preserve both segment length and bend radius,
            // and softly shape-match to the head-space authored curve.  This gives the
            // lower braid inertia without letting it become rubbery or collapse into a line.
            let frame_dt = dt.clamp(1.0 / 240.0, 1.0 / 20.0);
            const SUBSTEPS: usize = 3;
            const ITERATIONS: usize = 7;
            let step_dt = frame_dt / SUBSTEPS as f32;
            let mut root_acceleration_local =
                (root_velocity_local - self.previous_root_velocity_local) / frame_dt.max(1.0e-5);
            let acceleration_len = root_acceleration_local.length();
            if acceleration_len > 22.0 {
                root_acceleration_local *= 22.0 / acceleration_len;
            }
            self.previous_root_velocity_local = root_velocity_local;
            let gravity = Vec3::new(0.0, -5.4 * step_dt * step_dt, 0.0);
            let inertial = -root_acceleration_local * (0.42 * step_dt * step_dt);

            let back_a = torso.transform_point3(Vec3::new(0.0, 1.18, -0.015));
            let back_b = torso.transform_point3(Vec3::new(0.0, 1.56, -0.025));
            let neck_center = head.transform_point3(Vec3::new(0.0, 1.61, -0.035));
            let shoulder_l = torso.transform_point3(Vec3::new(0.16, 1.48, -0.015));
            let shoulder_r = torso.transform_point3(Vec3::new(-0.16, 1.48, -0.015));

            for _ in 0..SUBSTEPS {
                for index in ABBY_BRAID_PINNED_JOINTS..ABBY_BRAID_SOFT_BODY_JOINTS {
                    let current = self.points[index];
                    let velocity = (current - self.previous_points[index]) * 0.972;
                    self.previous_points[index] = current;
                    self.points[index] = current + velocity + gravity + inertial;
                }
                for index in 0..ABBY_BRAID_PINNED_JOINTS {
                    self.points[index] = guide[index];
                    self.previous_points[index] = guide[index];
                }

                for _ in 0..ITERATIONS {
                    for index in 0..ABBY_BRAID_PINNED_JOINTS {
                        self.points[index] = guide[index];
                    }
                    // Stretch constraints.
                    for index in 0..ABBY_BRAID_SOFT_BODY_JOINTS - 1 {
                        let rest = (bind[index + 1] - bind[index]).length();
                        solve_braid_distance(&mut self.points, index, index + 1, rest, 1.0);
                    }
                    // Bend constraints make the plait keep a believable radius/weight instead
                    // of behaving like a necklace.  The stiffness fades toward the free tip.
                    for index in 0..ABBY_BRAID_SOFT_BODY_JOINTS - 2 {
                        let rest = (bind[index + 2] - bind[index]).length();
                        let t = index as f32 / (ABBY_BRAID_SOFT_BODY_JOINTS - 2) as f32;
                        solve_braid_distance(
                            &mut self.points,
                            index,
                            index + 2,
                            rest,
                            0.34 - 0.16 * t,
                        );
                    }
                    // Shape memory follows animated head motion, strong at the tie and subtle at
                    // the tip.  It damps implausible sideways kinks while retaining delayed sway.
                    for index in ABBY_BRAID_PINNED_JOINTS..ABBY_BRAID_SOFT_BODY_JOINTS {
                        let t = index as f32 / (ABBY_BRAID_SOFT_BODY_JOINTS - 1) as f32;
                        let stiffness = 0.085 * (1.0 - t).powi(2) + 0.006;
                        self.points[index] = self.points[index].lerp(guide[index], stiffness);
                    }
                    for index in 0..ABBY_BRAID_PINNED_JOINTS {
                        self.points[index] = guide[index];
                    }

                    for index in ABBY_BRAID_PINNED_JOINTS..ABBY_BRAID_SOFT_BODY_JOINTS {
                        project_out_of_capsule(&mut self.points[index], back_a, back_b, 0.125);
                        project_out_of_sphere(&mut self.points[index], neck_center, 0.105);
                        if index >= ABBY_BRAID_PINNED_JOINTS + 2 {
                            project_out_of_sphere(&mut self.points[index], shoulder_l, 0.105);
                            project_out_of_sphere(&mut self.points[index], shoulder_r, 0.105);
                        }
                    }
                    // Collision projection may separate neighbouring particles.  Finish every
                    // solver iteration with a hard stretch pass so braid links remain taut.
                    for _ in 0..4 {
                        for index in 0..ABBY_BRAID_PINNED_JOINTS {
                            self.points[index] = guide[index];
                        }
                        for index in 0..ABBY_BRAID_SOFT_BODY_JOINTS - 1 {
                            let rest = (bind[index + 1] - bind[index]).length();
                            solve_braid_distance(&mut self.points, index, index + 1, rest, 1.0);
                        }
                    }
                    for index in 0..ABBY_BRAID_PINNED_JOINTS {
                        self.points[index] = guide[index];
                    }
                }
            }
        }

        let (_, head_rotation, _) = head.to_scale_rotation_translation();
        for index in 0..ABBY_BRAID_SOFT_BODY_JOINTS {
            let guide_direction = if index + 1 < ABBY_BRAID_SOFT_BODY_JOINTS {
                guide[index + 1] - guide[index]
            } else {
                guide[index] - guide[index - 1]
            }
            .normalize_or_zero();
            let current_direction = if index + 1 < ABBY_BRAID_SOFT_BODY_JOINTS {
                self.points[index + 1] - self.points[index]
            } else {
                self.points[index] - self.points[index - 1]
            }
            .normalize_or_zero();
            // Preserve head roll/twist first, then bend the physical chain away from the
            // head-space guide.  Without the head rotation the braid could remain in its
            // original bind plane while the head turned, which visually detached it.
            let bend = if guide_direction.length_squared() > 1.0e-8
                && current_direction.length_squared() > 1.0e-8
            {
                Quat::from_rotation_arc(guide_direction, current_direction)
            } else {
                Quat::IDENTITY
            };
            let rotation = (bend * head_rotation).normalize();
            let deformation = Mat4::from_translation(self.points[index])
                * Mat4::from_quat(rotation)
                * Mat4::from_translation(-bind[index]);
            palette.push(deformation);
        }
        Ok(())
    }
}

fn solve_braid_distance(
    points: &mut [Vec3; ABBY_BRAID_SOFT_BODY_JOINTS],
    a: usize,
    b: usize,
    rest: f32,
    stiffness: f32,
) {
    let delta = points[b] - points[a];
    let length = delta.length();
    if length <= 1.0e-6 || !length.is_finite() {
        return;
    }
    let correction = delta * (((length - rest) / length) * stiffness.clamp(0.0, 1.0));
    let a_pinned = a < ABBY_BRAID_PINNED_JOINTS;
    let b_pinned = b < ABBY_BRAID_PINNED_JOINTS;
    match (a_pinned, b_pinned) {
        (true, true) => {}
        (true, false) => points[b] -= correction,
        (false, true) => points[a] += correction,
        (false, false) => {
            points[a] += correction * 0.5;
            points[b] -= correction * 0.5;
        }
    }
}

fn project_out_of_sphere(point: &mut Vec3, center: Vec3, radius: f32) {
    let delta = *point - center;
    let distance = delta.length();
    if distance < radius {
        let normal = if distance > 1.0e-6 {
            delta / distance
        } else {
            Vec3::new(0.0, 0.0, -1.0)
        };
        *point = center + normal * radius;
    }
}

fn project_out_of_capsule(point: &mut Vec3, a: Vec3, b: Vec3, radius: f32) {
    let axis = b - a;
    let axis_len_sq = axis.length_squared();
    if axis_len_sq <= 1.0e-8 {
        return;
    }
    let t = ((*point - a).dot(axis) / axis_len_sq).clamp(0.0, 1.0);
    let closest = a + axis * t;
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
    /// Mirrored Naughty Dog deform/helper branches must follow their primary joints.
    helper_mirror_pairs: Vec<(usize, usize)>,
    /// Blender control/face branches in the source rig are constraint-driven and are not
    /// parented under the deform spine in exported hierarchy. Project their deformation
    /// through the animated upper-spine driver so face/eyes/head follow locomotion.
    head_follow_driver: Option<usize>,
    head_follow_joints: Vec<usize>,
    braid_soft_body: Option<AbbyBraidSoftBodyRuntime>,
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

    pub(super) fn expected_palette_joints(&self) -> usize {
        self.skeleton.joints.len()
            + self
                .braid_soft_body
                .as_ref()
                .map(|_| ABBY_BRAID_SOFT_BODY_JOINTS)
                .unwrap_or(0)
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
            L::CrouchWalk => &[5, 1, 0],
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

fn build_detached_head_follow(skeleton: &ModelSkeletonMetadata) -> (Option<usize>, Vec<usize>) {
    let driver = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == "DEF-spine.006")
        .or_else(|| {
            skeleton
                .joints
                .iter()
                .position(|joint| joint.name == skeleton.anchors.head)
        });
    let roots = skeleton
        .joints
        .iter()
        .enumerate()
        .filter_map(|(index, joint)| {
            matches!(
                joint.name.as_str(),
                "ORG-face" | "MCH-eyes_parent" | "MCH-ROT-neck"
            )
            .then_some(index)
        })
        .collect::<Vec<_>>();
    if driver.is_none() || roots.is_empty() {
        return (driver, Vec::new());
    }
    let mut followers = Vec::new();
    for index in 0..skeleton.joints.len() {
        let mut cursor = Some(index);
        let mut follows = false;
        while let Some(current) = cursor {
            if roots.contains(&current) {
                follows = true;
                break;
            }
            cursor = skeleton.joints[current]
                .parent_index
                .map(|value| value as usize);
        }
        if follows && Some(index) != driver {
            followers.push(index);
        }
    }
    (driver, followers)
}

fn apply_detached_head_follow_palette(
    driver: Option<usize>,
    followers: &[usize],
    palette: &mut [Mat4],
) -> Result<(), String> {
    let Some(driver) = driver else {
        return Ok(());
    };
    let driver_matrix = *palette
        .get(driver)
        .ok_or_else(|| format!("head-follow driver outside palette joint={driver}"))?;
    for &joint in followers {
        let local_deformation = *palette
            .get(joint)
            .ok_or_else(|| format!("head-follow joint outside palette joint={joint}"))?;
        palette[joint] = driver_matrix * local_deformation;
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
        out.push(JointLocalPose {
            translation: [translation.x, translation.y, translation.z],
            rotation: [q.x, q.y, q.z, q.w],
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
    if clip.joint_count() != skeleton.joints.len() {
        return Err(format!(
            "player animation/skeleton joint count mismatch clip={} skeleton={} ref='{}'",
            clip.joint_count(),
            skeleton.joints.len(),
            clip_ref
        ));
    }
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
    for (index, joint) in skeleton.joints.iter().enumerate() {
        if clip.joint_tags.get(index).copied() != Some(joint.tag) {
            return Err(format!(
                "player animation skeleton tag mismatch ref='{}' index={} clip={:?} skeleton={}",
                clip_ref,
                index,
                clip.joint_tags.get(index),
                joint.tag
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

fn prepare_abby_braid_soft_body(
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    parts: &[PlayerRuntimeModelPart],
    skeleton: &ModelSkeletonMetadata,
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
    if !has_supplemental_skin {
        return Ok(None);
    }
    let normalized = assignment
        .source
        .trim()
        .replace('\\', "/")
        .to_ascii_lowercase();
    if !normalized.contains("/characters/abby/") && !normalized.contains("@abby") {
        return Err(format!(
            "supplemental player soft-body palette is only authored for Abby source='{}'",
            assignment.source
        ));
    }
    let head_joint = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == skeleton.anchors.head || joint.name == "head")
        .ok_or_else(|| "Abby braid soft-body requires authored head joint".to_owned())?;
    let torso_joint = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == "DEF-spine.004")
        .ok_or_else(|| {
            "Abby braid soft-body requires authored upper-torso deform joint".to_owned()
        })?;
    Ok(Some(AbbyBraidSoftBodyRuntime::new(head_joint, torso_joint)))
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
    let (head_follow_driver, head_follow_joints) = build_detached_head_follow(skeleton);
    let braid_soft_body = prepare_abby_braid_soft_body(assignment, parts, skeleton)?;
    let mut current_locals = Vec::with_capacity(skeleton.joints.len());
    idle.clip.sample_local_pose(0.0, &mut current_locals)?;
    synchronize_helper_pose(&helper_mirror_pairs, &mut current_locals);
    let mut palette_scratch = Vec::with_capacity(skeleton.joints.len());
    build_skin_palette_from_local_pose(
        skeleton,
        source_to_model,
        &current_locals,
        &mut palette_scratch,
    )?;
    apply_detached_head_follow_palette(
        head_follow_driver,
        &head_follow_joints,
        &mut palette_scratch,
    )?;
    let sampled_target_locals = current_locals.clone();
    let transition_from_locals = current_locals.clone();
    if !helper_mirror_pairs.is_empty() {
        newengine_ulog_api::ulog::info!(
            "game-ready: mirrored Naughty Dog helper rig channels={} policy='primary local pose -> *_helper deform branch before skin palette'",
            helper_mirror_pairs.len()
        );
    }
    if !head_follow_joints.is_empty() {
        newengine_ulog_api::ulog::info!(
            "game-ready: detached face/head rig followers={} driver={:?} policy='upper deform spine palette -> exported control/face branches'",
            head_follow_joints.len(),
            head_follow_driver
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
        helper_mirror_pairs,
        head_follow_driver,
        head_follow_joints,
        braid_soft_body,
    }))
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
        let root_velocity_local = world
            .get::<Transform>(player)
            .map(|transform| transform.rotation.inverse() * world_velocity)
            .unwrap_or(world_velocity);
        let (palette, clip_ref, active_state) = {
            let Some(binding) = world.get_mut::<PlayerAnimationRuntimeBinding>(player) else {
                continue;
            };
            let desired_slot = binding.resolve_slot(animation_state.locomotion);
            let transitioned = binding.active_state != animation_state.locomotion
                || binding.active_slot != desired_slot;
            if transitioned {
                // Cross-fade from the pose that was actually visible, not merely from
                // the previous clip. This keeps hands/forearms continuous even if the
                // player changes locomotion state again before the prior fade finishes.
                binding
                    .transition_from_locals
                    .clone_from(&binding.current_locals);
                binding.active_state = animation_state.locomotion;
                binding.active_slot = desired_slot;
                binding.time_seconds = 0.0;
            } else {
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
            if let Err(error) = active_clip
                .clip
                .sample_local_pose(binding.time_seconds, &mut binding.sampled_target_locals)
            {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: player animation sample failed player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }

            synchronize_helper_pose(
                &binding.helper_mirror_pairs,
                &mut binding.sampled_target_locals,
            );

            let alpha = animation_state.transition_alpha.clamp(0.0, 1.0);
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

            synchronize_helper_pose(&binding.helper_mirror_pairs, &mut binding.current_locals);

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
                binding.head_follow_driver,
                &binding.head_follow_joints,
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
            if let Some(braid) = binding.braid_soft_body.as_mut() {
                if let Err(error) =
                    braid.tick_and_append(dt, root_velocity_local, &mut binding.palette_scratch)
                {
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
                    .map(|_| ABBY_BRAID_SOFT_BODY_JOINTS)
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
    fn detached_face_palette_inherits_upper_spine_driver() {
        let mut palette = vec![Mat4::IDENTITY; 3];
        palette[0] = Mat4::from_translation(Vec3::new(0.2, 0.1, -0.3));
        palette[1] = Mat4::from_translation(Vec3::new(0.0, 0.02, 0.0));
        apply_detached_head_follow_palette(Some(0), &[1], &mut palette).expect("projection");
        let moved = palette[1].transform_point3(Vec3::ZERO);
        assert!((moved.x - 0.2).abs() < 1.0e-5);
        assert!((moved.y - 0.12).abs() < 1.0e-5);
        assert!((moved.z + 0.3).abs() < 1.0e-5);
    }

    #[test]
    fn abby_braid_soft_body_appends_eighteen_finite_joint_matrices() {
        let mut braid = AbbyBraidSoftBodyRuntime::new(0, 0);
        let head = Mat4::from_translation(Vec3::new(0.05, 0.02, -0.01));
        let mut palette = vec![head];
        braid
            .tick_and_append(1.0 / 60.0, Vec3::ZERO, &mut palette)
            .expect("soft body");
        assert_eq!(palette.len(), 1 + ABBY_BRAID_SOFT_BODY_JOINTS);
        assert!(palette
            .iter()
            .all(|matrix| { matrix.to_cols_array().iter().all(|value| value.is_finite()) }));
        let bind_root = Vec3::new(
            ABBY_BRAID_BIND_POINTS[0][0],
            ABBY_BRAID_BIND_POINTS[0][1],
            ABBY_BRAID_BIND_POINTS[0][2],
        );
        let deformed_root = palette[1].transform_point3(bind_root);
        let expected_root = head.transform_point3(bind_root);
        assert!((deformed_root - expected_root).length() < 1.0e-4);
    }

    #[test]
    fn abby_braid_soft_body_preserves_segment_lengths_under_gravity() {
        let mut braid = AbbyBraidSoftBodyRuntime::new(0, 0);
        let mut palette = vec![Mat4::IDENTITY];
        braid
            .tick_and_append(1.0 / 60.0, Vec3::ZERO, &mut palette)
            .expect("init");
        for _ in 0..120 {
            let mut palette = vec![Mat4::IDENTITY];
            braid
                .tick_and_append(1.0 / 60.0, Vec3::ZERO, &mut palette)
                .expect("step");
        }
        let bind = ABBY_BRAID_BIND_POINTS.map(|p| Vec3::new(p[0], p[1], p[2]));
        for index in 0..ABBY_BRAID_SOFT_BODY_JOINTS - 1 {
            let rest = (bind[index + 1] - bind[index]).length();
            let actual = (braid.points[index + 1] - braid.points[index]).length();
            assert!(
                (actual - rest).abs() < 0.004,
                "segment={index} rest={rest} actual={actual}"
            );
        }
    }

    #[test]
    fn local_pose_crossfade_preserves_endpoints_and_shortest_quaternion_path() {
        let from = [JointLocalPose {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }];
        let to = [JointLocalPose {
            translation: [2.0, 4.0, 6.0],
            // Same identity rotation with opposite quaternion sign.
            rotation: [0.0, 0.0, 0.0, -1.0],
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
