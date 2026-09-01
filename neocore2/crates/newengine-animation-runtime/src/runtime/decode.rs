fn ycd_schema_supported(schema: u32) -> bool {
    matches!(
        schema,
        YCD_BODY_SCHEMA_VERSION_LEGACY | YCD_BODY_SCHEMA_VERSION_V2 | YCD_BODY_SCHEMA_VERSION
    )
}

fn ycd_pose_stride(schema: u32) -> Result<usize, String> {
    match schema {
        YCD_BODY_SCHEMA_VERSION_LEGACY => Ok(LOCAL_POSE_STRIDE_V1),
        YCD_BODY_SCHEMA_VERSION_V2 | YCD_BODY_SCHEMA_VERSION => Ok(LOCAL_POSE_STRIDE_V2),
        _ => Err(format!(
            "unsupported YCD body schema={schema} supported={YCD_BODY_SCHEMA_VERSION_LEGACY},{YCD_BODY_SCHEMA_VERSION_V2},{YCD_BODY_SCHEMA_VERSION}"
        )),
    }
}

fn ycd_event_table_offset(
    body: &[u8],
    schema: u32,
    payload_floor: usize,
) -> Result<Option<usize>, String> {
    if schema != YCD_BODY_SCHEMA_VERSION {
        return Ok(None);
    }
    let offset = usize_from_u64(read_u64(body, 40)?, "event table")?;
    if offset < payload_floor || offset > body.len() {
        return Err(format!(
            "YCD v3 event table outside payload region offset={offset} payload_floor={payload_floor} body={}",
            body.len()
        ));
    }
    Ok(Some(offset))
}

pub fn decode_ycd_dictionary(body: &[u8]) -> Result<AnimationDictionary, String> {
    if body.len() < YCD_BODY_HEADER_LEN {
        return Err(format!(
            "YCD body too small bytes={} expected>={YCD_BODY_HEADER_LEN}",
            body.len()
        ));
    }
    let schema = read_u32(body, 0)?;
    if !ycd_schema_supported(schema) {
        return Err(format!(
            "unsupported YCD body schema={schema} supported={YCD_BODY_SCHEMA_VERSION_LEGACY},{YCD_BODY_SCHEMA_VERSION_V2},{YCD_BODY_SCHEMA_VERSION}"
        ));
    }
    let local_pose_stride = ycd_pose_stride(schema)?;
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
    let event_table_offset = ycd_event_table_offset(body, schema, payload_floor)?;

    let mut clips = Vec::with_capacity(clip_count);
    for index in 0..clip_count {
        let record = table_offset + index * YCD_CLIP_RECORD_LEN;
        clips.push(std::sync::Arc::new(decode_ycd_clip_record(
            body,
            strings,
            record,
            payload_floor,
            event_table_offset,
            schema,
            local_pose_stride,
        )?));
    }
    Ok(AnimationDictionary { clips })
}

pub fn decode_ycd_body(body: &[u8], selector: Option<&str>) -> Result<AnimationClip, String> {
    let requested = selector.map(str::trim).filter(|value| !value.is_empty());
    if let Some(selector) = requested {
        return decode_ycd_selected_clip(body, selector);
    }
    let dictionary = decode_ycd_dictionary(body)?;
    dictionary
        .clip(None)
        .map(|clip| (*clip).clone())
        .ok_or_else(|| "YCD selector '<first>' was not found".to_owned())
}

/// Decodes one addressed clip without making unrelated dictionary entries part of the runtime
/// binding contract. Full dictionary validation remains the responsibility of
/// [`decode_ycd_dictionary`].
fn decode_ycd_selected_clip(body: &[u8], selector: &str) -> Result<AnimationClip, String> {
    if body.len() < YCD_BODY_HEADER_LEN {
        return Err(format!(
            "YCD body too small bytes={} expected>={YCD_BODY_HEADER_LEN}",
            body.len()
        ));
    }
    let schema = read_u32(body, 0)?;
    if !ycd_schema_supported(schema) {
        return Err(format!(
            "unsupported YCD body schema={schema} supported={YCD_BODY_SCHEMA_VERSION_LEGACY},{YCD_BODY_SCHEMA_VERSION_V2},{YCD_BODY_SCHEMA_VERSION}"
        ));
    }
    let local_pose_stride = ycd_pose_stride(schema)?;
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
    let event_table_offset = ycd_event_table_offset(body, schema, payload_floor)?;

    for index in 0..clip_count {
        let record = table_offset + index * YCD_CLIP_RECORD_LEN;
        let name_offset = read_u32(body, record + 8)?;
        let Ok(name) = read_string(strings, name_offset) else {
            continue;
        };
        if name.eq_ignore_ascii_case(selector) {
            return decode_ycd_clip_record(
                body,
                strings,
                record,
                payload_floor,
                event_table_offset,
                schema,
                local_pose_stride,
            );
        }
    }
    Err(format!("YCD selector '{selector}' was not found"))
}

fn decode_ycd_events(
    body: &[u8],
    strings: &[u8],
    clip_name: &str,
    event_table_offset: usize,
    event_start: usize,
    event_count: usize,
) -> Result<Vec<AnimationEvent>, String> {
    if event_count > 1_000_000 {
        return Err(format!(
            "YCD clip '{clip_name}' has unreasonable event count={event_count}"
        ));
    }
    let start = event_table_offset
        .checked_add(
            event_start
                .checked_mul(YCD_EVENT_RECORD_LEN)
                .ok_or("YCD event start overflow")?,
        )
        .ok_or("YCD event table offset overflow")?;
    checked_slice(
        body,
        start,
        event_count
            .checked_mul(YCD_EVENT_RECORD_LEN)
            .ok_or("YCD event table size overflow")?,
        "event records",
    )?;

    let mut events = Vec::with_capacity(event_count);
    for index in 0..event_count {
        let record = start + index * YCD_EVENT_RECORD_LEN;
        let time_seconds = read_f32(body, record)?;
        let tag = read_string(strings, read_u32(body, record + 4)?)?;
        let parameter_offset = read_u32(body, record + 8)? as usize;
        let parameter_count = read_u32(body, record + 12)? as usize;
        if parameter_count > 65_536 {
            return Err(format!(
                "YCD clip '{clip_name}' event={index} has unreasonable parameter count={parameter_count}"
            ));
        }
        let mut parameters = Vec::with_capacity(parameter_count);
        if parameter_count != 0 {
            checked_slice(
                body,
                parameter_offset,
                parameter_count
                    .checked_mul(YCD_EVENT_PARAMETER_RECORD_LEN)
                    .ok_or("YCD event parameter table size overflow")?,
                "event parameter records",
            )?;
            for parameter_index in 0..parameter_count {
                let at = parameter_offset + parameter_index * YCD_EVENT_PARAMETER_RECORD_LEN;
                parameters.push(AnimationEventParameter {
                    key: read_string(strings, read_u32(body, at)?)?,
                    value: read_string(strings, read_u32(body, at + 4)?)?,
                });
            }
        }
        events.push(AnimationEvent {
            time_seconds,
            tag,
            parameters,
        });
    }
    Ok(events)
}

fn decode_ycd_clip_record(
    body: &[u8],
    strings: &[u8],
    record: usize,
    payload_floor: usize,
    event_table_offset: Option<usize>,
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
    if let Some(event_table_offset) = event_table_offset {
        let payload_end = payload_offset
            .checked_add(payload_len)
            .ok_or("YCD clip payload end overflow")?;
        if payload_end > event_table_offset {
            return Err(format!(
                "YCD v3 clip '{name}' pose payload overlaps event table payload_end={payload_end} event_table={event_table_offset}"
            ));
        }
    }
    let payload = checked_slice(body, payload_offset, payload_len, "clip payload")?;
    let source_locator = read_u64(body, record + 56)?;
    let (source_offset, event_start, event_count) = if schema == YCD_BODY_SCHEMA_VERSION {
        (
            (source_locator & 0xffff_ffff) as u32,
            (source_locator >> 32) as usize,
            read_u32(body, record + 36)? as usize,
        )
    } else {
        let source_offset = usize_from_u64(source_locator, "source string offset")?;
        let source_offset = u32::try_from(source_offset)
            .map_err(|_| "YCD source string offset exceeds u32".to_owned())?;
        (source_offset, 0, 0)
    };
    let source = read_string(strings, source_offset)?;
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
        let scale = if schema == YCD_BODY_SCHEMA_VERSION_LEGACY {
            None
        } else {
            let value = [
                read_f32(payload, cursor + 28)?,
                read_f32(payload, cursor + 32)?,
                read_f32(payload, cursor + 36)?,
            ];
            if value.iter().any(|component| !component.is_finite()) {
                return Err(format!("YCD clip '{name}' contains invalid scale"));
            }
            Some(value)
        };
        poses.push(JointLocalPose {
            translation,
            rotation: quat_array(q.normalize()),
            scale,
        });
        cursor += local_pose_stride;
    }
    let events = match (event_table_offset, event_count) {
        (Some(offset), count) if count != 0 => {
            decode_ycd_events(body, strings, &name, offset, event_start, count)?
        }
        _ => Vec::new(),
    };
    let clip = AnimationClip {
        name,
        skeleton_ref,
        source,
        duration_seconds,
        sample_rate_hz,
        looped: flags & YCD_CLIP_FLAG_LOOP != 0,
        joint_tags,
        events,
        poses,
    };
    clip.validate_structure()?;
    Ok(clip)
}
