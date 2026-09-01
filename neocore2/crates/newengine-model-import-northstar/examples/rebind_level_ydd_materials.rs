use flate2::{write::DeflateEncoder, Compression};
use newengine_asset_format_nef8::ydd_binary::{decode_ydd_binary_body, encode_ydd_binary_body};
use newengine_assets_api::{decode_list_file_envelope, encode_list_file, ListFileEncodeRequest};
use std::{collections::BTreeMap, env, fs, io::Write, path::PathBuf};

#[derive(Clone, Debug)]
struct SourceMaterial {
    mesh_name: String,
    source: String,
}

fn material_family(mesh: &str, source: &str) -> Option<&'static str> {
    let mesh_l = mesh.to_ascii_lowercase();
    let source_l = source.to_ascii_lowercase();
    let s = format!("{mesh_l} {source_l}");
    let has = |tokens: &[&str]| tokens.iter().any(|token| s.contains(token));
    if has(&["shadowproxy", "shadow_proxy", "lib-shadowblocker"]) {
        return None;
    }
    if source_l.contains("seattle-wide") {
        if source_l.contains("window") {
            return Some("stadium_glass");
        }
        if source_l.contains("brick") {
            return Some("stadium_brick");
        }
        if ["grass", "dirt", "plant", "ground"]
            .iter()
            .any(|t| source_l.contains(t))
        {
            return Some(
                if source_l.contains("grass") || source_l.contains("plant") {
                    "stadium_turf"
                } else {
                    "stadium_dirt"
                },
            );
        }
        if mesh_l.contains("watershape") {
            return Some("stadium_water");
        }
        return Some("stadium_building");
    }
    if mesh_l.starts_with("watershape") || source_l.contains("ocean-water") {
        return Some("stadium_water");
    }
    if has(&["glass", "transparent"]) {
        return Some("stadium_glass");
    }
    if source_l.contains("fob-seat") || has(&["chair", "/seating/"]) {
        return Some("stadium_seat_plastic");
    }
    if has(&[
        "stadium-graphics",
        "sign",
        "graphic",
        "logo",
        "mural",
        "advert",
        "stadium-text",
        "letter",
        "poster",
        "photo",
    ]) {
        return Some("stadium_signage");
    }
    if has(&["road-line", "marking", "paint-stripe"]) {
        return Some("stadium_markings");
    }
    if has(&["rubber", "tire", "tyre"]) {
        return Some("stadium_rubber");
    }
    if has(&[
        "tarp", "fabric", "cloth", "flag", "clothing", "towel", "canvas", "curtain", "tent",
        "carpet", "blanket",
    ]) {
        return Some("stadium_fabric");
    }
    if has(&["asphalt", "pavement"]) || (s.contains("road") && !s.contains("road-line")) {
        return Some("stadium_asphalt");
    }
    if has(&[
        "concrete", "cinder", "cement", "plaster", "stucco", "tile", "ceiling",
    ]) {
        if has(&["paint", "blue", "white", "stadium", "plaster", "tile"]) {
            return Some("stadium_painted_concrete");
        }
        return Some("stadium_concrete");
    }
    if s.contains("brick") {
        return Some("stadium_brick");
    }
    if has(&["grass", "turf", "fern", "vegetation", "plant"]) {
        return Some("stadium_turf");
    }
    if has(&["dirt", "mud", "soil", "earth"]) {
        return Some("stadium_dirt");
    }
    if has(&["wood", "plywood", "ply-", "plank", "board"]) {
        return Some("stadium_wood");
    }
    if has(&[
        "metal", "steel", "iron", "railing", "pipe", "corrugat", "fence", "chain", "wire",
        "garage", "rebar", "cable",
    ]) {
        if has(&["paint", "blue", "white", "corrugat", "garage"]) {
            return Some("stadium_metal_painted");
        }
        return Some("stadium_metal_raw");
    }
    Some("stadium_misc")
}

fn read_materials(path: &PathBuf) -> Result<BTreeMap<usize, SourceMaterial>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut out = BTreeMap::new();
    for (line_no, line) in text.lines().enumerate().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let cols = line.split('\t').collect::<Vec<_>>();
        if cols.len() < 4 {
            return Err(format!(
                "{}:{} requires 4 TSV columns",
                path.display(),
                line_no + 1
            ));
        }
        let index = cols[0]
            .parse::<usize>()
            .map_err(|e| format!("{}:{} invalid mesh index: {e}", path.display(), line_no + 1))?;
        out.insert(
            index,
            SourceMaterial {
                mesh_name: cols[1].to_owned(),
                source: cols[3].to_owned(),
            },
        );
    }
    Ok(out)
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let input = PathBuf::from(args.next().ok_or(
        "usage: rebind_level_ydd_materials INPUT.ydd LOGICAL_REF MATERIALS.tsv OUTPUT.ydd",
    )?);
    let logical = args.next().ok_or("missing LOGICAL_REF")?;
    let materials_path = PathBuf::from(args.next().ok_or("missing MATERIALS.tsv")?);
    let output = PathBuf::from(args.next().ok_or("missing OUTPUT.ydd")?);
    if args.next().is_some() {
        return Err("too many arguments".to_owned());
    }

    let mapping = read_materials(&materials_path)?;
    let bytes = fs::read(&input).map_err(|e| format!("read {}: {e}", input.display()))?;
    let decoded = decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD,
        &logical,
    )?;
    let mut document = decode_ydd_binary_body(&decoded.body)?;
    let mut assigned = BTreeMap::<String, usize>::new();
    let mut removed = 0usize;
    let mut total = 0usize;

    for entry in &mut document.entries {
        let before = entry.meshes.len();
        // northstar-ydd-packer currently preserves OBJ group order but may duplicate the first
        // group name across meshes. The source extractor emits OBJ groups in the exact TSV
        // mesh_index order, so ordinal identity is the authoritative reconstruction key here.
        let mut rebound = Vec::with_capacity(before);
        for (index, mut mesh) in std::mem::take(&mut entry.meshes).into_iter().enumerate() {
            total += 1;
            let Some(source) = mapping.get(&index) else {
                return Err(format!(
                    "no TSV material row for entry='{}' mesh_ordinal={} mesh='{}' mapping_rows={}",
                    entry.name,
                    index,
                    mesh.name,
                    mapping.len()
                ));
            };
            debug_assert!(!source.mesh_name.is_empty());
            // Restore stable source identity even when the generic OBJ importer duplicated a group name.
            mesh.name = format!(
                "m{index:04}_{}",
                source.mesh_name.to_ascii_lowercase().replace(
                    |c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '.',
                    "_"
                )
            );
            match material_family(&source.mesh_name, &source.source) {
                Some(family) => {
                    mesh.material_ref = Some(format!("materials/{family}.nemat@{family}"));
                    *assigned.entry(family.to_owned()).or_default() += 1;
                    rebound.push(mesh);
                }
                None => {
                    removed += 1;
                }
            }
        }
        entry.meshes = rebound;
        if before > 0 && entry.meshes.is_empty() {
            return Err(format!("all meshes removed from entry '{}'", entry.name));
        }
    }

    let body = encode_ydd_binary_body(&document)?;
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
    let verify = decode_list_file_envelope(
        &output_bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD,
        &logical,
    )?;
    let verify_doc = decode_ydd_binary_body(&verify.body)?;
    let output_meshes: usize = verify_doc
        .entries
        .iter()
        .map(|entry| entry.meshes.len())
        .sum();
    if output_meshes + removed != total {
        return Err("material rebind mesh accounting mismatch".to_owned());
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&output, output_bytes).map_err(|e| format!("write {}: {e}", output.display()))?;
    println!("LEVEL_YDD_MATERIAL_REBIND_OK input='{}' output='{}' meshes={} removed_shadow_proxies={} families={:?}", input.display(), output.display(), output_meshes, removed, assigned);
    Ok(())
}
