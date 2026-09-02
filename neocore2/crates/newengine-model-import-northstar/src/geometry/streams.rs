fn detect_submesh_stride(pak: &PakFile, table: usize, count: usize) -> Result<usize, String> {
    let mut best = (0usize, 0usize);
    for stride in [PC_SUBMESH_STRIDE, LEGACY_NORTHSTAR_SUBMESH_STRIDE] {
        let mut score = 0usize;
        for index in 0..count.min(64) {
            let field = table
                .saturating_add(index.saturating_mul(stride))
                .saturating_add(32);
            if let Ok(Some(pointer)) = pak.resolve_pointer(field) {
                if let Ok(name) = pak.string_at(pointer) {
                    if name.contains("Shape") || name.contains("LOD") {
                        score += 1;
                    }
                }
            }
        }
        if score > best.0 {
            best = (score, stride);
        }
    }
    if best.0 == 0 {
        Err("unable to determine NorthStar submesh record stride".to_owned())
    } else {
        Ok(best.1)
    }
}

fn lod_index(name: &str) -> u32 {
    for marker in ["LODShape", "Shape"] {
        if let Some(at) = name.find(marker) {
            if let Some(ch) = name[at + marker.len()..].chars().next() {
                if let Some(value) = ch.to_digit(10) {
                    return value;
                }
            }
        }
    }
    0
}

fn decode_stream_desc(pak: &PakFile, at: usize) -> Result<StreamDesc, String> {
    let buffer = pak
        .resolve_pointer(at)?
        .ok_or_else(|| format!("vertex stream has no buffer pointer at 0x{at:x}"))?;
    let num_vertices = pak.read_u32(at + 8)? as usize;
    let buffer_size = pak.read_u32(at + 16)? as usize;
    let kind = pak.read_u8(at + 20)?;
    let sizes = [
        pak.read_u8(at + 24)?,
        pak.read_u8(at + 25)?,
        pak.read_u8(at + 26)?,
        pak.read_u8(at + 27)?,
    ];
    let mut q_scale = [0.0; 4];
    let mut q_offset = [0.0; 4];
    for component in 0..4 {
        q_scale[component] = pak.read_f32(at + 32 + component * 4)?;
        q_offset[component] = pak.read_f32(at + 48 + component * 4)?;
    }
    Ok(StreamDesc {
        kind,
        buffer,
        buffer_size,
        num_vertices,
        sizes,
        q_scale,
        q_offset,
    })
}

fn decode_raw_f32_stream(
    pak: &PakFile,
    stream: &StreamDesc,
    vertex_count: usize,
    wanted_components: usize,
) -> Result<Vec<[f32; 4]>, String> {
    if wanted_components == 0 || wanted_components > 4 {
        return Err(format!(
            "invalid raw f32 component count {wanted_components}"
        ));
    }
    if stream.num_vertices < vertex_count {
        return Err(format!(
            "raw f32 stream shorter than submesh stream_vertices={} mesh_vertices={vertex_count}",
            stream.num_vertices
        ));
    }
    let stride = wanted_components
        .checked_mul(4)
        .ok_or("raw f32 vertex stride overflow")?;
    let required = vertex_count
        .checked_mul(stride)
        .ok_or("raw f32 byte range overflow")?;
    if stream.buffer_size < required {
        return Err(format!(
            "raw f32 stream buffer too small bytes={} required={required}",
            stream.buffer_size
        ));
    }
    let bytes = pak.slice(stream.buffer, required)?;
    let mut out = Vec::with_capacity(vertex_count);
    for vertex in 0..vertex_count {
        let mut value = [0.0f32; 4];
        let base = vertex * stride;
        for (component, output) in value.iter_mut().enumerate().take(wanted_components) {
            let at = base + component * 4;
            *output =
                f32::from_le_bytes(bytes[at..at + 4].try_into().expect("raw f32 component"));
        }
        if value[..wanted_components]
            .iter()
            .any(|component| !component.is_finite())
        {
            return Err("raw f32 vertex stream produced non-finite value".to_owned());
        }
        out.push(value);
    }
    Ok(out)
}

fn decode_raw_f16_stream(
    pak: &PakFile,
    stream: &StreamDesc,
    vertex_count: usize,
    wanted_components: usize,
) -> Result<Vec<[f32; 4]>, String> {
    if wanted_components == 0 || wanted_components > 4 {
        return Err(format!(
            "invalid raw f16 component count {wanted_components}"
        ));
    }
    if stream.num_vertices < vertex_count {
        return Err(format!(
            "raw f16 stream shorter than submesh stream_vertices={} mesh_vertices={vertex_count}",
            stream.num_vertices
        ));
    }
    let stride = wanted_components
        .checked_mul(2)
        .ok_or("raw f16 vertex stride overflow")?;
    let required = vertex_count
        .checked_mul(stride)
        .ok_or("raw f16 byte range overflow")?;
    if stream.buffer_size < required {
        return Err(format!(
            "raw f16 stream buffer too small bytes={} required={required}",
            stream.buffer_size
        ));
    }
    let bytes = pak.slice(stream.buffer, required)?;
    let mut out = Vec::with_capacity(vertex_count);
    for vertex in 0..vertex_count {
        let mut value = [0.0f32; 4];
        let base = vertex * stride;
        for (component, output) in value.iter_mut().enumerate().take(wanted_components) {
            let at = base + component * 2;
            let bits = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
            *output = f16_to_f32(bits);
        }
        if value[..wanted_components]
            .iter()
            .any(|component| !component.is_finite())
        {
            return Err("raw f16 vertex stream produced non-finite value".to_owned());
        }
        out.push(value);
    }
    Ok(out)
}

fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits & 0x8000) as u32) << 16;
    let exponent = ((bits >> 10) & 0x1f) as u32;
    let mantissa = (bits & 0x03ff) as u32;
    let raw = match exponent {
        0 => {
            if mantissa == 0 {
                sign
            } else {
                let mut mantissa = mantissa;
                let mut exponent = 113u32;
                while mantissa & 0x0400 == 0 {
                    mantissa <<= 1;
                    exponent -= 1;
                }
                mantissa &= 0x03ff;
                sign | (exponent << 23) | (mantissa << 13)
            }
        }
        0x1f => sign | 0x7f80_0000 | (mantissa << 13),
        _ => sign | ((exponent + 112) << 23) | (mantissa << 13),
    };
    f32::from_bits(raw)
}

fn decode_quantized_stream(
    pak: &PakFile,
    stream: &StreamDesc,
    vertex_count: usize,
    wanted_components: usize,
) -> Result<Vec<[f32; 4]>, String> {
    if stream.num_vertices < vertex_count {
        return Err(format!(
            "vertex stream shorter than submesh stream_vertices={} mesh_vertices={vertex_count}",
            stream.num_vertices
        ));
    }
    let data = pak.slice(stream.buffer, stream.buffer_size)?;
    let mut bits = LsbBitReader::new(data);
    let mut out = Vec::with_capacity(vertex_count);
    for _ in 0..vertex_count {
        let mut value = [0.0f32; 4];
        for (component, output) in value.iter_mut().enumerate() {
            let width = stream.sizes[component] as usize;
            if width > 32 {
                return Err(format!("unsupported quantized component width {width}"));
            }
            if width != 0 {
                *output = bits.read(width)? as f32 * stream.q_scale[component]
                    + stream.q_offset[component];
            } else if stream.kind == 64 && component < 3 {
                *output = stream.q_scale[component] + stream.q_offset[component];
            }
        }
        if value[..wanted_components]
            .iter()
            .any(|component| !component.is_finite())
        {
            return Err("quantized vertex stream produced non-finite value".to_owned());
        }
        out.push(value);
    }
    Ok(out)
}

fn decode_indices(
    pak: &PakFile,
    at: usize,
    index_count: usize,
    vertex_count: usize,
    mesh_name: &str,
) -> Result<Vec<u32>, String> {
    let bytes = pak.slice(
        at,
        index_count
            .checked_mul(2)
            .ok_or("index byte range overflow")?,
    )?;
    let mut out = Vec::with_capacity(index_count);
    for index in 0..index_count {
        let at = index * 2;
        let value = u16::from_le_bytes([bytes[at], bytes[at + 1]]) as u32;
        if value as usize >= vertex_count {
            return Err(format!(
                "source index out of range mesh='{mesh_name}' index={value} vertices={vertex_count}"
            ));
        }
        out.push(value);
    }
    Ok(out)
}
