pub fn decode_ycd_dictionary(body: &[u8]) -> Result<AnimationDictionary, String> {
    if body.len() < YCD_BODY_HEADER_LEN {
        return Err(format!(
            "YCD body too small bytes={} expected>={YCD_BODY_HEADER_LEN}",
            body.len()
        ));
    }
    let schema = read_u32(body, 0)?;
    if schema != YCD_BODY_SCHEMA_VERSION && schema != YCD_BODY_SCHEMA_VERSION_LEGACY {
        return Err(format!(
            "unsupported YCD body schema={schema} supported={YCD_BODY_SCHEMA_VERSION_LEGACY},{YCD_BODY_SCHEMA_VERSION}"
        ));
    }
    let local_pose_stride = if schema == YCD_BODY_SCHEMA_VERSION {
        LOCAL_POSE_STRIDE_V2
    } else {
        LOCAL_POSE_STRIDE_V1
    };
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

    let mut clips = Vec::with_capacity(clip_count);
    for index in 0..clip_count {
        let record = table_offset + index * YCD_CLIP_RECORD_LEN;
        clips.push(std::sync::Arc::new(decode_ycd_clip_record(
            body,
            strings,
            record,
            payload_floor,
            schema,
            local_pose_stride,
        )?));
    }
    Ok(AnimationDictionary { clips })
}

pub fn decode_ycd_body(body: &[u8], selector: Option<&str>) -> Result<AnimationClip, String> {
    let dictionary = decode_ycd_dictionary(body)?;
    dictionary
        .clip(selector)
        .map(|clip| (*clip).clone())
        .ok_or_else(|| {
            format!(
                "YCD selector '{}' was not found",
                selector
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("<first>")
            )
        })
}

fn decode_ycd_clip_record(
    body: &[u8],
    strings: &[u8],
    record: usize,
    payload_floor: usize,
    schema: u32,
    local_pose_stride: usize,
) -> Result<AnimationClip, String> {
    let name = read_string(strings, read_u32(body, record + 8)?)?;
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
    if !duration_seconds.is_finite()
        || !sample_rate_hz.is_finite()
        || duration_seconds <= 0.0
        || sample_rate_hz <= 0.0
    {
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
        .checked_mul(local_pose_stride)
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
        if translation.iter().any(|value| !value.is_finite()) {
            return Err(format!("YCD clip '{name}' contains invalid translation"));
        }
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
        let scale = if schema == YCD_BODY_SCHEMA_VERSION {
            let value = [
                read_f32(payload, cursor + 28)?,
                read_f32(payload, cursor + 32)?,
                read_f32(payload, cursor + 36)?,
            ];
            if value.iter().any(|component| !component.is_finite()) {
                return Err(format!("YCD clip '{name}' contains invalid scale"));
            }
            Some(value)
        } else {
            None
        };
        poses.push(JointLocalPose {
            translation,
            rotation: quat_array(q.normalize()),
            scale,
        });
        cursor += local_pose_stride;
    }
    let clip = AnimationClip {
        name,
        skeleton_ref,
        source,
        duration_seconds,
        sample_rate_hz,
        looped: flags & YCD_CLIP_FLAG_LOOP != 0,
        joint_tags,
        events: Vec::new(),
        poses,
    };
    clip.validate_structure()?;
    Ok(clip)
}
