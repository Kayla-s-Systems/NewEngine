#![forbid(unsafe_op_in_unsafe_fn)]

use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Path, PathBuf},
};

use flate2::{write::DeflateEncoder, Compression};
use newengine_contract_api::{ContractKind, ContractVersion};

pub const MIGRATION_REGISTRY_SCHEMA: &str = "northstar.migration_registry.v1";
pub const MIGRATION_REGISTRY_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStrategy {
    EnvelopeSchemaRewrite,
    SemanticReencode,
    AuthoredSchemaRewrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationReversibility {
    ExactPayloadPreserving,
    SemanticOnly,
    ExactTextExceptSchema,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationBackupPolicy {
    RequiredFullCopyWithSha256Manifest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MigrationContractRef {
    pub contract_key: &'static str,
    pub version: ContractVersion,
    pub representation_id: Option<&'static str>,
}

impl MigrationContractRef {
    pub const fn major(contract_key: &'static str, major: u16) -> Self {
        Self {
            contract_key,
            version: ContractVersion::major(major),
            representation_id: None,
        }
    }

    pub const fn represented(
        contract_key: &'static str,
        major: u16,
        representation_id: &'static str,
    ) -> Self {
        Self {
            contract_key,
            version: ContractVersion::major(major),
            representation_id: Some(representation_id),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MigrationToolSpec {
    pub workspace: &'static str,
    pub package: &'static str,
    pub example: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MigrationCorpusGateSpec {
    pub file_suffix: &'static str,
    pub content_kind: Option<u32>,
    pub source_versions: &'static [u16],
    pub target_version: u16,
    pub roots: &'static [&'static str],
    pub require_zero_source_after_migration: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MigrationSpec {
    pub migration_version: u16,
    pub id: &'static str,
    pub source: MigrationContractRef,
    pub target: MigrationContractRef,
    pub strategy: MigrationStrategy,
    pub tool: MigrationToolSpec,
    pub reversibility: MigrationReversibility,
    pub backup_policy: MigrationBackupPolicy,
    pub corpus_gate: MigrationCorpusGateSpec,
}

const FIRST_PARTY_CORPUS_ROOTS: &[&str] = &["Projects", "Shared"];
const MIGRATION_TOOL: MigrationToolSpec = MigrationToolSpec {
    workspace: "neocore",
    package: "newengine-migration-registry",
    example: "migrate_asset",
};

pub const MIGRATIONS: &[MigrationSpec] = &[
    MigrationSpec {
        migration_version: 1,
        id: "asset.ytd.schema.v2_to_v1",
        source: MigrationContractRef::major("asset.ytd.schema", 2),
        target: MigrationContractRef::major(
            "asset.ytd.schema",
            1,
        ),
        strategy: MigrationStrategy::EnvelopeSchemaRewrite,
        tool: MIGRATION_TOOL,
        reversibility: MigrationReversibility::ExactPayloadPreserving,
        backup_policy: MigrationBackupPolicy::RequiredFullCopyWithSha256Manifest,
        corpus_gate: MigrationCorpusGateSpec {
            file_suffix: ".ytd",
            content_kind: Some(newengine_assets_api::LIST_FILE_CONTENT_KIND_YTD),
            source_versions: &[2],
            target_version: 1,
            roots: FIRST_PARTY_CORPUS_ROOTS,
            require_zero_source_after_migration: true,
        },
    },
    MigrationSpec {
        migration_version: 1,
        id: "asset.ydd.body.v2_to_v4",
        source: MigrationContractRef::major(
            "asset.ydd.body",
            newengine_asset_format_nef8::ydd_binary::YDD_BINARY_SCHEMA_VERSION_V2 as u16,
        ),
        target: MigrationContractRef::major(
            "asset.ydd.body",
            newengine_asset_format_nef8::YDD_BINARY_SCHEMA_VERSION as u16,
        ),
        strategy: MigrationStrategy::SemanticReencode,
        tool: MIGRATION_TOOL,
        reversibility: MigrationReversibility::SemanticOnly,
        backup_policy: MigrationBackupPolicy::RequiredFullCopyWithSha256Manifest,
        corpus_gate: MigrationCorpusGateSpec {
            file_suffix: ".ydd",
            content_kind: Some(newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD),
            source_versions: &[
                newengine_asset_format_nef8::ydd_binary::YDD_BINARY_SCHEMA_VERSION_V2 as u16,
            ],
            target_version: newengine_asset_format_nef8::YDD_BINARY_SCHEMA_VERSION as u16,
            roots: FIRST_PARTY_CORPUS_ROOTS,
            require_zero_source_after_migration: true,
        },
    },
    MigrationSpec {
        migration_version: 1,
        id: "asset.ydd.body.v3_to_v4",
        source: MigrationContractRef::major(
            "asset.ydd.body",
            newengine_asset_format_nef8::ydd_binary::YDD_BINARY_SCHEMA_VERSION_V3 as u16,
        ),
        target: MigrationContractRef::major(
            "asset.ydd.body",
            newengine_asset_format_nef8::YDD_BINARY_SCHEMA_VERSION as u16,
        ),
        strategy: MigrationStrategy::SemanticReencode,
        tool: MIGRATION_TOOL,
        reversibility: MigrationReversibility::SemanticOnly,
        backup_policy: MigrationBackupPolicy::RequiredFullCopyWithSha256Manifest,
        corpus_gate: MigrationCorpusGateSpec {
            file_suffix: ".ydd",
            content_kind: Some(newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD),
            source_versions: &[
                newengine_asset_format_nef8::ydd_binary::YDD_BINARY_SCHEMA_VERSION_V3 as u16,
            ],
            target_version: newengine_asset_format_nef8::YDD_BINARY_SCHEMA_VERSION as u16,
            roots: FIRST_PARTY_CORPUS_ROOTS,
            require_zero_source_after_migration: true,
        },
    },
    MigrationSpec {
        migration_version: 1,
        id: "asset.nemat.authored_xml.legacy_to_xmltype_v1",
        source: MigrationContractRef::represented(
            "asset.nemat.authored_xml",
            1,
            "newengine.nemat.material_library.v1",
        ),
        target: MigrationContractRef::represented(
            "asset.nemat.authored_xml",
            1,
            "newengine.nemat.xmltype.v1",
        ),
        strategy: MigrationStrategy::AuthoredSchemaRewrite,
        tool: MIGRATION_TOOL,
        reversibility: MigrationReversibility::ExactTextExceptSchema,
        backup_policy: MigrationBackupPolicy::RequiredFullCopyWithSha256Manifest,
        corpus_gate: MigrationCorpusGateSpec {
            file_suffix: ".nemat.xml",
            content_kind: None,
            source_versions: &[1],
            target_version: 1,
            roots: FIRST_PARTY_CORPUS_ROOTS,
            require_zero_source_after_migration: true,
        },
    },
];

#[inline]
pub const fn migrations() -> &'static [MigrationSpec] {
    MIGRATIONS
}

pub fn migration(id: &str) -> Option<&'static MigrationSpec> {
    MIGRATIONS.iter().find(|spec| spec.id == id)
}

pub fn validate_registry() -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let mut ids = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for spec in MIGRATIONS {
        if spec.migration_version == 0 {
            errors.push(format!(
                "migration '{}' has zero migration_version",
                spec.id
            ));
        }
        if !ids.insert(spec.id) {
            errors.push(format!("duplicate migration id '{}'", spec.id));
        }
        if spec.source.version == spec.target.version
            && spec.source.contract_key == spec.target.contract_key
            && spec.source.representation_id == spec.target.representation_id
        {
            errors.push(format!(
                "migration '{}' source and target are identical",
                spec.id
            ));
        }
        if let Some(target) = newengine_contract_registry::contract(spec.target.contract_key) {
            if target.kind != ContractKind::Schema {
                errors.push(format!(
                    "migration '{}' target '{}' is not a schema contract",
                    spec.id, target.key
                ));
            }
            if target.version != spec.target.version {
                errors.push(format!(
                    "migration '{}' target version {} does not match registered core contract {}",
                    spec.id, spec.target.version, target.version
                ));
            }
            if let Some(representation_id) = spec.target.representation_id {
                if target.advertised_id != Some(representation_id) {
                    errors.push(format!(
                        "migration '{}' target representation '{}' does not match registered advertised id {:?}",
                        spec.id, representation_id, target.advertised_id
                    ));
                }
            }
        } else if !spec.target.contract_key.starts_with("asset.") {
            errors.push(format!(
                "migration '{}' target contract '{}' is neither a core contract nor a descriptor-owned asset schema",
                spec.id, spec.target.contract_key
            ));
        }
        if !sources.insert((
            spec.source.contract_key,
            spec.source.version.major,
            spec.source.representation_id,
        )) {
            errors.push(format!(
                "duplicate migration source '{}@{}'",
                spec.source.contract_key, spec.source.version
            ));
        }
        if spec.corpus_gate.target_version != spec.target.version.major {
            errors.push(format!(
                "migration '{}' corpus target={} differs from target contract={}",
                spec.id, spec.corpus_gate.target_version, spec.target.version.major
            ));
        }
        if !spec
            .corpus_gate
            .source_versions
            .contains(&spec.source.version.major)
        {
            errors.push(format!(
                "migration '{}' corpus source versions {:?} omit source {}",
                spec.id, spec.corpus_gate.source_versions, spec.source.version.major
            ));
        }
        if spec.tool.package.trim().is_empty() || spec.tool.example.trim().is_empty() {
            errors.push(format!("migration '{}' has incomplete tool spec", spec.id));
        }
    }
    // Format current/readable schema policy is validated against StarVault descriptors
    // by descriptor-driven conformance. This registry owns only explicit migration edges.
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn range<'a>(source: &'a [u8], offset: u64, len: u64, label: &str) -> Result<&'a [u8], String> {
    let start = usize::try_from(offset).map_err(|_| format!("{label} offset overflow"))?;
    let len = usize::try_from(len).map_err(|_| format!("{label} len overflow"))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| format!("{label} range overflow"))?;
    source
        .get(start..end)
        .ok_or_else(|| format!("{label} range truncated"))
}

fn verify_source(
    spec: &MigrationSpec,
    source: &[u8],
) -> Result<newengine_assets_api::ListFileHeader, String> {
    let header = newengine_assets_api::parse_list_file_header(source)?;
    let expected_kind = spec
        .corpus_gate
        .content_kind
        .ok_or_else(|| format!("migration '{}' has no ListFile content kind", spec.id))?;
    if header.content_kind != expected_kind {
        return Err(format!(
            "migration '{}' content_kind={} expected={}",
            spec.id, header.content_kind, expected_kind
        ));
    }
    if header.content_schema_version != spec.source.version.major {
        return Err(format!(
            "migration '{}' source schema={} expected={}",
            spec.id, header.content_schema_version, spec.source.version.major
        ));
    }
    Ok(header)
}

fn rewrite_envelope_schema(spec: &MigrationSpec, source: &[u8]) -> Result<Vec<u8>, String> {
    let header = verify_source(spec, source)?;
    let metadata = range(
        source,
        header.header_metadata_offset,
        header.header_metadata_len,
        "metadata",
    )?;
    let body = range(source, header.body_offset, header.body_len, "stored body")?;
    let rewritten =
        newengine_assets_api::encode_list_file(newengine_assets_api::ListFileEncodeRequest {
            content_kind: header.content_kind,
            content_schema_version: spec.target.version.major,
            entry_count: header.entry_count,
            additional_flags: header.flags,
            min_size_class: header.size_class,
            header_metadata: metadata,
            body_stored: body,
            body_uncompressed_len: header.body_uncompressed_len,
            body_raw_hash: header.has_body_raw_hash().then_some(header.body_raw_hash),
            stable_file_id: header.has_stable_file_id().then_some(header.stable_file_id),
            import_settings_hash: header
                .has_import_settings_hash()
                .then_some(header.import_settings_hash),
        })?;
    let out_header = newengine_assets_api::parse_list_file_header(&rewritten)?;
    let out_meta = range(
        &rewritten,
        out_header.header_metadata_offset,
        out_header.header_metadata_len,
        "rewritten metadata",
    )?;
    let out_body = range(
        &rewritten,
        out_header.body_offset,
        out_header.body_len,
        "rewritten stored body",
    )?;
    if metadata != out_meta || body != out_body {
        return Err("payload-preserving migration changed metadata or stored body".to_owned());
    }
    Ok(rewritten)
}

fn reencode_ydd_current(
    spec: &MigrationSpec,
    source: &[u8],
    logical_path: &str,
) -> Result<Vec<u8>, String> {
    let header = verify_source(spec, source)?;
    let decoded = newengine_assets_api::decode_list_file_envelope(
        source,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD,
        logical_path,
    )?;
    let document = newengine_asset_format_nef8::ydd_binary::decode_ydd_binary_body(&decoded.body)?;
    let current_body = newengine_asset_format_nef8::encode_ydd_binary_body(&document)?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&current_body)
        .map_err(|e| format!("YDD migration deflate write failed: {e}"))?;
    let stored = encoder
        .finish()
        .map_err(|e| format!("YDD migration deflate finish failed: {e}"))?;
    let metadata = range(
        source,
        header.header_metadata_offset,
        header.header_metadata_len,
        "metadata",
    )?;
    let body_hash = (current_body.len()
        >= newengine_assets_api::LIST_FILE_FULL_HASH_BODY_THRESHOLD)
        .then(|| *blake3::hash(&current_body).as_bytes());
    let rewritten =
        newengine_assets_api::encode_list_file(newengine_assets_api::ListFileEncodeRequest {
            content_kind: header.content_kind,
            content_schema_version: spec.target.version.major,
            entry_count: header.entry_count,
            additional_flags: header.flags,
            min_size_class: header.size_class,
            header_metadata: metadata,
            body_stored: &stored,
            body_uncompressed_len: current_body.len() as u64,
            body_raw_hash: body_hash,
            stable_file_id: header.has_stable_file_id().then_some(header.stable_file_id),
            import_settings_hash: header
                .has_import_settings_hash()
                .then_some(header.import_settings_hash),
        })?;
    let check = newengine_assets_api::decode_list_file_envelope(
        &rewritten,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD,
        logical_path,
    )?;
    if check.header.content_schema_version != spec.target.version.major {
        return Err("YDD migration target schema mismatch".to_owned());
    }
    let roundtrip = newengine_asset_format_nef8::ydd_binary::decode_ydd_binary_body(&check.body)?;
    if roundtrip != document {
        return Err("YDD migration semantic document changed during v4 re-encode".to_owned());
    }
    Ok(rewritten)
}

fn rewrite_authored_schema(spec: &MigrationSpec, source: &[u8]) -> Result<Vec<u8>, String> {
    let from = spec
        .source
        .representation_id
        .ok_or_else(|| format!("migration '{}' missing source representation id", spec.id))?;
    let to = spec
        .target
        .representation_id
        .ok_or_else(|| format!("migration '{}' missing target representation id", spec.id))?;
    let text = std::str::from_utf8(source)
        .map_err(|e| format!("migration '{}' authored XML is not UTF-8: {e}", spec.id))?;
    let doc = roxmltree::Document::parse(text)
        .map_err(|e| format!("migration '{}' XML parse failed: {e}", spec.id))?;
    let root = doc.root_element();
    let actual = root.attribute("schema").unwrap_or_default();
    if actual != from {
        return Err(format!(
            "migration '{}' source schema='{}' expected='{}'",
            spec.id, actual, from
        ));
    }
    let double = format!("schema=\"{}\"", from);
    let single = format!("schema='{}'", from);
    let rewritten = if text.contains(&double) {
        text.replacen(&double, &format!("schema=\"{}\"", to), 1)
    } else if text.contains(&single) {
        text.replacen(&single, &format!("schema='{}'", to), 1)
    } else {
        return Err(format!(
            "migration '{}' could not locate root schema attribute text",
            spec.id
        ));
    };
    let check = roxmltree::Document::parse(&rewritten)
        .map_err(|e| format!("migration '{}' rewritten XML parse failed: {e}", spec.id))?;
    if check.root_element().attribute("schema") != Some(to) {
        return Err(format!(
            "migration '{}' target authored schema verification failed",
            spec.id
        ));
    }
    Ok(rewritten.into_bytes())
}

pub fn verify_target(spec: &MigrationSpec, bytes: &[u8], logical_path: &str) -> Result<(), String> {
    match spec.strategy {
        MigrationStrategy::EnvelopeSchemaRewrite | MigrationStrategy::SemanticReencode => {
            let header = newengine_assets_api::parse_list_file_header(bytes)?;
            let expected_kind = spec
                .corpus_gate
                .content_kind
                .ok_or_else(|| format!("migration '{}' missing target content kind", spec.id))?;
            if header.content_kind != expected_kind
                || header.content_schema_version != spec.target.version.major
            {
                return Err(format!(
                    "migration '{}' target ListFile mismatch kind={} schema={}",
                    spec.id, header.content_kind, header.content_schema_version
                ));
            }
            if spec.strategy == MigrationStrategy::SemanticReencode {
                let decoded = newengine_assets_api::decode_list_file_envelope(
                    bytes,
                    expected_kind,
                    logical_path,
                )?;
                let _ =
                    newengine_asset_format_nef8::ydd_binary::decode_ydd_binary_body(&decoded.body)?;
            }
            Ok(())
        }
        MigrationStrategy::AuthoredSchemaRewrite => {
            let to = spec.target.representation_id.ok_or_else(|| {
                format!("migration '{}' missing target representation id", spec.id)
            })?;
            let text = std::str::from_utf8(bytes)
                .map_err(|e| format!("target authored XML is not UTF-8: {e}"))?;
            let doc = roxmltree::Document::parse(text)
                .map_err(|e| format!("target authored XML parse failed: {e}"))?;
            if doc.root_element().attribute("schema") != Some(to) {
                return Err(format!(
                    "migration '{}' target representation mismatch",
                    spec.id
                ));
            }
            Ok(())
        }
    }
}

pub fn migrate_bytes(
    spec: &MigrationSpec,
    source: &[u8],
    logical_path: &str,
) -> Result<Vec<u8>, String> {
    match spec.strategy {
        MigrationStrategy::EnvelopeSchemaRewrite => rewrite_envelope_schema(spec, source),
        MigrationStrategy::SemanticReencode => reencode_ydd_current(spec, source, logical_path),
        MigrationStrategy::AuthoredSchemaRewrite => rewrite_authored_schema(spec, source),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct MigrationCorpusReport {
    pub migration_id: String,
    pub source_count: usize,
    pub target_count: usize,
    pub other_versions: BTreeMap<u16, usize>,
    pub other_representations: BTreeMap<String, usize>,
    pub source_files: Vec<String>,
}

fn collect_suffix(root: &Path, suffix: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_suffix(&path, suffix, out);
        } else if path
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(&suffix.to_ascii_lowercase())
        {
            out.push(path);
        }
    }
}

pub fn scan_corpus(
    repo_root: &Path,
    spec: &MigrationSpec,
) -> Result<MigrationCorpusReport, Vec<String>> {
    let mut files = Vec::new();
    for rel in spec.corpus_gate.roots {
        collect_suffix(
            &repo_root.join(rel),
            spec.corpus_gate.file_suffix,
            &mut files,
        );
    }
    let mut errors = Vec::new();
    let mut source_count = 0usize;
    let mut target_count = 0usize;
    let mut other_versions = BTreeMap::new();
    let mut other_representations = BTreeMap::new();
    let mut source_files = Vec::new();
    for path in files {
        match spec.strategy {
            MigrationStrategy::EnvelopeSchemaRewrite | MigrationStrategy::SemanticReencode => {
                match std::fs::read(&path).and_then(|bytes| {
                    newengine_assets_api::parse_list_file_header(&bytes)
                        .map_err(std::io::Error::other)
                }) {
                    Ok(header) if Some(header.content_kind) == spec.corpus_gate.content_kind => {
                        if spec
                            .corpus_gate
                            .source_versions
                            .contains(&header.content_schema_version)
                        {
                            source_count += 1;
                            source_files.push(path.to_string_lossy().into_owned());
                        } else if header.content_schema_version == spec.corpus_gate.target_version {
                            target_count += 1;
                        } else {
                            *other_versions
                                .entry(header.content_schema_version)
                                .or_default() += 1;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => errors.push(format!("{}: {error}", path.display())),
                }
            }
            MigrationStrategy::AuthoredSchemaRewrite => match std::fs::read_to_string(&path) {
                Ok(text) => match roxmltree::Document::parse(&text) {
                    Ok(doc) => {
                        let schema = doc.root_element().attribute("schema").unwrap_or_default();
                        if Some(schema) == spec.source.representation_id {
                            source_count += 1;
                            source_files.push(path.to_string_lossy().into_owned());
                        } else if Some(schema) == spec.target.representation_id {
                            target_count += 1;
                        } else {
                            *other_representations.entry(schema.to_owned()).or_default() += 1;
                        }
                    }
                    Err(error) => {
                        errors.push(format!("{}: XML parse failed: {error}", path.display()))
                    }
                },
                Err(error) => errors.push(format!("{}: {error}", path.display())),
            },
        }
    }
    if errors.is_empty() {
        Ok(MigrationCorpusReport {
            migration_id: spec.id.to_owned(),
            source_count,
            target_count,
            other_versions,
            other_representations,
            source_files,
        })
    } else {
        Err(errors)
    }
}

pub fn validate_corpus_canonical(
    repo_root: &Path,
) -> Result<Vec<MigrationCorpusReport>, Vec<String>> {
    let mut reports = Vec::new();
    let mut errors = Vec::new();
    for spec in MIGRATIONS {
        match scan_corpus(repo_root, spec) {
            Ok(report) => {
                if spec.corpus_gate.require_zero_source_after_migration && report.source_count != 0
                {
                    errors.push(format!(
                        "migration '{}' still has {} legacy source artifact(s)",
                        spec.id, report.source_count
                    ));
                }
                if !report.other_versions.is_empty() {
                    errors.push(format!(
                        "migration '{}' corpus has unregistered schema versions {:?}",
                        spec.id, report.other_versions
                    ));
                }
                if !report.other_representations.is_empty() {
                    errors.push(format!(
                        "migration '{}' corpus has unregistered representations {:?}",
                        spec.id, report.other_representations
                    ));
                }
                reports.push(report);
            }
            Err(items) => errors.extend(items),
        }
    }
    if errors.is_empty() {
        Ok(reports)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    include!("tests/tests.rs");
}
