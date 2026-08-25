use std::collections::BTreeMap;

use newengine_asset_format_nef8::ydd_binary::{YddBinarySkinVertex, YddBinaryVertex};
use newengine_math::Vec3;

use crate::pak::PakFile;

const PC_SUBMESH_STRIDE: usize = 192;
const LEGACY_TLOU2_SUBMESH_STRIDE: usize = 176;
const STREAM_DESC_STRIDE: usize = 64;
const PACKED_WEIGHT_MASK: u32 = (1 << 22) - 1;
const PACKED_WEIGHT_DENOMINATOR: f32 = PACKED_WEIGHT_MASK as f32;

#[derive(Clone, Copy, Debug, Default)]
pub struct SkinLossStats {
    pub weighted_vertices: u64,
    pub source_influences: u64,
    pub max_source_influences: u32,
    pub top4_loss_sum: f64,
    pub top4_loss_max: f32,
    pub top8_loss_sum: f64,
    pub top8_loss_max: f32,
}

impl SkinLossStats {
    pub fn merge(&mut self, other: Self) {
        self.weighted_vertices += other.weighted_vertices;
        self.source_influences += other.source_influences;
        self.max_source_influences = self.max_source_influences.max(other.max_source_influences);
        self.top4_loss_sum += other.top4_loss_sum;
        self.top4_loss_max = self.top4_loss_max.max(other.top4_loss_max);
        self.top8_loss_sum += other.top8_loss_sum;
        self.top8_loss_max = self.top8_loss_max.max(other.top8_loss_max);
    }

    pub fn average_source_influences(&self) -> f64 {
        if self.weighted_vertices == 0 {
            0.0
        } else {
            self.source_influences as f64 / self.weighted_vertices as f64
        }
    }

    pub fn average_top4_loss(&self) -> f64 {
        if self.weighted_vertices == 0 {
            0.0
        } else {
            self.top4_loss_sum / self.weighted_vertices as f64
        }
    }

    pub fn average_top8_loss(&self) -> f64 {
        if self.weighted_vertices == 0 {
            0.0
        } else {
            self.top8_loss_sum / self.weighted_vertices as f64
        }
    }
}

#[derive(Clone, Debug)]
pub struct ImportMesh {
    pub name: String,
    pub source_material: Option<String>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub vertices: Vec<YddBinaryVertex>,
    pub skin: Option<Vec<YddBinarySkinVertex>>,
    /// Size of the native skin joint domain declared by this submesh. For ordinary
    /// character geometry this matches the master JOINT_HIERARCHY size. Cloth-backed
    /// geometry can instead address a package-local simulation-node domain and must
    /// never be interpreted as master skeleton indices.
    pub source_skin_joint_domain_size: Option<usize>,
    pub skin_loss: SkinLossStats,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct DecodedGeometry {
    pub meshes: Vec<ImportMesh>,
    pub skin_loss: SkinLossStats,
}

#[derive(Clone, Debug)]
struct StreamDesc {
    kind: u8,
    buffer: usize,
    buffer_size: usize,
    num_vertices: usize,
    sizes: [u8; 4],
    q_scale: [f32; 4],
    q_offset: [f32; 4],
}

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
        if vertex_count == 0 || index_count == 0 || index_count % 3 != 0 {
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
                format!("submesh '{name}' has no supported TLOU2 position stream type=64/0")
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
        // Naughty Dog packages can retain dead source vertices after mesh partitioning. Some of
        // those vertices intentionally have no skin weight record. They are not renderable data:
        // compact strictly to vertices referenced by the triangle index buffer before skin decode.
        // A zero-weight vertex that is actually referenced still fails in `decode_skin` below.
        let (positions, uv0, indices, source_vertex_indices) =
            compact_indexed_vertex_streams(&positions, &uv0, &source_indices, &name)?;
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

fn detect_submesh_stride(pak: &PakFile, table: usize, count: usize) -> Result<usize, String> {
    let mut best = (0usize, 0usize);
    for stride in [PC_SUBMESH_STRIDE, LEGACY_TLOU2_SUBMESH_STRIDE] {
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
        Err("unable to determine TLOU2 submesh record stride".to_owned())
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
        for component in 0..wanted_components {
            let at = base + component * 4;
            value[component] =
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
        for component in 0..wanted_components {
            let at = base + component * 2;
            let bits = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
            value[component] = f16_to_f32(bits);
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
        for component in 0..4 {
            let width = stream.sizes[component] as usize;
            if width > 32 {
                return Err(format!("unsupported quantized component width {width}"));
            }
            if width != 0 {
                value[component] = bits.read(width)? as f32 * stream.q_scale[component]
                    + stream.q_offset[component];
            } else if stream.kind == 64 && component < 3 {
                value[component] = stream.q_scale[component] + stream.q_offset[component];
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

fn compact_indexed_vertex_streams(
    positions: &[[f32; 4]],
    uv0: &[[f32; 4]],
    source_indices: &[u32],
    mesh_name: &str,
) -> Result<(Vec<[f32; 4]>, Vec<[f32; 4]>, Vec<u32>, Vec<usize>), String> {
    if positions.len() != uv0.len() {
        return Err(format!(
            "vertex stream length mismatch mesh='{mesh_name}' positions={} uv0={}",
            positions.len(),
            uv0.len()
        ));
    }
    let mut referenced = vec![false; positions.len()];
    for &index in source_indices {
        let source = usize::try_from(index).map_err(|_| {
            format!("source index conversion failed mesh='{mesh_name}' index={index}")
        })?;
        let Some(flag) = referenced.get_mut(source) else {
            return Err(format!(
                "source index outside vertex stream mesh='{mesh_name}' index={source} vertices={}",
                positions.len()
            ));
        };
        *flag = true;
    }
    let source_vertex_indices = referenced
        .iter()
        .enumerate()
        .filter_map(|(index, used)| used.then_some(index))
        .collect::<Vec<_>>();
    if source_vertex_indices.is_empty() {
        return Err(format!(
            "indexed mesh references no vertices mesh='{mesh_name}'"
        ));
    }
    let mut remap = vec![u32::MAX; positions.len()];
    let mut compact_positions = Vec::with_capacity(source_vertex_indices.len());
    let mut compact_uv0 = Vec::with_capacity(source_vertex_indices.len());
    for (dense, &source) in source_vertex_indices.iter().enumerate() {
        remap[source] = u32::try_from(dense)
            .map_err(|_| format!("dense vertex index overflow mesh='{mesh_name}'"))?;
        compact_positions.push(positions[source]);
        compact_uv0.push(uv0[source]);
    }
    let indices = source_indices
        .iter()
        .map(|&source| {
            let source = usize::try_from(source)
                .map_err(|_| format!("source index conversion failed mesh='{mesh_name}'"))?;
            remap
                .get(source)
                .copied()
                .filter(|value| *value != u32::MAX)
                .ok_or_else(|| {
                    format!("source index was not remapped mesh='{mesh_name}' index={source}")
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        compact_positions,
        compact_uv0,
        indices,
        source_vertex_indices,
    ))
}

fn decode_skin(
    pak: &PakFile,
    header: usize,
    source_vertex_indices: &[usize],
    mesh_name: &str,
) -> Result<(Vec<YddBinarySkinVertex>, SkinLossStats), String> {
    let map = pak
        .resolve_pointer(header + 16)?
        .ok_or_else(|| format!("skin map missing mesh='{mesh_name}'"))?;
    let weights = pak
        .resolve_pointer(header + 24)?
        .ok_or_else(|| format!("skin weights missing mesh='{mesh_name}'"))?;
    let profile = pak.read_u32(header + 8)?;
    if profile > 1 {
        return Err(format!(
            "unsupported source skin profile mesh='{mesh_name}' profile={profile}"
        ));
    }
    let mut out = Vec::with_capacity(source_vertex_indices.len());
    let mut stats = SkinLossStats::default();
    for &vertex in source_vertex_indices {
        let count = pak.read_u32(map + vertex * 8)? as usize;
        let relative = pak.read_u32(map + vertex * 8 + 4)? as usize;
        if count == 0 || count > 12 {
            return Err(format!(
                "unsupported source skin influence count mesh='{mesh_name}' vertex={vertex} count={count}"
            ));
        }
        let mut combined = BTreeMap::<u16, f32>::new();
        for influence in 0..count {
            let base = weights
                .checked_add(relative)
                .ok_or("skin weight address overflow")?;
            let (joint, weight) = match profile {
                0 => {
                    let packed = pak.read_u32(
                        base.checked_add(influence * 4)
                            .ok_or("packed skin weight address overflow")?,
                    )?;
                    (
                        (packed >> 22) as u16,
                        (packed & PACKED_WEIGHT_MASK) as f32 / PACKED_WEIGHT_DENOMINATOR,
                    )
                }
                1 => {
                    // TLOU2 PC also uses an explicit 8-byte influence representation:
                    // f32 weight followed by u32 joint index. The profile bit at skin_header+8
                    // selects this layout. Treating these words as the packed 22/10-bit profile
                    // corrupts both weights and joints (notably Ellie backpack cloth/straps).
                    let influence_base = base
                        .checked_add(influence * 8)
                        .ok_or("explicit skin influence address overflow")?;
                    let weight = pak.read_f32(influence_base)?;
                    let joint = pak.read_u32(influence_base + 4)?;
                    let joint = u16::try_from(joint).map_err(|_| {
                        format!(
                            "explicit source skin joint exceeds u16 mesh='{mesh_name}' vertex={vertex} joint={joint}"
                        )
                    })?;
                    (joint, weight)
                }
                _ => unreachable!(),
            };
            if !weight.is_finite() || weight < 0.0 {
                return Err(format!(
                    "invalid source skin influence mesh='{mesh_name}' vertex={vertex} joint={joint} weight={weight}"
                ));
            }
            *combined.entry(joint).or_insert(0.0) += weight;
        }
        let mut influences = combined.into_iter().collect::<Vec<_>>();
        influences.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.0.cmp(&right.0))
        });
        let total = influences.iter().map(|(_, weight)| *weight).sum::<f32>();
        if !total.is_finite() || total <= 1.0e-8 {
            return Err(format!(
                "invalid source skin total mesh='{mesh_name}' vertex={vertex} total={total}"
            ));
        }
        let top4 = influences
            .iter()
            .take(4)
            .map(|(_, weight)| *weight)
            .sum::<f32>();
        let top8 = influences
            .iter()
            .take(8)
            .map(|(_, weight)| *weight)
            .sum::<f32>();
        let loss4 = (1.0 - top4 / total).clamp(0.0, 1.0);
        let loss8 = (1.0 - top8 / total).clamp(0.0, 1.0);
        stats.weighted_vertices += 1;
        stats.source_influences += influences.len() as u64;
        stats.max_source_influences = stats.max_source_influences.max(influences.len() as u32);
        stats.top4_loss_sum += loss4 as f64;
        stats.top4_loss_max = stats.top4_loss_max.max(loss4);
        stats.top8_loss_sum += loss8 as f64;
        stats.top8_loss_max = stats.top8_loss_max.max(loss8);

        let retained = top8.max(1.0e-8);
        let mut joints = [0u16; 8];
        let mut normalized = [0.0f32; 8];
        for (slot, (joint, weight)) in influences.into_iter().take(8).enumerate() {
            joints[slot] = joint;
            normalized[slot] = weight / retained;
        }
        out.push(YddBinarySkinVertex {
            joints: [joints[0], joints[1], joints[2], joints[3]],
            weights: [normalized[0], normalized[1], normalized[2], normalized[3]],
            joints_extra: [joints[4], joints[5], joints[6], joints[7]],
            weights_extra: [normalized[4], normalized[5], normalized[6], normalized[7]],
        });
    }
    Ok((out, stats))
}

fn recalculate_normals(positions: &[[f32; 4]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for triangle in indices.chunks_exact(3) {
        let a = vec3(positions[triangle[0] as usize]);
        let b = vec3(positions[triangle[1] as usize]);
        let c = vec3(positions[triangle[2] as usize]);
        let face = (b - a).cross(c - a);
        if face.length_squared() > 1.0e-18 && face.is_finite() {
            normals[triangle[0] as usize] += face;
            normals[triangle[1] as usize] += face;
            normals[triangle[2] as usize] += face;
        }
    }
    normals
        .into_iter()
        .map(|normal| {
            let normal = normal.normalize_or_zero();
            if normal.length_squared() <= 1.0e-12 {
                [0.0, 1.0, 0.0]
            } else {
                [normal.x, normal.y, normal.z]
            }
        })
        .collect()
}

#[inline]
fn vec3(value: [f32; 4]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

struct LsbBitReader<'a> {
    bytes: &'a [u8],
    bit: usize,
}

impl<'a> LsbBitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, bit: 0 }
    }

    fn read(&mut self, count: usize) -> Result<u32, String> {
        if count == 0 {
            return Ok(0);
        }
        let end = self
            .bit
            .checked_add(count)
            .ok_or("quantized bit offset overflow")?;
        if end > self.bytes.len().saturating_mul(8) {
            return Err("quantized stream ended before declared vertices".to_owned());
        }
        let mut value = 0u32;
        for output_bit in 0..count {
            let source = self.bit + output_bit;
            let bit = (self.bytes[source / 8] >> (source % 8)) & 1;
            value |= u32::from(bit) << output_bit;
        }
        self.bit = end;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsb_reader_matches_tlou2_packing_order() {
        let bytes = [0b1011_0010u8, 0b0000_0011];
        let mut bits = LsbBitReader::new(&bytes);
        assert_eq!(bits.read(4).unwrap(), 0b0010);
        assert_eq!(bits.read(4).unwrap(), 0b1011);
        assert_eq!(bits.read(2).unwrap(), 0b11);
    }

    #[test]
    fn indexed_compaction_discards_only_dead_vertices() {
        let positions = vec![
            [0.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [9.0, 9.0, 9.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
        ];
        let uv0 = positions.clone();
        let (p, uv, indices, source) =
            compact_indexed_vertex_streams(&positions, &uv0, &[0, 1, 3], "test").unwrap();
        assert_eq!(source, vec![0, 1, 3]);
        assert_eq!(indices, vec![0, 1, 2]);
        assert_eq!(p.len(), 3);
        assert_eq!(uv.len(), 3);
        assert_eq!(p[2], positions[3]);
    }

    #[test]
    fn lod_parser_selects_shape_zero() {
        assert_eq!(lod_index("abby_head_lod0_LODShape0_shader0"), 0);
        assert_eq!(lod_index("abby_head_lod0_LODShape3_shader0"), 3);
    }
}
