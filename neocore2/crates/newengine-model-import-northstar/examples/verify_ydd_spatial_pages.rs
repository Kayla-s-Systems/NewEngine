use newengine_asset_format_nef8::ydd_binary::{decode_ydd_binary_entries, YddBinaryEntry};
use newengine_assets_api::decode_list_file_envelope;
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
struct MappingRow {
    definition_ref: String,
    fingerprint: String,
    entry: String,
    source_ydd: String,
    page_ydd: String,
}

fn normalize(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_owned()
}

fn read_map(path: &Path) -> Result<Vec<MappingRow>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut lines = text.lines();
    let header = lines.next().ok_or("entry map is empty")?;
    let columns: Vec<&str> = header.split('\t').collect();
    let index = |name: &str| -> Result<usize, String> {
        columns
            .iter()
            .position(|column| *column == name)
            .ok_or_else(|| format!("entry map missing column '{name}'"))
    };
    let definition_i = index("definition_ref")?;
    let fingerprint_i = index("fingerprint")?;
    let entry_i = index("entry")?;
    let source_i = index("source_ydd")?;
    let page_i = index("page_ydd")?;
    let required = [definition_i, fingerprint_i, entry_i, source_i, page_i]
        .into_iter()
        .max()
        .unwrap_or(0);
    let mut rows = Vec::new();
    for (line_index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() <= required {
            return Err(format!("entry map row {} is truncated", line_index + 2));
        }
        rows.push(MappingRow {
            definition_ref: normalize(fields[definition_i]),
            fingerprint: fields[fingerprint_i].trim().to_owned(),
            entry: fields[entry_i].trim().to_owned(),
            source_ydd: normalize(fields[source_i]),
            page_ydd: normalize(fields[page_i]),
        });
    }
    Ok(rows)
}

fn load_entries(
    root: &Path,
    logical: &str,
    selectors: &[String],
) -> Result<Vec<YddBinaryEntry>, String> {
    let path = root.join(logical.replace('/', std::path::MAIN_SEPARATOR_STR));
    let bytes = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let decoded = decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD,
        logical,
    )?;
    Ok(decode_ydd_binary_entries(&decoded.body, selectors)
        .map_err(|e| format!("selective decode logical='{logical}': {e}"))?
        .entries)
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let content_root = PathBuf::from(args.next().ok_or(
        "usage: verify_ydd_spatial_pages <Content-root> <shadow-root> <cooked_entry_map.tsv>",
    )?);
    let shadow_root = PathBuf::from(args.next().ok_or("missing shadow-root")?);
    let map_path = PathBuf::from(args.next().ok_or("missing cooked_entry_map.tsv")?);
    if args.next().is_some() {
        return Err("too many arguments".to_owned());
    }

    let rows = read_map(&map_path)?;
    let expected_count = rows.len();
    let mut rows_by_source = BTreeMap::<String, Vec<MappingRow>>::new();
    let mut rows_by_page = BTreeMap::<String, Vec<MappingRow>>::new();
    for row in rows {
        rows_by_source
            .entry(row.source_ydd.clone())
            .or_default()
            .push(row.clone());
        rows_by_page
            .entry(row.page_ydd.clone())
            .or_default()
            .push(row);
    }

    let mut source_entries = BTreeMap::<String, YddBinaryEntry>::new();
    for (source_ydd, source_rows) in rows_by_source {
        let selectors: Vec<String> = source_rows.iter().map(|row| row.entry.clone()).collect();
        let decoded = load_entries(&content_root, &source_ydd, &selectors)?;
        let by_name: BTreeMap<String, YddBinaryEntry> = decoded
            .into_iter()
            .map(|entry| (entry.name.to_ascii_lowercase(), entry))
            .collect();
        for row in source_rows {
            let entry = by_name
                .get(&row.entry.to_ascii_lowercase())
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "source parity selector missing definition='{}' source='{}' entry='{}'",
                        row.definition_ref, source_ydd, row.entry
                    )
                })?;
            if source_entries
                .insert(row.definition_ref.clone(), entry)
                .is_some()
            {
                return Err(format!(
                    "duplicate source parity definition_ref='{}'",
                    row.definition_ref
                ));
            }
        }
    }

    let mut compared = 0usize;
    let mut meshes = 0usize;
    let mut triangles = 0u64;
    for (page_ydd, page_rows) in rows_by_page {
        let selectors: Vec<String> = page_rows.iter().map(|row| row.entry.clone()).collect();
        let decoded = load_entries(&shadow_root, &page_ydd, &selectors)?;
        let by_name: BTreeMap<String, YddBinaryEntry> = decoded
            .into_iter()
            .map(|entry| (entry.name.to_ascii_lowercase(), entry))
            .collect();
        for row in page_rows {
            let actual = by_name
                .get(&row.entry.to_ascii_lowercase())
                .ok_or_else(|| {
                    format!(
                        "page parity selector missing definition='{}' page='{}' entry='{}'",
                        row.definition_ref, page_ydd, row.entry
                    )
                })?;
            let expected = source_entries.remove(&row.definition_ref).ok_or_else(|| {
                format!(
                    "source parity entry missing definition='{}' fingerprint='{}'",
                    row.definition_ref, row.fingerprint
                )
            })?;
            if expected != *actual {
                return Err(format!(
                    "spatial page decoded entry parity mismatch definition='{}' fingerprint='{}' source='{}' page='{}' entry='{}' source_meshes={} page_meshes={}",
                    row.definition_ref,
                    row.fingerprint,
                    row.source_ydd,
                    page_ydd,
                    row.entry,
                    expected.meshes.len(),
                    actual.meshes.len(),
                ));
            }
            meshes += actual.meshes.len();
            triangles += actual
                .meshes
                .iter()
                .map(|mesh| (mesh.indices.len() / 3) as u64)
                .sum::<u64>();
            compared += 1;
        }
    }
    if !source_entries.is_empty() {
        return Err(format!(
            "spatial page parity left unmatched source entries count={}",
            source_entries.len()
        ));
    }
    if compared != expected_count {
        return Err(format!(
            "spatial page parity count mismatch expected={} compared={}",
            expected_count, compared
        ));
    }
    println!(
        "spatial YDD decoded-entry parity PASS entries={} meshes={} triangles={} source_mode='authoritative SceneV1 YDD' page_mode='bounded spatial pages'",
        compared, meshes, triangles
    );
    Ok(())
}
