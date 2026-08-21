#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime decoding/sampling for canonical NorthStar YCD clip bodies and generic
//! linear-blend-skinning palette construction.

use newengine_math::{Mat4, Quat, Vec3};
use newengine_model_skeleton_api::ModelSkeletonMetadata;

pub const YCD_BODY_SCHEMA_VERSION: u32 = 1;
pub const YCD_BODY_HEADER_LEN: usize = 48;
pub const YCD_CLIP_RECORD_LEN: usize = 64;
pub const YCD_CLIP_FLAG_LOOP: u32 = 0x1;
const LOCAL_POSE_STRIDE: usize = 28;

#[inline]
fn vec3(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

#[inline]
fn vec3_array(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

#[inline]
fn quat(value: [f32; 4]) -> Quat {
    Quat::from_xyzw(value[0], value[1], value[2], value[3])
}

#[inline]
fn quat_array(value: Quat) -> [f32; 4] {
    [value.x, value.y, value.z, value.w]
}

#[inline]
fn affine_invertible(matrix: Mat4) -> bool {
    let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
    scale.x.is_finite()
        && scale.y.is_finite()
        && scale.z.is_finite()
        && scale.x.abs() > 1.0e-8
        && scale.y.abs() > 1.0e-8
        && scale.z.abs() > 1.0e-8
        && rotation.is_finite()
        && translation.x.is_finite()
        && translation.y.is_finite()
        && translation.z.is_finite()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointLocalPose {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
}

impl JointLocalPose {
    #[inline]
    pub fn matrix(self, scale: [f32; 3]) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            vec3(scale),
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
            out.push(JointLocalPose {
                translation: vec3_array(translation),
                rotation: quat_array(qa),
            });
        }
        Ok(())
    }
}

pub fn decode_ycd_body(body: &[u8], selector: Option<&str>) -> Result<AnimationClip, String> {
    if body.len() < YCD_BODY_HEADER_LEN {
        return Err(format!(
            "YCD body too small bytes={} expected>={YCD_BODY_HEADER_LEN}",
            body.len()
        ));
    }
    let schema = read_u32(body, 0)?;
    if schema != YCD_BODY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported YCD body schema={schema} expected={YCD_BODY_SCHEMA_VERSION}"
        ));
    }
    let clip_count = read_u32(body, 4)? as usize;
    if clip_count == 0 {
        return Err("YCD body contains no clips".to_owned());
    }
    let table_offset = usize_from_u64(read_u64(body, 8)?, "clip table")?;
    let string_offset = usize_from_u64(read_u64(body, 16)?, "string table")?;
    let string_len = usize_from_u64(read_u64(body, 24)?, "string length")?;
    let payload_floor = usize_from_u64(read_u64(body, 32)?, "payload floor")?;
    checked_slice(
        body,
        table_offset,
        clip_count
            .checked_mul(YCD_CLIP_RECORD_LEN)
            .ok_or("YCD clip table overflow")?,
        "clip table",
    )?;
    let strings = checked_slice(body, string_offset, string_len, "string table")?;
    if payload_floor > body.len() {
        return Err("YCD payload floor outside body".to_owned());
    }

    let requested = selector.map(str::trim).filter(|value| !value.is_empty());
    let mut selected_record = None;
    for index in 0..clip_count {
        let record = table_offset + index * YCD_CLIP_RECORD_LEN;
        let name = read_string(strings, read_u32(body, record + 8)?)?;
        let matches = requested
            .map(|value| name.eq_ignore_ascii_case(value))
            .unwrap_or(index == 0);
        if matches {
            selected_record = Some((record, name));
            break;
        }
    }
    let (record, name) = selected_record.ok_or_else(|| {
        format!(
            "YCD selector '{}' was not found",
            requested.unwrap_or("<first>")
        )
    })?;

    let skeleton_ref = read_string(strings, read_u32(body, record + 12)?)?;
    let joint_count = read_u32(body, record + 16)? as usize;
    let frame_count = read_u32(body, record + 20)? as usize;
    let duration_seconds = read_f32(body, record + 24)?;
    let sample_rate_hz = read_f32(body, record + 28)?;
    let flags = read_u32(body, record + 32)?;
    if flags & !YCD_CLIP_FLAG_LOOP != 0 {
        return Err(format!(
            "YCD clip '{name}' has unsupported flags=0x{flags:08x}"
        ));
    }
    if joint_count == 0 || frame_count == 0 || joint_count > 4096 || frame_count > 1_000_000 {
        return Err(format!(
            "YCD clip '{name}' invalid dimensions joints={joint_count} frames={frame_count}"
        ));
    }
    if duration_seconds <= 0.0 || sample_rate_hz <= 0.0 {
        return Err(format!(
            "YCD clip '{name}' invalid timing duration={duration_seconds} sample_rate={sample_rate_hz}"
        ));
    }
    let payload_offset = usize_from_u64(read_u64(body, record + 40)?, "clip payload")?;
    let payload_len = usize_from_u64(read_u64(body, record + 48)?, "clip payload length")?;
    if payload_offset < payload_floor {
        return Err(format!("YCD clip '{name}' payload precedes payload floor"));
    }
    let payload = checked_slice(body, payload_offset, payload_len, "clip payload")?;
    let source = read_string(
        strings,
        usize_from_u64(read_u64(body, record + 56)?, "source string offset")? as u32,
    )?;
    let tag_bytes = joint_count.checked_mul(4).ok_or("YCD tag bytes overflow")?;
    let pose_count = joint_count
        .checked_mul(frame_count)
        .ok_or("YCD pose count overflow")?;
    let pose_bytes = pose_count
        .checked_mul(LOCAL_POSE_STRIDE)
        .ok_or("YCD pose bytes overflow")?;
    if tag_bytes
        .checked_add(pose_bytes)
        .ok_or("YCD payload size overflow")?
        != payload.len()
    {
        return Err(format!(
            "YCD clip '{name}' payload size mismatch actual={} expected={} tags={} poses={}",
            payload.len(),
            tag_bytes + pose_bytes,
            tag_bytes,
            pose_bytes
        ));
    }
    let mut joint_tags = Vec::with_capacity(joint_count);
    for joint in 0..joint_count {
        joint_tags.push(read_u32(payload, joint * 4)?);
    }
    let mut poses = Vec::with_capacity(pose_count);
    let mut cursor = tag_bytes;
    for _ in 0..pose_count {
        let translation = [
            read_f32(payload, cursor)?,
            read_f32(payload, cursor + 4)?,
            read_f32(payload, cursor + 8)?,
        ];
        let rotation = [
            read_f32(payload, cursor + 12)?,
            read_f32(payload, cursor + 16)?,
            read_f32(payload, cursor + 20)?,
            read_f32(payload, cursor + 24)?,
        ];
        let q = quat(rotation);
        let len2 = q.length_squared();
        if !len2.is_finite() || len2 <= 1.0e-8 {
            return Err(format!("YCD clip '{name}' contains invalid quaternion"));
        }
        poses.push(JointLocalPose {
            translation,
            rotation: quat_array(q.normalize()),
        });
        cursor += LOCAL_POSE_STRIDE;
    }
    Ok(AnimationClip {
        name,
        skeleton_ref,
        source,
        duration_seconds,
        sample_rate_hz,
        looped: flags & YCD_CLIP_FLAG_LOOP != 0,
        joint_tags,
        poses,
    })
}

/// Builds and validates the authored bind-pose skin palette.
///
/// A correct bind pose must reduce to identity in model space for every joint.
/// This is intentionally computed through the same hierarchy/source-space math as
/// animated palettes instead of returning `Mat4::IDENTITY` blindly, so malformed
/// skeleton hierarchy or source transforms fail before reaching the GPU.
pub fn build_bind_pose_palette(
    skeleton: &ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    out_palette: &mut Vec<Mat4>,
) -> Result<(), String> {
    let joint_count = skeleton.joints.len();
    if joint_count == 0 {
        return Err("bind-pose palette requires at least one skeleton joint".to_owned());
    }
    for (index, joint) in skeleton.joints.iter().enumerate() {
        if joint.index as usize != index {
            return Err(format!(
                "skeleton joint indices must be dense index={} authored={}",
                index, joint.index
            ));
        }
    }
    let bind_locals = skeleton
        .joints
        .iter()
        .map(|joint| JointLocalPose {
            translation: joint.position_ls,
            rotation: joint.rotation_ls,
        })
        .collect::<Vec<_>>();
    let bind_globals = build_globals(skeleton, &bind_locals)?;
    let source_to_model = Mat4::from_cols_array(&source_to_model);
    if !affine_invertible(source_to_model) {
        return Err("skin source-to-model transform is singular/non-finite".to_owned());
    }
    let model_to_source = source_to_model.inverse();
    out_palette.clear();
    out_palette.reserve(joint_count);
    let mut max_identity_error = 0.0_f32;
    let mut max_joint = 0usize;
    for (index, bind) in bind_globals.into_iter().enumerate() {
        if !affine_invertible(bind) {
            return Err(format!("bind global matrix is singular joint={index}"));
        }
        let palette = source_to_model * (bind * bind.inverse()) * model_to_source;
        let values = palette.to_cols_array();
        if values.iter().any(|value| !value.is_finite()) {
            return Err(format!("bind-pose palette contains non-finite value joint={index}"));
        }
        let identity = Mat4::IDENTITY.to_cols_array();
        let error = values
            .iter()
            .zip(identity.iter())
            .map(|(actual, expected)| (actual - expected).abs())
            .fold(0.0_f32, f32::max);
        if error > max_identity_error {
            max_identity_error = error;
            max_joint = index;
        }
        out_palette.push(palette);
    }
    const MAX_BIND_PALETTE_IDENTITY_ERROR: f32 = 1.0e-4;
    if max_identity_error > MAX_BIND_PALETTE_IDENTITY_ERROR {
        return Err(format!(
            "bind-pose palette is not identity max_error={max_identity_error:.8} joint={max_joint} limit={MAX_BIND_PALETTE_IDENTITY_ERROR}"
        ));
    }
    Ok(())
}

/// Builds skin matrices in baked model space.
///
/// The clip and skeleton remain in the authored source space. The final conjugation
/// by `source_to_model` is what makes a palette valid for vertices whose positions
/// were baked through an importer transform (for example RAGE Z-up -> NewEngine Y-up).
pub fn build_skin_palette(
    clip: &AnimationClip,
    skeleton: &ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    time_seconds: f32,
    sampled_locals: &mut Vec<JointLocalPose>,
    out_palette: &mut Vec<Mat4>,
) -> Result<(), String> {
    let joint_count = skeleton.joints.len();
    if clip.joint_count() != joint_count {
        return Err(format!(
            "animation/skeleton joint count mismatch clip={} skeleton={}",
            clip.joint_count(),
            joint_count
        ));
    }
    for (index, joint) in skeleton.joints.iter().enumerate() {
        if joint.index as usize != index {
            return Err(format!(
                "skeleton joint indices must be dense index={} authored={}",
                index, joint.index
            ));
        }
        if clip.joint_tags[index] != joint.tag {
            return Err(format!(
                "animation/skeleton tag mismatch index={index} clip={} skeleton={}",
                clip.joint_tags[index], joint.tag
            ));
        }
    }
    clip.sample_local_pose(time_seconds, sampled_locals)?;
    let bind_locals = skeleton
        .joints
        .iter()
        .map(|joint| JointLocalPose {
            translation: joint.position_ls,
            rotation: joint.rotation_ls,
        })
        .collect::<Vec<_>>();
    let bind_globals = build_globals(skeleton, &bind_locals)?;
    let animated_globals = build_globals(skeleton, sampled_locals)?;
    let source_to_model = Mat4::from_cols_array(&source_to_model);
    if !affine_invertible(source_to_model) {
        return Err("skin source-to-model transform is singular/non-finite".to_owned());
    }
    let model_to_source = source_to_model.inverse();
    out_palette.clear();
    out_palette.reserve(joint_count);
    for index in 0..joint_count {
        let bind = bind_globals[index];
        if !affine_invertible(bind) {
            return Err(format!("bind global matrix is singular joint={index}"));
        }
        let source_palette = animated_globals[index] * bind.inverse();
        let palette = source_to_model * source_palette * model_to_source;
        if palette.to_cols_array().iter().any(|value| !value.is_finite()) {
            return Err(format!("animated skin palette contains non-finite value joint={index}"));
        }
        out_palette.push(palette);
    }
    Ok(())
}

fn build_globals(
    skeleton: &ModelSkeletonMetadata,
    locals: &[JointLocalPose],
) -> Result<Vec<Mat4>, String> {
    let joint_count = skeleton.joints.len();
    if locals.len() != joint_count {
        return Err(format!(
            "local pose count mismatch poses={} joints={joint_count}",
            locals.len()
        ));
    }
    let mut globals = vec![Mat4::IDENTITY; joint_count];
    let mut resolved = vec![false; joint_count];
    let mut remaining = joint_count;
    while remaining > 0 {
        let mut progress = false;
        for (index, joint) in skeleton.joints.iter().enumerate() {
            if resolved[index] {
                continue;
            }
            let parent = joint.parent_index.map(|value| value as usize);
            if parent.is_some_and(|parent| parent >= joint_count) {
                return Err(format!(
                    "skeleton parent index outside joint table joint={index}"
                ));
            }
            if let Some(parent) = parent {
                if !resolved[parent] {
                    continue;
                }
            }
            let local = locals[index].matrix(joint.scale_ls);
            globals[index] = parent
                .map(|parent| globals[parent] * local)
                .unwrap_or(local);
            resolved[index] = true;
            remaining -= 1;
            progress = true;
        }
        if !progress {
            return Err("skeleton hierarchy contains a cycle/unresolvable parent".to_owned());
        }
    }
    Ok(globals)
}

#[inline]
fn checked_slice<'a>(
    bytes: &'a [u8],
    offset: usize,
    len: usize,
    label: &str,
) -> Result<&'a [u8], String> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| format!("YCD {label} range overflow"))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| format!("YCD {label} outside body offset={offset} len={len}"))
}

#[inline]
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        checked_slice(bytes, offset, 4, "u32")?
            .try_into()
            .expect("u32 slice"),
    ))
}

#[inline]
fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        checked_slice(bytes, offset, 8, "u64")?
            .try_into()
            .expect("u64 slice"),
    ))
}

#[inline]
fn read_f32(bytes: &[u8], offset: usize) -> Result<f32, String> {
    let value = f32::from_le_bytes(
        checked_slice(bytes, offset, 4, "f32")?
            .try_into()
            .expect("f32 slice"),
    );
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("YCD contains non-finite f32 at {offset}"))
    }
}

#[inline]
fn usize_from_u64(value: u64, label: &str) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| format!("YCD {label} exceeds usize"))
}

fn read_string(strings: &[u8], offset: u32) -> Result<String, String> {
    let start = offset as usize;
    let tail = strings
        .get(start..)
        .ok_or_else(|| format!("YCD string offset outside table offset={offset}"))?;
    let len = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| format!("YCD string not terminated offset={offset}"))?;
    String::from_utf8(tail[..len].to_vec())
        .map_err(|error| format!("YCD string is not UTF-8: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_model_skeleton_api::{ModelSkeletonAnchors, ModelSkeletonJointMetadata};

    fn test_body() -> Vec<u8> {
        let strings = b"idle\0skeleton.ymt@body\0source.ycd\0";
        let table_offset = YCD_BODY_HEADER_LEN;
        let string_offset = table_offset + YCD_CLIP_RECORD_LEN;
        let payload_offset = string_offset + strings.len();
        let joint_count = 1u32;
        let frame_count = 2u32;
        let payload_len = 4 + 2 * LOCAL_POSE_STRIDE;
        let mut out = Vec::new();
        out.extend_from_slice(&YCD_BODY_SCHEMA_VERSION.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        for value in [
            table_offset as u64,
            string_offset as u64,
            strings.len() as u64,
            payload_offset as u64,
            0,
        ] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&1u64.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&5u32.to_le_bytes());
        out.extend_from_slice(&joint_count.to_le_bytes());
        out.extend_from_slice(&frame_count.to_le_bytes());
        out.extend_from_slice(&1.0f32.to_le_bytes());
        out.extend_from_slice(&2.0f32.to_le_bytes());
        out.extend_from_slice(&YCD_CLIP_FLAG_LOOP.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(payload_offset as u64).to_le_bytes());
        out.extend_from_slice(&(payload_len as u64).to_le_bytes());
        out.extend_from_slice(&23u64.to_le_bytes());
        out.extend_from_slice(strings);
        out.extend_from_slice(&42u32.to_le_bytes());
        for (translation, rotation) in [
            ([0.0f32, 0.0, 0.0], [0.0f32, 0.0, 0.0, 1.0]),
            ([1.0f32, 0.0, 0.0], [0.0f32, 0.0, 0.0, 1.0]),
        ] {
            for value in translation.into_iter().chain(rotation) {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        out
    }

    fn one_joint_skeleton() -> ModelSkeletonMetadata {
        ModelSkeletonMetadata {
            source: "skeleton.ymt".to_owned(),
            source_format: "test".to_owned(),
            container_magic: "NEF8".to_owned(),
            byte_len: 0,
            content_hash: "test".to_owned(),
            decode_status: "test".to_owned(),
            joints: vec![ModelSkeletonJointMetadata {
                index: 0,
                tag: 42,
                name: "root".to_owned(),
                parent: None,
                parent_index: None,
                position_ls: [0.0, 0.0, 0.0],
                rotation_ls: [0.0, 0.0, 0.0, 1.0],
                scale_ls: [1.0, 1.0, 1.0],
                flags: Vec::new(),
            }],
            anchors: ModelSkeletonAnchors {
                root: "root".to_owned(),
                hips: "root".to_owned(),
                head: "root".to_owned(),
                left_hand: "root".to_owned(),
                right_hand: "root".to_owned(),
                left_foot: "root".to_owned(),
                right_foot: "root".to_owned(),
                eye: "root".to_owned(),
                eye_height: 1.0,
            },
        }
    }

    #[test]
    fn decodes_and_interpolates_canonical_ycd() {
        let clip = decode_ycd_body(&test_body(), Some("idle")).expect("decode");
        assert_eq!(clip.joint_tags, vec![42]);
        assert_eq!(clip.frame_count(), 2);
        let mut sampled = Vec::new();
        clip.sample_local_pose(0.25, &mut sampled).expect("sample");
        assert!((sampled[0].translation[0] - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn palette_conjugates_source_motion_into_model_space() {
        let clip = decode_ycd_body(&test_body(), None).expect("decode");
        let source_to_model = [
            2.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0, 0.0, 1.0,
        ];
        let mut sampled = Vec::new();
        let mut palette = Vec::new();
        build_skin_palette(
            &clip,
            &one_joint_skeleton(),
            source_to_model,
            0.25,
            &mut sampled,
            &mut palette,
        )
        .expect("palette");
        let moved = palette[0].transform_point3(Vec3::ZERO);
        // Source +0.5 X is scaled by source_to_model to +1.0 model-space X.
        assert!((moved.x - 1.0).abs() < 1.0e-5, "moved={moved:?}");
        assert!(moved.y.abs() < 1.0e-5);
        assert!(moved.z.abs() < 1.0e-5);
    }
    #[test]
    fn bind_pose_palette_is_validated_identity() {
        let source_to_model = [
            2.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0, 0.0, 1.0,
        ];
        let mut palette = Vec::new();
        build_bind_pose_palette(&one_joint_skeleton(), source_to_model, &mut palette)
            .expect("bind palette");
        assert_eq!(palette.len(), 1);
        let actual = palette[0].to_cols_array();
        let expected = Mat4::IDENTITY.to_cols_array();
        assert!(actual.iter().zip(expected.iter()).all(|(a, b)| (a - b).abs() < 1.0e-5));
    }

}
