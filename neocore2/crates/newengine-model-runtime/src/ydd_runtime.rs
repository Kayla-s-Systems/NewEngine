use newengine_assets_api::{AssetDecodeRequest, AssetServiceClient};
use newengine_math::Vec3;
use newengine_plugin_host::default_host_api;
use newengine_primitives::{fnv1a_64, PrimitiveId, PrimitiveMesh, PrimitiveVertex};

#[derive(Clone, Debug)]
pub struct DecodedRuntimeYddMeshPart {
    pub primitive_id: PrimitiveId,
    pub name: String,
    pub material_slot: String,
    pub material_ref: Option<String>,
    pub mesh: PrimitiveMesh,
}

use abi_stable::std_types::RString;
use newengine_asset_format_nef8::ydd_binary::{
    decode_ydd_binary_entries, YddBinaryEntry, YddBinaryVertex,
};
use newengine_plugin_api::{Blob, MethodName};
use newengine_render_api::{
    decode_multi_adapter_mesh_transcode_result, encode_multi_adapter_mesh_transcode_request,
    MultiAdapterMeshTranscodeRequest, RENDER_SERVICE_ID,
    RENDER_SERVICE_METHOD_MULTI_ADAPTER_MESH_TRANSCODE_BIN_V1,
};
use std::sync::atomic::{AtomicBool, Ordering};

static MULTI_ADAPTER_SERVICE_DISABLED: AtomicBool = AtomicBool::new(false);

pub fn recompute_ydd_mesh_bounds(mesh: &mut PrimitiveMesh) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for vertex in &mesh.vertices {
        let point = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
        min = min.min(point);
        max = max.max(point);
    }
    if mesh.vertices.is_empty() || !min.is_finite() || !max.is_finite() {
        mesh.bounds_center = Vec3::ZERO;
        mesh.bounds_radius = 0.001;
        return;
    }
    mesh.bounds_center = (min + max) * 0.5;
    mesh.bounds_radius = mesh
        .vertices
        .iter()
        .map(|vertex| {
            let point = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
            (point - mesh.bounds_center).length()
        })
        .fold(0.001_f32, f32::max);
}

fn multi_adapter_mesh_min_bytes() -> usize {
    newengine_runtime_env::var_usize(
        "NEWENGINE_MULTI_ADAPTER_MESH_MIN_BYTES",
        64 * 1024,
        4 * 1024,
        128 * 1024 * 1024,
    )
}

fn pack_ydd_vertices(vertices: &[YddBinaryVertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vertices.len().saturating_mul(32));
    for vertex in vertices {
        for value in vertex
            .position
            .iter()
            .chain(vertex.normal.iter())
            .chain(vertex.uv0.iter())
        {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

fn primitive_vertices_from_bytes(bytes: &[u8]) -> Result<Vec<PrimitiveVertex>, String> {
    if !bytes.len().is_multiple_of(32) {
        return Err(format!(
            "multi-adapter vertex response is not 32-byte aligned bytes={}",
            bytes.len()
        ));
    }
    fn read_f32(bytes: &[u8], offset: usize) -> f32 {
        f32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    }
    Ok(bytes
        .as_chunks::<32>()
        .0
        .iter()
        .map(|record| PrimitiveVertex {
            pos: [
                read_f32(record, 0),
                read_f32(record, 4),
                read_f32(record, 8),
            ],
            nrm: [
                read_f32(record, 12),
                read_f32(record, 16),
                read_f32(record, 20),
            ],
            uv: [read_f32(record, 24), read_f32(record, 28)],
        })
        .collect())
}

fn cpu_primitive_vertices(vertices: &[YddBinaryVertex]) -> Vec<PrimitiveVertex> {
    vertices
        .iter()
        .map(|vertex| PrimitiveVertex {
            pos: vertex.position,
            nrm: vertex.normal,
            uv: vertex.uv0,
        })
        .collect()
}

fn transcode_ydd_vertices(
    logical_ref: &str,
    mesh_name: &str,
    vertices: &[YddBinaryVertex],
) -> Vec<PrimitiveVertex> {
    let bytes = pack_ydd_vertices(vertices);
    if bytes.len() < multi_adapter_mesh_min_bytes()
        || MULTI_ADAPTER_SERVICE_DISABLED.load(Ordering::Relaxed)
    {
        return cpu_primitive_vertices(vertices);
    }

    let request = match MultiAdapterMeshTranscodeRequest::new(bytes) {
        Ok(request) => request,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "model runtime: multi-adapter request rejected path='{}' mesh='{}' err='{}'; using CPU vertex stream",
                logical_ref,
                mesh_name,
                error
            );
            return cpu_primitive_vertices(vertices);
        }
    };
    let packet = match encode_multi_adapter_mesh_transcode_request(&request) {
        Ok(packet) => packet,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "model runtime: multi-adapter request encode failed path='{}' mesh='{}' err='{}'; using CPU vertex stream",
                logical_ref,
                mesh_name,
                error
            );
            return cpu_primitive_vertices(vertices);
        }
    };

    let host = default_host_api();
    let response = (host.call_service_v1)(
        RString::from(RENDER_SERVICE_ID),
        MethodName::from(RENDER_SERVICE_METHOD_MULTI_ADAPTER_MESH_TRANSCODE_BIN_V1),
        Blob::from(packet),
    )
    .into_result();
    let response = match response {
        Ok(response) => response.into_vec(),
        Err(error) => {
            let detail = error.to_string();
            if detail.contains("unavailable")
                || detail.contains("unknown method")
                || detail.contains("unknown service")
                || detail.contains("not found")
            {
                if !MULTI_ADAPTER_SERVICE_DISABLED.swap(true, Ordering::Relaxed) {
                    newengine_ulog_api::ulog::info!(
                        "model runtime: independent multi-adapter mesh transcode unavailable; CPU fallback active err='{}'",
                        detail
                    );
                }
            } else {
                newengine_ulog_api::ulog::warn!(
                    "model runtime: multi-adapter mesh transcode failed path='{}' mesh='{}' err='{}'; using CPU vertex stream",
                    logical_ref,
                    mesh_name,
                    detail
                );
            }
            return cpu_primitive_vertices(vertices);
        }
    };
    let result = match decode_multi_adapter_mesh_transcode_result(&response) {
        Ok(result) => result,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "model runtime: multi-adapter response decode failed path='{}' mesh='{}' err='{}'; using CPU vertex stream",
                logical_ref,
                mesh_name,
                error
            );
            return cpu_primitive_vertices(vertices);
        }
    };
    if result.vertex_count() != vertices.len() {
        newengine_ulog_api::ulog::warn!(
            "model runtime: multi-adapter vertex count mismatch path='{}' mesh='{}' expected={} actual={}; using CPU vertex stream",
            logical_ref,
            mesh_name,
            vertices.len(),
            result.vertex_count()
        );
        return cpu_primitive_vertices(vertices);
    }
    match primitive_vertices_from_bytes(&result.vertex_bytes) {
        Ok(vertices) => {
            newengine_ulog_api::ulog::info!(
                "model runtime: multi-adapter vertex stream ready path='{}' mesh='{}' worker={} vertices={} bytes={} repaired={} gpu_ns={}",
                logical_ref,
                mesh_name,
                result.worker_index,
                vertices.len(),
                result.vertex_bytes.len(),
                result.invalid_vertex_count,
                result.gpu_elapsed_ns,
            );
            vertices
        }
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "model runtime: multi-adapter vertex response invalid path='{}' mesh='{}' err='{}'; using CPU vertex stream",
                logical_ref,
                mesh_name,
                error
            );
            cpu_primitive_vertices(vertices)
        }
    }
}

pub fn ydd_dictionary_ref(logical_ref: &str) -> Result<String, String> {
    let normalized = logical_ref.trim().replace('\\', "/");
    let (dictionary, selector) = normalized.rsplit_once('@').ok_or_else(|| {
        format!("runtime YDD reference requires '.ydd@entry' path='{logical_ref}'")
    })?;
    if selector.trim().is_empty() {
        return Err(format!(
            "runtime YDD reference has empty selector path='{logical_ref}'"
        ));
    }
    if !dictionary.to_ascii_lowercase().ends_with(".ydd") {
        return Err(format!(
            "runtime YDD reference has non-YDD dictionary path='{logical_ref}'"
        ));
    }
    Ok(dictionary.to_owned())
}

fn decode_runtime_ydd_entry(
    logical_ref: &str,
    entry: &YddBinaryEntry,
) -> Result<Vec<DecodedRuntimeYddMeshPart>, String> {
    let mut parts = Vec::with_capacity(entry.meshes.len());
    for (mesh_index, source_mesh) in entry.meshes.iter().enumerate() {
        let material_ref = source_mesh
            .material_ref
            .clone()
            .or_else(|| entry.properties_ref.clone());
        let material_slot = source_mesh.material_slot();
        let vertices =
            transcode_ydd_vertices(logical_ref, &source_mesh.name, &source_mesh.vertices);
        let primitive_id = PrimitiveId(fnv1a_64(&format!(
            "newengine.prefab.ydd:{logical_ref}#entry:{}:mesh:{mesh_index}:slot:{material_slot}",
            entry.name
        )));
        let mut mesh = PrimitiveMesh {
            vertices,
            indices: source_mesh.indices.clone(),
            bounds_center: Vec3::ZERO,
            bounds_radius: 0.001,
        };
        recompute_ydd_mesh_bounds(&mut mesh);
        parts.push(DecodedRuntimeYddMeshPart {
            primitive_id,
            name: source_mesh.name.clone(),
            material_slot,
            material_ref,
            mesh,
        });
    }
    if parts.is_empty() {
        return Err(format!(
            "binary YDD contains no runtime mesh parts path='{logical_ref}'"
        ));
    }
    Ok(parts)
}

/// Decode a set of `.ydd@entry` references that all belong to the same physical
/// dictionary. The AssetManager body and binary YDD document are materialized exactly
/// once; selectors are resolved from that shared document afterwards.
pub fn decode_runtime_ydd_prefabs(
    logical_refs: &[String],
) -> Result<std::collections::BTreeMap<String, Vec<DecodedRuntimeYddMeshPart>>, String> {
    let Some(first_ref) = logical_refs.first() else {
        return Ok(std::collections::BTreeMap::new());
    };
    let dictionary = ydd_dictionary_ref(first_ref)?;
    let mut sources = std::collections::BTreeSet::new();
    for logical_ref in logical_refs {
        let normalized = logical_ref.trim().replace('\\', "/");
        let source_dictionary = ydd_dictionary_ref(&normalized)?;
        if !source_dictionary.eq_ignore_ascii_case(&dictionary) {
            return Err(format!(
                "runtime YDD batch mixes dictionaries expected='{}' actual='{}' source='{}'",
                dictionary, source_dictionary, normalized
            ));
        }
        sources.insert(normalized);
    }

    let assets = AssetServiceClient::new(default_host_api());
    let body = assets
        .decode_v1(&AssetDecodeRequest {
            logical_path: dictionary.clone(),
            output_kind: "asset.list_file_body".to_owned(),
            selector: serde_json::Value::Null,
            format_descriptor: None,
        })
        .map_err(|error| {
            format!("AssetManager decode .ydd body failed dictionary='{dictionary}' err='{error}'")
        })?;
    let selectors = sources
        .iter()
        .filter_map(|logical_ref| {
            logical_ref
                .rsplit_once('@')
                .map(|(_, selector)| selector.trim().to_owned())
        })
        .collect::<Vec<_>>();
    let document = decode_ydd_binary_entries(&body, &selectors).map_err(|error| {
        format!("binary YDD selective decode failed dictionary='{dictionary}' err='{error}'")
    })?;

    let mut decoded = std::collections::BTreeMap::new();
    for logical_ref in sources {
        let selector = logical_ref
            .rsplit_once('@')
            .map(|(_, selector)| selector.trim())
            .filter(|selector| !selector.is_empty());
        let entry = document.select_entry(selector, false).map_err(|error| {
            format!("binary YDD selection failed path='{logical_ref}' err='{error}'")
        })?;
        decoded.insert(
            logical_ref.clone(),
            decode_runtime_ydd_entry(&logical_ref, entry)?,
        );
    }

    newengine_ulog_api::ulog::info!(
        "model runtime: binary ydd dictionary decoded dictionary='{}' selectors={} body_bytes={} encoding='{}' policy='one physical YDD read -> selected entry payloads only'",
        dictionary,
        decoded.len(),
        body.len(),
        newengine_asset_format_nef8::ydd_binary::YDD_BINARY_ENCODING,
    );
    Ok(decoded)
}

pub fn decode_runtime_ydd_prefab(
    logical_ref: &str,
) -> Result<Vec<DecodedRuntimeYddMeshPart>, String> {
    let normalized = logical_ref.trim().replace('\\', "/");
    let mut decoded = decode_runtime_ydd_prefabs(std::slice::from_ref(&normalized))?;
    decoded
        .remove(&normalized)
        .ok_or_else(|| format!("binary YDD batch omitted requested source path='{normalized}'"))
}

#[cfg(test)]
mod dictionary_ref_tests {
    use super::ydd_dictionary_ref;

    #[test]
    fn dictionary_ref_strips_selector_and_normalizes_separators() {
        let dictionary =
            ydd_dictionary_ref(r"models\maps\stadium.ydd@mesh_001").expect("valid YDD entry ref");
        assert_eq!(dictionary, "models/maps/stadium.ydd");
    }

    #[test]
    fn dictionary_ref_rejects_missing_or_empty_selector() {
        assert!(ydd_dictionary_ref("models/maps/stadium.ydd").is_err());
        assert!(ydd_dictionary_ref("models/maps/stadium.ydd@").is_err());
    }
}
