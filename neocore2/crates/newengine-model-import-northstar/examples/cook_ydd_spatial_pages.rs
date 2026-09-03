use flate2::{write::DeflateEncoder, Compression};
use newengine_asset_format_nef8::ydd_binary::{
    decode_ydd_binary_body, decode_ydd_binary_entries, encode_ydd_binary_body, YddBinaryDocument,
    YddBinaryEntry,
};
use newengine_assets_api::{decode_list_file_envelope, encode_list_file, ListFileEncodeRequest};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    thread,
};

const TARGET_PAGE_BYTES: usize = 4 * 1024 * 1024;
const HARD_MAX_PAGE_BYTES: usize = 6 * 1024 * 1024;
// Geometry-heavy YDD bodies normally compress substantially. This raw-body target is only the
// first deterministic cut; the hard physical limit below is enforced against the actual NEF8 file.
const RAW_TARGET_BYTES: usize = TARGET_PAGE_BYTES * 2;
const MAX_COOK_WORKERS: usize = 8;

#[derive(Clone, Debug)]
struct PlanRow {
    fingerprint: String,
    definition_ref: String,
    entry: String,
    source_ydd: String,
    page_ydd: String,
}

#[derive(Debug)]
struct ScheduledEntry {
    row: PlanRow,
    entry: YddBinaryEntry,
}

#[derive(Debug)]
struct PageJob {
    base_page_ydd: String,
    schema: u16,
    entries: Vec<ScheduledEntry>,
}

#[derive(Debug)]
struct PackedPage {
    page_ydd: String,
    base_page_ydd: String,
    rows: Vec<PlanRow>,
    encoded: Vec<u8>,
    uncompressed_bytes: usize,
}

fn normalize(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_owned()
}

fn read_plan(path: &Path) -> Result<Vec<PlanRow>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut lines = text.lines();
    let header = lines.next().ok_or("spatial page plan is empty")?;
    let columns: Vec<&str> = header.split('\t').collect();
    let index = |name: &str| -> Result<usize, String> {
        columns
            .iter()
            .position(|column| *column == name)
            .ok_or_else(|| format!("spatial page plan missing column '{name}'"))
    };
    let fp_i = index("fingerprint")?;
    let definition_i = index("definition_ref")?;
    let entry_i = index("entry")?;
    let source_i = index("source_ydd")?;
    let page_i = index("page_ydd")?;
    let required = [fp_i, definition_i, entry_i, source_i, page_i]
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
            return Err(format!(
                "spatial page plan row {} is truncated",
                line_index + 2
            ));
        }
        let row = PlanRow {
            fingerprint: fields[fp_i].trim().to_owned(),
            definition_ref: normalize(fields[definition_i]),
            entry: fields[entry_i].trim().to_owned(),
            source_ydd: normalize(fields[source_i]),
            page_ydd: normalize(fields[page_i]),
        };
        if row.definition_ref.is_empty()
            || row.entry.is_empty()
            || row.source_ydd.is_empty()
            || row.page_ydd.is_empty()
        {
            return Err(format!(
                "spatial page plan row {} has an empty required field",
                line_index + 2
            ));
        }
        rows.push(row);
    }
    if rows.is_empty() {
        return Err("spatial page plan contains no rows".to_owned());
    }
    Ok(rows)
}

fn load_ydd_entries(
    content_root: &Path,
    logical: &str,
    selectors: &[String],
) -> Result<(newengine_assets_api::ListFileHeader, YddBinaryDocument), String> {
    let path = content_root.join(logical.replace('/', std::path::MAIN_SEPARATOR_STR));
    let bytes = fs::read(&path).map_err(|e| format!("read source YDD {}: {e}", path.display()))?;
    let decoded = decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD,
        logical,
    )?;
    let document = decode_ydd_binary_entries(&decoded.body, selectors)
        .map_err(|e| format!("selective decode source YDD logical='{logical}': {e}"))?;
    Ok((decoded.header, document))
}

fn encode_page_document(
    logical: &str,
    document: &YddBinaryDocument,
    content_schema_version: u16,
) -> Result<(Vec<u8>, usize), String> {
    let body = encode_ydd_binary_body(document)?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&body).map_err(|e| e.to_string())?;
    let stored = encoder.finish().map_err(|e| e.to_string())?;
    let entry_count = document.entries.len();
    let encoded = encode_list_file(ListFileEncodeRequest {
        content_kind: newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD,
        content_schema_version,
        entry_count: u32::try_from(entry_count).map_err(|_| "page entry count exceeds u32")?,
        additional_flags: 0,
        min_size_class: 5,
        header_metadata: &[],
        body_stored: &stored,
        body_uncompressed_len: body.len() as u64,
        body_raw_hash: None,
        stable_file_id: None,
        import_settings_hash: None,
    })?;
    let verify = decode_list_file_envelope(
        &encoded,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD,
        logical,
    )?;
    let verify_doc = decode_ydd_binary_body(&verify.body)?;
    if verify_doc.entries.len() != entry_count {
        return Err(format!(
            "page verify entry count mismatch logical='{logical}' expected={entry_count} actual={}",
            verify_doc.entries.len()
        ));
    }
    let actual: BTreeSet<String> = verify_doc
        .entries
        .iter()
        .map(|entry| entry.name.to_ascii_lowercase())
        .collect();
    let expected: BTreeSet<String> = document
        .entries
        .iter()
        .map(|entry| entry.name.to_ascii_lowercase())
        .collect();
    if actual != expected {
        return Err(format!(
            "page verify entry identity mismatch logical='{logical}'"
        ));
    }
    Ok((encoded, body.len()))
}

fn estimated_raw_bytes(entry: &YddBinaryEntry) -> usize {
    let mut bytes = 192usize
        .saturating_add(entry.name.len())
        .saturating_add(entry.source_path.len())
        .saturating_add(entry.properties_ref.as_deref().map_or(0, str::len));
    for mesh in &entry.meshes {
        bytes = bytes
            .saturating_add(128)
            .saturating_add(mesh.name.len())
            .saturating_add(mesh.material_ref.as_deref().map_or(0, str::len))
            .saturating_add(mesh.vertices.len().saturating_mul(32))
            .saturating_add(mesh.indices.len().saturating_mul(4));
        if let Some(skin) = &mesh.skin {
            bytes = bytes.saturating_add(skin.len().saturating_mul(48));
        }
    }
    bytes.max(1)
}

fn initial_buckets(mut entries: Vec<ScheduledEntry>) -> Vec<Vec<ScheduledEntry>> {
    entries.sort_by(|a, b| {
        a.row
            .entry
            .to_ascii_lowercase()
            .cmp(&b.row.entry.to_ascii_lowercase())
    });
    let mut buckets = Vec::new();
    let mut current = Vec::new();
    let mut current_bytes = 0usize;
    for scheduled in entries {
        let weight = estimated_raw_bytes(&scheduled.entry);
        if !current.is_empty() && current_bytes.saturating_add(weight) > RAW_TARGET_BYTES {
            buckets.push(std::mem::take(&mut current));
            current_bytes = 0;
        }
        current_bytes = current_bytes.saturating_add(weight);
        current.push(scheduled);
    }
    if !current.is_empty() {
        buckets.push(current);
    }
    buckets
}

fn split_by_weight(entries: Vec<ScheduledEntry>) -> (Vec<ScheduledEntry>, Vec<ScheduledEntry>) {
    debug_assert!(entries.len() > 1);
    let total: usize = entries.iter().map(|e| estimated_raw_bytes(&e.entry)).sum();
    let target = total / 2;
    let mut accumulated = 0usize;
    let mut split = 1usize;
    for (index, entry) in entries.iter().enumerate().take(entries.len() - 1) {
        accumulated = accumulated.saturating_add(estimated_raw_bytes(&entry.entry));
        split = index + 1;
        if accumulated >= target {
            break;
        }
    }
    let mut left = entries;
    let right = left.split_off(split.clamp(1, left.len() - 1));
    (left, right)
}

fn pack_candidate(
    base_page_ydd: &str,
    schema: u16,
    entries: Vec<ScheduledEntry>,
    accepted: &mut Vec<(Vec<PlanRow>, Vec<u8>, usize)>,
) -> Result<(), String> {
    let rows: Vec<PlanRow> = entries.iter().map(|entry| entry.row.clone()).collect();
    let document = YddBinaryDocument {
        entries: entries.iter().map(|entry| entry.entry.clone()).collect(),
    };
    let (encoded, uncompressed_bytes) = encode_page_document(base_page_ydd, &document, schema)?;
    if entries.len() == 1 {
        if encoded.len() > HARD_MAX_PAGE_BYTES {
            return Err(format!(
                "single definition exceeds hard spatial page cap base='{}' entry='{}' bytes={} hard_max={}",
                base_page_ydd,
                entries[0].row.entry,
                encoded.len(),
                HARD_MAX_PAGE_BYTES
            ));
        }
        accepted.push((rows, encoded, uncompressed_bytes));
        return Ok(());
    }
    if encoded.len() <= TARGET_PAGE_BYTES {
        accepted.push((rows, encoded, uncompressed_bytes));
        return Ok(());
    }
    let (left, right) = split_by_weight(entries);
    pack_candidate(base_page_ydd, schema, left, accepted)?;
    pack_candidate(base_page_ydd, schema, right, accepted)?;
    Ok(())
}

fn subpage_name(base_page_ydd: &str, index: usize) -> String {
    let base = base_page_ydd.strip_suffix(".ydd").unwrap_or(base_page_ydd);
    format!("{base}_{index:02}.ydd")
}

fn pack_job(job: PageJob) -> Result<Vec<PackedPage>, String> {
    let mut accepted = Vec::new();
    for bucket in initial_buckets(job.entries) {
        pack_candidate(&job.base_page_ydd, job.schema, bucket, &mut accepted)?;
    }
    let mut out = Vec::with_capacity(accepted.len());
    for (index, (rows, encoded, uncompressed_bytes)) in accepted.into_iter().enumerate() {
        out.push(PackedPage {
            page_ydd: subpage_name(&job.base_page_ydd, index),
            base_page_ydd: job.base_page_ydd.clone(),
            rows,
            encoded,
            uncompressed_bytes,
        });
    }
    Ok(out)
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let content_root = PathBuf::from(args.next().ok_or(
        "usage: cook_ydd_spatial_pages <Content-root> <spatial_page_plan.tsv> <output-root> <manifest.tsv>",
    )?);
    let plan_path = PathBuf::from(args.next().ok_or("missing spatial_page_plan.tsv")?);
    let output_root = PathBuf::from(args.next().ok_or("missing output-root")?);
    let manifest_path = PathBuf::from(args.next().ok_or("missing manifest.tsv")?);
    if args.next().is_some() {
        return Err("too many arguments".to_owned());
    }

    let rows = read_plan(&plan_path)?;
    let planned_rows = rows.len();
    let mut rows_by_source = BTreeMap::<String, Vec<PlanRow>>::new();
    for row in rows {
        rows_by_source
            .entry(row.source_ydd.clone())
            .or_default()
            .push(row);
    }

    let mut page_entries = BTreeMap::<String, Vec<ScheduledEntry>>::new();
    let mut page_entry_names = BTreeMap::<String, BTreeSet<String>>::new();
    let mut page_schema = BTreeMap::<String, u16>::new();
    let mut resolved = 0usize;

    for (source_ydd, source_rows) in rows_by_source {
        let selectors: Vec<String> = source_rows.iter().map(|row| row.entry.clone()).collect();
        let (header, document) = load_ydd_entries(&content_root, &source_ydd, &selectors)?;
        let mut by_name: BTreeMap<String, YddBinaryEntry> = document
            .entries
            .into_iter()
            .map(|entry| (entry.name.to_ascii_lowercase(), entry))
            .collect();
        for row in source_rows {
            let selector = row.entry.to_ascii_lowercase();
            let entry = by_name.remove(&selector).ok_or_else(|| {
                format!(
                    "authoritative page-plan selector missing source='{}' entry='{}' fingerprint='{}'",
                    source_ydd, row.entry, row.fingerprint
                )
            })?;
            let names = page_entry_names.entry(row.page_ydd.clone()).or_default();
            if !names.insert(selector) {
                return Err(format!(
                    "duplicate entry scheduled into base page page='{}' entry='{}'",
                    row.page_ydd, row.entry
                ));
            }
            page_schema
                .entry(row.page_ydd.clone())
                .and_modify(|schema| {
                    if *schema != header.content_schema_version {
                        *schema = u16::MAX;
                    }
                })
                .or_insert(header.content_schema_version);
            page_entries
                .entry(row.page_ydd.clone())
                .or_default()
                .push(ScheduledEntry { row, entry });
            resolved += 1;
        }
    }

    if resolved != planned_rows {
        return Err(format!(
            "spatial page source resolution mismatch planned={} resolved={}",
            planned_rows, resolved
        ));
    }

    let mut jobs = Vec::with_capacity(page_entries.len());
    for (base_page_ydd, entries) in page_entries {
        let schema = page_schema.get(&base_page_ydd).copied().unwrap_or(1);
        if schema == u16::MAX {
            return Err(format!(
                "base page mixes source YDD content schema versions page='{base_page_ydd}'"
            ));
        }
        jobs.push(PageJob {
            base_page_ydd,
            schema,
            entries,
        });
    }

    // Large groups first gives substantially better load balancing while output ordering remains
    // deterministic because packed pages are sorted before writing.
    jobs.sort_by(|a, b| {
        let a_weight: usize = a
            .entries
            .iter()
            .map(|e| estimated_raw_bytes(&e.entry))
            .sum();
        let b_weight: usize = b
            .entries
            .iter()
            .map(|e| estimated_raw_bytes(&e.entry))
            .sum();
        b_weight
            .cmp(&a_weight)
            .then_with(|| a.base_page_ydd.cmp(&b.base_page_ydd))
    });
    let worker_count = thread::available_parallelism()
        .map_or(1, usize::from)
        .min(MAX_COOK_WORKERS)
        .min(jobs.len().max(1));
    let mut worker_jobs: Vec<Vec<PageJob>> = (0..worker_count).map(|_| Vec::new()).collect();
    for (index, job) in jobs.into_iter().enumerate() {
        worker_jobs[index % worker_count].push(job);
    }

    let mut packed_pages = Vec::new();
    thread::scope(|scope| -> Result<(), String> {
        let mut handles = Vec::new();
        for bucket in worker_jobs {
            handles.push(scope.spawn(move || -> Result<Vec<PackedPage>, String> {
                let mut pages = Vec::new();
                for job in bucket {
                    pages.extend(pack_job(job)?);
                }
                Ok(pages)
            }));
        }
        for handle in handles {
            packed_pages.extend(
                handle
                    .join()
                    .map_err(|_| "spatial page worker panicked".to_owned())??,
            );
        }
        Ok(())
    })?;

    packed_pages.sort_by(|a, b| a.page_ydd.cmp(&b.page_ydd));
    let mapped_entries: usize = packed_pages.iter().map(|page| page.rows.len()).sum();
    if mapped_entries != resolved {
        return Err(format!(
            "packed page mapping mismatch resolved={} mapped={}",
            resolved, mapped_entries
        ));
    }
    if let Some(page) = packed_pages
        .iter()
        .find(|page| page.encoded.len() > HARD_MAX_PAGE_BYTES)
    {
        return Err(format!(
            "hard spatial page cap violated page='{}' bytes={} hard_max={}",
            page.page_ydd,
            page.encoded.len(),
            HARD_MAX_PAGE_BYTES
        ));
    }

    if output_root.exists() {
        fs::remove_dir_all(&output_root)
            .map_err(|e| format!("clean shadow output {}: {e}", output_root.display()))?;
    }
    fs::create_dir_all(&output_root).map_err(|e| e.to_string())?;
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let entry_map_path = manifest_path.with_file_name("cooked_entry_map.tsv");
    let mut manifest = String::from(
        "page_ydd\tbase_page_ydd\tentries\tbytes\tuncompressed_bytes\ttarget_bytes\thard_max_bytes\n",
    );
    let mut entry_map =
        String::from("definition_ref\tfingerprint\tentry\tsource_ydd\tbase_page_ydd\tpage_ydd\n");
    let mut total_bytes = 0u64;
    let mut max_page_bytes = 0usize;
    let mut pages_over_target = 0usize;
    for page in packed_pages {
        let output = output_root.join(page.page_ydd.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&output, &page.encoded)
            .map_err(|e| format!("write {}: {e}", output.display()))?;
        manifest.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            page.page_ydd,
            page.base_page_ydd,
            page.rows.len(),
            page.encoded.len(),
            page.uncompressed_bytes,
            TARGET_PAGE_BYTES,
            HARD_MAX_PAGE_BYTES,
        ));
        for row in &page.rows {
            entry_map.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                row.definition_ref,
                row.fingerprint,
                row.entry,
                row.source_ydd,
                page.base_page_ydd,
                page.page_ydd,
            ));
        }
        total_bytes += page.encoded.len() as u64;
        max_page_bytes = max_page_bytes.max(page.encoded.len());
        if page.encoded.len() > TARGET_PAGE_BYTES {
            pages_over_target += 1;
        }
    }
    fs::write(&manifest_path, manifest)
        .map_err(|e| format!("write {}: {e}", manifest_path.display()))?;
    fs::write(&entry_map_path, entry_map)
        .map_err(|e| format!("write {}: {e}", entry_map_path.display()))?;
    let total_pages = fs::read_to_string(&manifest_path)
        .map_err(|e| e.to_string())?
        .lines()
        .skip(1)
        .count();
    println!(
        "spatial YDD bounded shadow cook PASS resolved_entries={} pages={} bytes={} mb={:.2} max_page_bytes={} pages_over_target={} target={} hard_max={} workers={} entry_map='{}'",
        resolved,
        total_pages,
        total_bytes,
        total_bytes as f64 / 1024.0 / 1024.0,
        max_page_bytes,
        pages_over_target,
        TARGET_PAGE_BYTES,
        HARD_MAX_PAGE_BYTES,
        worker_count,
        entry_map_path.display(),
    );
    Ok(())
}
