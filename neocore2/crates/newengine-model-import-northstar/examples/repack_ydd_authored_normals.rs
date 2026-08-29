use flate2::{write::DeflateEncoder, Compression};
use newengine_assets_api::{encode_list_file, ListFileEncodeRequest};
use newengine_math::{Mat4, Vec3};
use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};
use std::{env, fs, io::Write, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let input = PathBuf::from(args.next().ok_or("input ydd required")?);
    let logical = args.next().ok_or("logical ydd path required")?;
    let source_pak = PathBuf::from(args.next().ok_or("source pak required")?);
    let output = PathBuf::from(args.next().ok_or("output ydd required")?);

    let original_bytes = fs::read(&input).map_err(|e| format!("read input: {e}"))?;
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &original_bytes,
        newengine_asset_format_nef8::ydd::CONTENT_KIND,
        &logical,
    )?;
    let mut document =
        newengine_asset_format_nef8::ydd_binary::decode_ydd_binary_body(&decoded.body)?;
    if document.entries.len() != 1 {
        return Err(format!(
            "expected one YDD entry, got {}",
            document.entries.len()
        ));
    }
    let original_document = document.clone();

    let pak = PakFile::parse(fs::read(&source_pak).map_err(|e| format!("read source pak: {e}"))?)?;
    let mut source = decode_geometry_lod0(&pak)?;
    let entry = &mut document.entries[0];
    let source_to_model = Mat4::from_cols_array(&entry.skin_source_to_model.unwrap_or([
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]));
    for mesh in &mut source.meshes {
        for vertex in &mut mesh.vertices {
            let position = source_to_model.transform_point3(Vec3::new(
                vertex.position[0],
                vertex.position[1],
                vertex.position[2],
            ));
            let normal = source_to_model
                .transform_vector3(Vec3::new(
                    vertex.normal[0],
                    vertex.normal[1],
                    vertex.normal[2],
                ))
                .normalize_or_zero();
            if !position.is_finite() || !normal.is_finite() || normal.length_squared() <= 1.0e-12 {
                return Err(format!(
                    "source_to_model produced invalid vertex mesh='{}'",
                    mesh.name
                ));
            }
            vertex.position = [position.x, position.y, position.z];
            vertex.normal = [normal.x, normal.y, normal.z];
        }
    }
    let mut patched = 0usize;
    let mut changed_normals = 0usize;

    for source_mesh in &source.meshes {
        let Some(target) = entry
            .meshes
            .iter_mut()
            .find(|mesh| mesh.name == source_mesh.name)
        else {
            continue;
        };
        if target.vertices.len() != source_mesh.vertices.len() {
            return Err(format!(
                "vertex count mismatch mesh='{}' ydd={} source={}",
                target.name,
                target.vertices.len(),
                source_mesh.vertices.len()
            ));
        }
        if target.indices != source_mesh.indices {
            return Err(format!("index buffer mismatch mesh='{}'", target.name));
        }
        for (target_vertex, source_vertex) in target.vertices.iter_mut().zip(&source_mesh.vertices)
        {
            if target_vertex.position != source_vertex.position
                || target_vertex.uv0 != source_vertex.uv0
            {
                return Err(format!("position/uv mismatch mesh='{}'", target.name));
            }
            if target_vertex.normal != source_vertex.normal {
                changed_normals += 1;
                target_vertex.normal = source_vertex.normal;
            }
        }
        patched += 1;
    }
    if patched == 0 {
        return Err("source pak matched no YDD meshes".into());
    }

    // Everything except per-vertex normals on matched meshes must remain identical.
    let original_entry = &original_document.entries[0];
    if entry.name != original_entry.name
        || entry.source_path != original_entry.source_path
        || entry.properties_ref != original_entry.properties_ref
        || entry.bounds_min != original_entry.bounds_min
        || entry.bounds_max != original_entry.bounds_max
        || entry.skin_source_to_model != original_entry.skin_source_to_model
        || entry.meshes.len() != original_entry.meshes.len()
    {
        return Err("YDD entry metadata changed during normal repack".into());
    }
    for (before, after) in original_entry.meshes.iter().zip(&entry.meshes) {
        if before.name != after.name
            || before.material_ref != after.material_ref
            || before.bounds_min != after.bounds_min
            || before.bounds_max != after.bounds_max
            || before.indices != after.indices
            || before.skin != after.skin
            || before.vertices.len() != after.vertices.len()
        {
            return Err(format!(
                "non-normal mesh contract changed mesh='{}'",
                before.name
            ));
        }
        for (a, b) in before.vertices.iter().zip(&after.vertices) {
            if a.position != b.position || a.uv0 != b.uv0 {
                return Err(format!(
                    "non-normal vertex data changed mesh='{}'",
                    before.name
                ));
            }
        }
    }

    let body = newengine_asset_format_nef8::ydd_binary::encode_ydd_binary_body(&document)?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&body).map_err(|e| e.to_string())?;
    let stored = encoder.finish().map_err(|e| e.to_string())?;
    let output_bytes = encode_list_file(ListFileEncodeRequest {
        content_kind: decoded.header.content_kind,
        content_schema_version: decoded.header.content_schema_version,
        entry_count: decoded.header.entry_count,
        additional_flags: 0,
        min_size_class: decoded.header.size_class.max(5),
        header_metadata: &[],
        body_stored: &stored,
        body_uncompressed_len: body.len() as u64,
        body_raw_hash: None,
        stable_file_id: decoded
            .header
            .has_stable_file_id()
            .then_some(decoded.header.stable_file_id),
        import_settings_hash: decoded
            .header
            .has_import_settings_hash()
            .then_some(decoded.header.import_settings_hash),
    })?;

    // Round-trip the produced artifact before publishing it.
    let verify = newengine_assets_api::decode_list_file_envelope(
        &output_bytes,
        newengine_asset_format_nef8::ydd::CONTENT_KIND,
        &logical,
    )?;
    let verify_doc = newengine_asset_format_nef8::ydd_binary::decode_ydd_binary_body(&verify.body)?;
    if verify_doc.entries.len() != 1
        || verify_doc.entries[0].meshes.len() != document.entries[0].meshes.len()
    {
        return Err("repacked YDD failed round-trip contract".into());
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&output, output_bytes).map_err(|e| format!("write output: {e}"))?;
    println!(
        "YDD_AUTHORED_NORMAL_REPACK_OK matched_meshes={} changed_normals={} total_meshes={} output='{}'",
        patched,
        changed_normals,
        document.entries[0].meshes.len(),
        output.display()
    );
    Ok(())
}
