pub fn decode_geometry_lod0(pak: &PakFile) -> Result<DecodedGeometry, String> {
    let resource = pak
        .resource("GEOMETRY_1")
        .ok_or_else(|| "package contains no GEOMETRY_1 resource".to_owned())?;
    let payload = pak.resource_payload(resource)?;
    let submesh_count = pak.read_u32(payload + 8)? as usize;
    if submesh_count == 0 || submesh_count > 100_000 {
        return Err(format!("invalid geometry submesh count {submesh_count}"));
    }
    let submesh_table = pak
        .resolve_pointer(payload + 40)?
        .ok_or_else(|| "GEOMETRY_1 has no submesh table".to_owned())?;
    let stride = detect_submesh_stride(pak, submesh_table, submesh_count)?;
    let count_offset = if stride == PC_SUBMESH_STRIDE {
        136
    } else {
        128
    };

    let mut result = DecodedGeometry::default();
    for index in 0..submesh_count {
        let sub = submesh_table
            .checked_add(index * stride)
            .ok_or("submesh address overflow")?;
        let name_pointer = pak
            .resolve_pointer(sub + 32)?
            .ok_or_else(|| format!("submesh {index} has no name pointer"))?;
        let raw_name = pak.string_at(name_pointer)?;
        let name = raw_name.rsplit('|').next().unwrap_or(&raw_name).to_owned();
        if lod_index(&name) != 0 {
            continue;
        }

        let vertex_count = pak.read_u32(sub + count_offset)? as usize;
        let index_count = pak.read_u32(sub + count_offset + 4)? as usize;
        let stream_count = pak.read_u32(sub + count_offset + 8)? as usize;
        if vertex_count == 0 || index_count == 0 || !index_count.is_multiple_of(3) {
            return Err(format!(
                "invalid source submesh geometry name='{name}' vertices={vertex_count} indices={index_count}"
            ));
        }
        if vertex_count > 10_000_000 || index_count > 60_000_000 || stream_count > 32 {
            return Err(format!(
                "source submesh exceeds importer limits name='{name}' vertices={vertex_count} indices={index_count} streams={stream_count}"
            ));
        }
        let stream_table = pak
            .resolve_pointer(sub + 48)?
            .ok_or_else(|| format!("submesh '{name}' has no stream table"))?;
        let index_buffer = pak
            .resolve_pointer(sub + 64)?
            .ok_or_else(|| format!("submesh '{name}' has no index buffer"))?;
        let skin_header = pak.resolve_pointer(sub + 88)?;
        let source_skin_joint_domain_size = if skin_header.is_some() && stride == PC_SUBMESH_STRIDE
        {
            let domain_size = pak.read_u32(sub + 152)? as usize;
            if domain_size == 0 || domain_size > 100_000 {
                return Err(format!(
                    "invalid native skin joint domain name='{name}' size={domain_size}"
                ));
            }
            Some(domain_size)
        } else {
            None
        };
        let material_header = pak.resolve_pointer(sub + 72)?;

        let streams = (0..stream_count)
            .map(|stream_index| {
                decode_stream_desc(pak, stream_table + stream_index * STREAM_DESC_STRIDE)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let position_stream = streams
            .iter()
            .find(|stream| stream.kind == 64 || stream.kind == 0)
            .ok_or_else(|| {
                format!("submesh '{name}' has no supported NorthStar position stream type=64/0")
            })?;
        let positions = match position_stream.kind {
            64 => decode_quantized_stream(pak, position_stream, vertex_count, 3)?,
            0 => decode_raw_f32_stream(pak, position_stream, vertex_count, 3)?,
            _ => unreachable!(),
        };
        let uv0 = streams
            .iter()
            .find(|stream| stream.kind == 65 || stream.kind == 1)
            .map(|stream| match stream.kind {
                65 => decode_quantized_stream(pak, stream, vertex_count, 2),
                1 => decode_raw_f16_stream(pak, stream, vertex_count, 2),
                _ => unreachable!(),
            })
            .transpose()?
            .unwrap_or_else(|| vec![[0.0, 0.0, 0.0, 0.0]; vertex_count]);
        let source_indices = decode_indices(pak, index_buffer, index_count, vertex_count, &name)?;
        // source packages can retain dead source vertices after mesh partitioning. Some of
        // those vertices intentionally have no skin weight record. They are not renderable data:
        // compact strictly to vertices referenced by the triangle index buffer before skin decode.
        // A zero-weight vertex that is actually referenced still fails in `decode_skin` below.
        let (positions, uv0, mut indices, source_vertex_indices) =
            compact_indexed_vertex_streams(&positions, &uv0, &source_indices, &name)?;
        // NorthStar PC packages encode triangle winding opposite to NewEngine's canonical
        // clockwise framebuffer-front-face convention. Preserve the source vertex/skin
        // streams, but canonicalize every triangle before deriving normals or publishing YDD.
        // Without this conversion skinned exterior shells are classified as back faces and
        // disappear under normal back-face culling, which looks like transparent skin.
        reverse_northstar_triangle_winding(&mut indices);
        let normals = recalculate_normals(&positions, &indices);
        let (skin, skin_loss) = match skin_header {
            Some(header) => {
                let (skin, stats) = decode_skin(pak, header, &source_vertex_indices, &name)?;
                (Some(skin), stats)
            }
            None => (None, SkinLossStats::default()),
        };
        result.skin_loss.merge(skin_loss);

        let vertices = positions
            .iter()
            .zip(normals.iter())
            .zip(uv0.iter())
            .map(|((position, normal), uv)| YddBinaryVertex {
                position: [position[0], position[1], position[2]],
                normal: *normal,
                uv0: [uv[0], uv[1]],
            })
            .collect::<Vec<_>>();
        let source_material = material_header
            .and_then(|material| pak.resolve_pointer(material).ok().flatten())
            .and_then(|name_ptr| pak.string_at(name_ptr).ok());

        result.meshes.push(ImportMesh {
            name,
            source_material,
            bounds_min: [
                pak.read_f32(sub)?,
                pak.read_f32(sub + 4)?,
                pak.read_f32(sub + 8)?,
            ],
            bounds_max: [
                pak.read_f32(sub + 16)?,
                pak.read_f32(sub + 20)?,
                pak.read_f32(sub + 24)?,
            ],
            vertices,
            skin,
            source_skin_joint_domain_size,
            skin_loss,
            indices,
        });
    }
    if result.meshes.is_empty() {
        return Err("GEOMETRY_1 contains no LOD0 submeshes".to_owned());
    }
    Ok(result)
}
