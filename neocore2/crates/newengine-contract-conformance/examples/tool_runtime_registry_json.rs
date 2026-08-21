fn main() {
    let specs = newengine_contract_conformance::tool_runtime_conformance_specs();
    let rows = specs
        .iter()
        .map(|spec| {
            let commands = spec
                .commands
                .iter()
                .map(|command| {
                    serde_json::json!({
                        "phase": command.phase.as_str(),
                        "args": command.args,
                    })
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "id": spec.id,
                "tool_key": spec.tool_key,
                "fixture": {
                    "kind": spec.fixture.kind.as_str(),
                    "testdata_name": spec.fixture.testdata_name,
                    "source_relative": spec.fixture.source_relative,
                },
                "output_relative": spec.output_relative,
                "content_kind": spec.content_kind,
                "schema_contract_key": spec.schema_contract_key,
                "readable_legacy_schema_versions": spec.readable_legacy_schema_versions,
                "commands": commands,
                "asset_manager_decode": spec.asset_manager_decode.map(|decode| serde_json::json!({
                    "workspace": decode.workspace.as_str(),
                    "package": decode.package,
                    "example": decode.example,
                    "output_kind": decode.output_kind,
                })),
                "runtime_decode": spec.runtime_decode.map(|decode| serde_json::json!({
                    "workspace": decode.workspace.as_str(),
                    "package": decode.package,
                    "example": decode.example,
                })),
                "canonical_projection": spec.canonical_projection.map(|projection| projection.as_str()),
            })
        })
        .collect::<Vec<_>>();
    let registry = serde_json::json!({
        "schema": newengine_contract_conformance::TOOL_RUNTIME_CONFORMANCE_REGISTRY_SCHEMA,
        "version": newengine_contract_conformance::TOOL_RUNTIME_CONFORMANCE_REGISTRY_VERSION,
        "specs": rows,
    });
    println!(
        "{}",
        serde_json::to_string(&registry).expect("serialize tool/runtime registry")
    );
}
