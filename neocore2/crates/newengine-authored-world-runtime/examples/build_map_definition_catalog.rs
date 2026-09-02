use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
};

use newengine_authored_world_runtime::{
    decode_map_definition_catalog, encode_map_definition_catalog,
    set_map_definition_physical_drawable_ref,
};
use newengine_definitions_runtime::{decode_ytyp_definition_entries_from_body, DefinitionEntryV1};

#[derive(Clone, Debug)]
struct PhysicalDrawableMapping {
    entry: String,
    physical_ref: String,
}

fn normalize(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

fn collect_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                let lower = name.to_ascii_lowercase();
                lower.ends_with(".ytyp") || lower.ends_with(".ytyp.xml")
            })
        {
            out.push(path);
        }
    }
    Ok(())
}

fn logical_source(root: &Path, path: &Path, logical_prefix: &str) -> Result<String, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    let relative = relative.strip_suffix(".xml").unwrap_or(&relative);
    let prefix = logical_prefix.trim().trim_matches('/').replace('\\', "/");
    Ok(if prefix.is_empty() {
        relative.to_owned()
    } else {
        format!("{prefix}/{relative}")
    })
}

fn load_physical_drawable_map(
    path: Option<&Path>,
) -> Result<BTreeMap<String, PhysicalDrawableMapping>, String> {
    let Some(path) = path else {
        return Ok(BTreeMap::new());
    };
    let text = fs::read_to_string(path)
        .map_err(|error| format!("read physical drawable map '{}': {error}", path.display()))?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| format!("physical drawable map '{}' is empty", path.display()))?;
    let columns: Vec<&str> = header.split('\t').collect();
    let index = |name: &str| -> Result<usize, String> {
        columns
            .iter()
            .position(|column| *column == name)
            .ok_or_else(|| {
                format!(
                    "physical drawable map '{}' missing column '{name}'",
                    path.display()
                )
            })
    };
    let definition_i = index("definition_ref")?;
    let entry_i = index("entry")?;
    let page_i = index("page_ydd")?;
    let required = [definition_i, entry_i, page_i]
        .into_iter()
        .max()
        .unwrap_or(0);
    let mut out = BTreeMap::new();
    for (line_index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() <= required {
            return Err(format!(
                "physical drawable map '{}' row {} is truncated",
                path.display(),
                line_index + 2
            ));
        }
        let definition = normalize(fields[definition_i]);
        let entry = fields[entry_i].trim().to_owned();
        let page = normalize(fields[page_i]);
        if definition.is_empty() || entry.is_empty() || !page.to_ascii_lowercase().ends_with(".ydd")
        {
            return Err(format!(
                "physical drawable map '{}' row {} is invalid definition='{}' entry='{}' page='{}'",
                path.display(),
                line_index + 2,
                definition,
                entry,
                page
            ));
        }
        let definition_ref = if definition.contains('@') {
            definition
        } else {
            format!("{definition}@{entry}")
        };
        let physical_ref = format!("{page}@{entry}");
        if out
            .insert(
                definition_ref.clone(),
                PhysicalDrawableMapping {
                    entry,
                    physical_ref,
                },
            )
            .is_some()
        {
            return Err(format!(
                "physical drawable map '{}' contains duplicate definition_ref='{definition_ref}'",
                path.display()
            ));
        }
    }
    Ok(out)
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let source_root = PathBuf::from(args.next().ok_or(
        "usage: build_map_definition_catalog <source-dir> <logical-prefix> <map-ref> <output.definition_catalog> [physical-entry-map.tsv]",
    )?);
    let logical_prefix = args.next().ok_or("missing logical-prefix")?;
    let map_ref = args.next().ok_or("missing map-ref")?;
    let output = PathBuf::from(args.next().ok_or("missing output path")?);
    let physical_map_path = args.next().map(PathBuf::from);
    if args.next().is_some() {
        return Err("too many arguments".to_owned());
    }

    let physical_map = load_physical_drawable_map(physical_map_path.as_deref())?;
    let mut files = Vec::new();
    collect_files(&source_root, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(format!(
            "no .ytyp/.ytyp.xml files found under '{}'",
            source_root.display()
        ));
    }

    let mut entries = BTreeMap::<String, DefinitionEntryV1>::new();
    let mut physical_applied = BTreeSet::new();
    for path in files {
        let source = logical_source(&source_root, &path, &logical_prefix)?;
        let body = fs::read(&path)
            .map_err(|error| format!("read failed path='{}' err='{error}'", path.display()))?;
        for mut entry in
            decode_ytyp_definition_entries_from_body(&source, &body).map_err(|error| {
                format!(
                    "YTYP semantic decode failed path='{}' source='{}' err='{error}'",
                    path.display(),
                    source
                )
            })?
        {
            let definition_ref = normalize(&entry.identity.definition_ref);
            if let Some(mapping) = physical_map.get(&definition_ref) {
                if !entry.identity.name.eq_ignore_ascii_case(&mapping.entry) {
                    return Err(format!(
                        "physical drawable selector mismatch definition='{}' identity_name='{}' mapped_entry='{}'",
                        definition_ref, entry.identity.name, mapping.entry
                    ));
                }
                let authored_selector = entry
                    .refs
                    .drawable_refs
                    .first()
                    .and_then(|reference| {
                        reference
                            .rsplit_once('@')
                            .map(|(_, selector)| selector.trim())
                    })
                    .unwrap_or_default();
                if !authored_selector.eq_ignore_ascii_case(&mapping.entry) {
                    return Err(format!(
                        "physical drawable selector does not preserve authored d_* identity definition='{}' authored='{}' mapped='{}'",
                        definition_ref, authored_selector, mapping.entry
                    ));
                }
                set_map_definition_physical_drawable_ref(&mut entry, &mapping.physical_ref)?;
                physical_applied.insert(definition_ref.clone());
            }
            if entries.insert(definition_ref.clone(), entry).is_some() {
                return Err(format!(
                    "duplicate definition_ref while building catalog ref='{definition_ref}'"
                ));
            }
        }
    }

    let missing_physical: Vec<&String> = physical_map
        .keys()
        .filter(|definition_ref| !physical_applied.contains(*definition_ref))
        .collect();
    if !missing_physical.is_empty() {
        return Err(format!(
            "physical drawable map contains definitions absent from catalog count={} first={:?}",
            missing_physical.len(),
            &missing_physical[..missing_physical.len().min(8)]
        ));
    }

    let entry_count = entries.len();
    let encoded = encode_map_definition_catalog(&map_ref, entries)?;
    if !physical_map.is_empty() {
        let decoded = decode_map_definition_catalog(&map_ref, &encoded)?;
        for (definition_ref, mapping) in &physical_map {
            let resolved = decoded.decode_entry(definition_ref)?.ok_or_else(|| {
                format!("shadow catalog lost mapped definition_ref='{definition_ref}'")
            })?;
            let actual = resolved
                .refs
                .drawable_refs
                .first()
                .map(String::as_str)
                .unwrap_or_default();
            if actual != mapping.physical_ref {
                return Err(format!(
                    "shadow catalog physical resolution mismatch definition='{}' expected='{}' actual='{}'",
                    definition_ref, mapping.physical_ref, actual
                ));
            }
        }
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&output, &encoded).map_err(|error| error.to_string())?;
    println!(
        "indexed map definition catalog built output='{}' entries={} bytes={} map='{}' physical_mapped={} physical_mode='{}'",
        output.display(),
        entry_count,
        encoded.len(),
        map_ref.trim(),
        physical_applied.len(),
        if physical_map.is_empty() { "authored" } else { "spatial_override" },
    );
    Ok(())
}
