fn main() {
    use newengine_migration_registry::{MigrationReversibility, MigrationStrategy};
    let rows = newengine_migration_registry::migrations()
        .iter()
        .map(|spec| {
            serde_json::json!({
                "migration_version": spec.migration_version,
                "id": spec.id,
                "source": {
                    "contract_key": spec.source.contract_key,
                    "version": {"major":spec.source.version.major,"minor":spec.source.version.minor,"patch":spec.source.version.patch},
                    "representation_id": spec.source.representation_id,
                },
                "target": {
                    "contract_key": spec.target.contract_key,
                    "version": {"major":spec.target.version.major,"minor":spec.target.version.minor,"patch":spec.target.version.patch},
                    "representation_id": spec.target.representation_id,
                },
                "strategy": match spec.strategy {
                    MigrationStrategy::EnvelopeSchemaRewrite => "envelope_schema_rewrite",
                    MigrationStrategy::SemanticReencode => "semantic_reencode",
                    MigrationStrategy::AuthoredSchemaRewrite => "authored_schema_rewrite",
                },
                "tool": {"workspace":spec.tool.workspace,"package":spec.tool.package,"example":spec.tool.example},
                "reversibility": match spec.reversibility {
                    MigrationReversibility::ExactPayloadPreserving => "exact_payload_preserving",
                    MigrationReversibility::SemanticOnly => "semantic_only",
                    MigrationReversibility::ExactTextExceptSchema => "exact_text_except_schema",
                },
                "backup_policy": "required_full_copy_with_sha256_manifest",
                "corpus_gate": {
                    "file_suffix": spec.corpus_gate.file_suffix,
                    "content_kind": spec.corpus_gate.content_kind,
                    "source_versions": spec.corpus_gate.source_versions,
                    "target_version": spec.corpus_gate.target_version,
                    "roots": spec.corpus_gate.roots,
                    "require_zero_source_after_migration": spec.corpus_gate.require_zero_source_after_migration,
                }
            })
        })
        .collect::<Vec<_>>();
    let payload = serde_json::json!({
        "schema": newengine_migration_registry::MIGRATION_REGISTRY_SCHEMA,
        "version": newengine_migration_registry::MIGRATION_REGISTRY_VERSION,
        "migrations": rows,
    });
    println!(
        "{}",
        serde_json::to_string(&payload).expect("migration registry json")
    );
}
