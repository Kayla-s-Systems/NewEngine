#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_contract_api::{ContractKind, ContractSpec};
use newengine_plugin_api::{CapabilityKind, CapabilityRole, PluginDescriptor};
use newengine_service_api::BackendServiceSpec;

mod dto_parity;
mod tool_runtime;
pub use dto_parity::*;
pub use tool_runtime::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAbiConformance {
    pub gateway_id: String,
    pub backend_capability_id: String,
    pub provider_service_id: String,
    pub provider_abi: String,
    pub contract_key: String,
}

/// Validate a provider descriptor against the engine-owned backend route and ABI contract.
///
/// This is deliberately a conformance check, not runtime admission policy. P6 owns
/// hard compatibility rejection. The route metadata is normalized by plugin-host's
/// canonical parser so this crate does not implement a second descriptor parser.
pub fn validate_provider_abi(
    descriptor: &PluginDescriptor,
    backend: BackendServiceSpec,
    expected_abi: ContractSpec,
) -> Result<ProviderAbiConformance, Vec<String>> {
    let mut errors = Vec::new();
    if expected_abi.kind != ContractKind::Abi {
        errors.push(format!(
            "expected contract '{}' is kind '{}', not ABI",
            expected_abi.key,
            expected_abi.kind.as_str()
        ));
    }
    let Some(expected_id) = expected_abi.advertised_id else {
        errors.push(format!(
            "expected ABI contract '{}' has no advertised id",
            expected_abi.key
        ));
        return Err(errors);
    };
    let registered = newengine_contract_registry::contract_by_advertised_id(expected_id);
    match registered {
        None => errors.push(format!("ABI id '{expected_id}' is not registered")),
        Some(spec) if spec.key != expected_abi.key || spec.kind != ContractKind::Abi => errors.push(
            format!(
                "ABI id '{expected_id}' resolves to key='{}' kind='{}', expected key='{}' kind='abi'",
                spec.key,
                spec.kind.as_str(),
                expected_abi.key
            ),
        ),
        Some(_) => {}
    }

    let mut routes = newengine_plugin_host::descriptor_gateway_capabilities(descriptor)
        .into_iter()
        .filter(|route| {
            route.gateway_id == backend.engine_gateway_id
                && route.backend_capability_id == backend.backend_capability_id
        })
        .collect::<Vec<_>>();
    if routes.len() != 1 {
        errors.push(format!(
            "provider '{}' exposes {} routes for gateway='{}' capability='{}'; expected exactly one",
            descriptor.id,
            routes.len(),
            backend.engine_gateway_id,
            backend.backend_capability_id
        ));
        return Err(errors);
    }
    let route = routes.remove(0);
    let Some(provider_service_id) = route.provider_service_id.as_deref() else {
        errors.push(format!(
            "provider '{}' route '{}' has no service contract",
            descriptor.id, backend.engine_gateway_id
        ));
        return Err(errors);
    };
    let declares_service = descriptor.capabilities.iter().any(|cap| {
        cap.role == CapabilityRole::Provides
            && cap.kind == CapabilityKind::ServiceV1
            && cap.id.as_str() == provider_service_id
    });
    if !declares_service {
        errors.push(format!(
            "provider '{}' routes gateway='{}' to service='{}' but does not declare that ServiceV1",
            descriptor.id, backend.engine_gateway_id, provider_service_id
        ));
    }

    let provider_abi = route.provider_abi.as_deref().unwrap_or_default();
    if provider_abi.is_empty() {
        errors.push(format!(
            "provider '{}' route '{}' does not advertise provider_abi",
            descriptor.id, backend.engine_gateway_id
        ));
    } else if provider_abi != expected_id {
        errors.push(format!(
            "provider '{}' route '{}' advertises ABI='{}', expected='{}'",
            descriptor.id, backend.engine_gateway_id, provider_abi, expected_id
        ));
    } else if newengine_contract_registry::contract_by_advertised_id(provider_abi).is_none() {
        errors.push(format!(
            "provider '{}' advertises unregistered ABI id='{}'",
            descriptor.id, provider_abi
        ));
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(ProviderAbiConformance {
        gateway_id: route.gateway_id,
        backend_capability_id: route.backend_capability_id,
        provider_service_id: provider_service_id.to_owned(),
        provider_abi: provider_abi.to_owned(),
        contract_key: expected_abi.key.to_owned(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveRouteAbiConformance {
    pub gateway_id: String,
    pub provider_owner_id: String,
    pub provider_service_id: String,
    pub provider_abi: String,
    pub contract_key: String,
}

/// Validate an already-selected runtime gateway route against the domain ABI registry.
/// This remains diagnostic/conformance-only; P6 owns admission/rejection policy.
pub fn validate_active_route_abi(
    route: &newengine_plugin_host::EngineGatewayRouteSnapshot,
    backend: BackendServiceSpec,
    expected_abi: ContractSpec,
) -> Result<ActiveRouteAbiConformance, Vec<String>> {
    let mut errors = Vec::new();
    if !route.active || route.selection_state != "active" {
        errors.push(format!(
            "gateway '{}' route owner='{}' is not active selection_state='{}'",
            route.gateway_id, route.provider_owner_id, route.selection_state
        ));
    }
    if route.gateway_id != backend.engine_gateway_id {
        errors.push(format!(
            "active route gateway mismatch got='{}' expected='{}'",
            route.gateway_id, backend.engine_gateway_id
        ));
    }
    if route.backend_capability_id != backend.backend_capability_id {
        errors.push(format!(
            "active route capability mismatch got='{}' expected='{}'",
            route.backend_capability_id, backend.backend_capability_id
        ));
    }
    if route.provider_service_id.trim().is_empty() {
        errors.push(format!(
            "active route gateway='{}' has empty routed service id",
            route.gateway_id
        ));
    }
    if expected_abi.kind != ContractKind::Abi {
        errors.push(format!(
            "expected contract '{}' is kind '{}', not ABI",
            expected_abi.key,
            expected_abi.kind.as_str()
        ));
    }
    let expected_id = expected_abi.advertised_id.unwrap_or_default();
    if expected_id.is_empty() {
        errors.push(format!(
            "expected ABI contract '{}' has no advertised id",
            expected_abi.key
        ));
    }
    let provider_abi = route.provider_abi.as_deref().unwrap_or_default();
    if provider_abi.is_empty() {
        errors.push(format!(
            "active route gateway='{}' owner='{}' does not advertise provider_abi",
            route.gateway_id, route.provider_owner_id
        ));
    } else if provider_abi != expected_id {
        errors.push(format!(
            "active route gateway='{}' advertises ABI='{}', expected='{}'",
            route.gateway_id, provider_abi, expected_id
        ));
    }
    match newengine_contract_registry::contract_by_advertised_id(provider_abi) {
        None if !provider_abi.is_empty() => errors.push(format!(
            "active route gateway='{}' advertises unregistered ABI id='{}'",
            route.gateway_id, provider_abi
        )),
        Some(spec) if spec.key != expected_abi.key || spec.kind != ContractKind::Abi => errors
            .push(format!(
                "active route ABI '{}' resolves to key='{}' kind='{}', expected key='{}'",
                provider_abi,
                spec.key,
                spec.kind.as_str(),
                expected_abi.key
            )),
        _ => {}
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(ActiveRouteAbiConformance {
        gateway_id: route.gateway_id.clone(),
        provider_owner_id: route.provider_owner_id.clone(),
        provider_service_id: route.provider_service_id.clone(),
        provider_abi: provider_abi.to_owned(),
        contract_key: expected_abi.key.to_owned(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListFileContractConformance {
    pub wire_version: u16,
    pub content_kind: u32,
    pub content_schema_version: u16,
    pub schema_contract_key: String,
}

/// Validate a canonical NEF8/ListFile envelope against the typed wire and
/// domain-schema registry. Parsing is delegated to newengine-assets-api; this
/// function never reads binary offsets itself.
pub fn validate_list_file_contract(
    bytes: &[u8],
    expected_content_kind: u32,
    expected_schema: ContractSpec,
) -> Result<ListFileContractConformance, Vec<String>> {
    validate_list_file_contract_with_read_compatibility(
        bytes,
        expected_content_kind,
        expected_schema,
        &[],
    )
}

/// Validate a ListFile against the current registered schema while allowing an
/// explicit, format-owner-defined set of historical schema versions for read
/// compatibility. This does not weaken the current producer contract: callers
/// of `validate_list_file_contract` remain exact/registry-driven.
pub fn validate_list_file_contract_with_read_compatibility(
    bytes: &[u8],
    expected_content_kind: u32,
    expected_schema: ContractSpec,
    readable_legacy_schema_versions: &[u16],
) -> Result<ListFileContractConformance, Vec<String>> {
    let mut errors = Vec::new();
    let header = match newengine_assets_api::parse_list_file_header(bytes) {
        Ok(header) => header,
        Err(error) => return Err(vec![format!("NEF8 parse failed: {error}")]),
    };
    let wire = newengine_contract_registry::contract("asset.nef8.wire")
        .expect("NEF8 wire contract must be registered");
    let offered_wire = newengine_contract_api::ContractVersion::major(header.version);
    if !wire.accepts_version(offered_wire) {
        errors.push(format!(
            "NEF8 wire version {} is incompatible with registered {}",
            header.version, wire.version
        ));
    }
    if expected_schema.kind != ContractKind::Schema {
        errors.push(format!(
            "expected content contract '{}' is kind '{}', not schema",
            expected_schema.key,
            expected_schema.kind.as_str()
        ));
    }
    let registered_schema = newengine_contract_registry::contract(expected_schema.key);
    match registered_schema {
        None => errors.push(format!(
            "content schema contract '{}' is not registered",
            expected_schema.key
        )),
        Some(spec) if *spec != expected_schema => errors.push(format!(
            "content schema contract '{}' does not match authoritative registry spec",
            expected_schema.key
        )),
        Some(_) => {}
    }
    if header.content_kind != expected_content_kind {
        errors.push(format!(
            "ListFile content kind mismatch: got={} expected={}",
            header.content_kind, expected_content_kind
        ));
    }
    let offered_schema =
        newengine_contract_api::ContractVersion::major(header.content_schema_version);
    let accepted_by_current_contract = expected_schema.accepts_version(offered_schema);
    let accepted_as_read_compatibility = readable_legacy_schema_versions
        .iter()
        .copied()
        .any(|version| version == header.content_schema_version);
    if !accepted_by_current_contract && !accepted_as_read_compatibility {
        errors.push(format!(
            "ListFile content schema version {} is incompatible with '{}' {} and readable legacy versions {:?}",
            header.content_schema_version,
            expected_schema.key,
            expected_schema.version,
            readable_legacy_schema_versions,
        ));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(ListFileContractConformance {
        wire_version: header.version,
        content_kind: header.content_kind,
        content_schema_version: header.content_schema_version,
        schema_contract_key: expected_schema.key.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_plugin_api::prelude::*;

    fn render_descriptor(
        provider_abi: Option<&'static str>,
        declare_service: bool,
    ) -> PluginDescriptor {
        let mut builder =
            PluginDescriptor::builder("test.render", "TestRender", "1.0.0", PluginKind::Runtime);
        if declare_service {
            builder = builder.provides_service(newengine_render_api::RENDER_SERVICE_ID, 1, "{}");
        }
        let mut route =
            BackendRouteDescriptor::new(newengine_render_api::RENDER_BACKEND_SERVICE_SPEC)
                .provider_route("engine.render.test")
                .priority(1);
        if let Some(provider_abi) = provider_abi {
            route = route.provider_abi(provider_abi);
        }
        builder
            .push(CapabilityDesc::backend_route(
                newengine_render_api::RENDER_BACKEND_CAPABILITY_ID,
                route,
            ))
            .build()
    }

    #[test]
    fn matching_provider_descriptor_is_conformant() {
        let descriptor =
            render_descriptor(Some(newengine_render_api::RENDER_PROVIDER_ABI_ID), true);
        let report = validate_provider_abi(
            &descriptor,
            newengine_render_api::RENDER_BACKEND_SERVICE_SPEC,
            newengine_render_api::RENDER_PROVIDER_ABI_CONTRACT_SPEC,
        )
        .expect("conformant render provider");
        assert_eq!(
            report.provider_abi,
            newengine_render_api::RENDER_PROVIDER_ABI_ID
        );
    }

    #[test]
    fn missing_provider_abi_is_rejected_by_conformance_suite() {
        let descriptor = render_descriptor(None, true);
        let errors = validate_provider_abi(
            &descriptor,
            newengine_render_api::RENDER_BACKEND_SERVICE_SPEC,
            newengine_render_api::RENDER_PROVIDER_ABI_CONTRACT_SPEC,
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.contains("does not advertise provider_abi")));
    }

    #[test]
    fn wrong_registered_domain_abi_is_rejected() {
        let descriptor = render_descriptor(Some(newengine_ui_api::UI_PROVIDER_ABI_ID), true);
        let errors = validate_provider_abi(
            &descriptor,
            newengine_render_api::RENDER_BACKEND_SERVICE_SPEC,
            newengine_render_api::RENDER_PROVIDER_ABI_CONTRACT_SPEC,
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.contains("expected='newengine.render-provider/v1'")));
    }

    #[test]
    fn routed_service_must_be_declared() {
        let descriptor =
            render_descriptor(Some(newengine_render_api::RENDER_PROVIDER_ABI_ID), false);
        let errors = validate_provider_abi(
            &descriptor,
            newengine_render_api::RENDER_BACKEND_SERVICE_SPEC,
            newengine_render_api::RENDER_PROVIDER_ABI_CONTRACT_SPEC,
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.contains("does not declare that ServiceV1")));
    }

    fn encoded_listfile(content_kind: u32, schema_version: u16) -> Vec<u8> {
        newengine_assets_api::encode_list_file(newengine_assets_api::ListFileEncodeRequest {
            content_kind,
            content_schema_version: schema_version,
            entry_count: 1,
            additional_flags: 0,
            min_size_class: newengine_assets_api::LIST_FILE_HEADER_SIZE_CLASS_MIN,
            header_metadata: &[],
            body_stored: b"fixture",
            body_uncompressed_len: 7,
            body_raw_hash: None,
            stable_file_id: None,
            import_settings_hash: None,
        })
        .expect("fixture encode")
    }

    #[test]
    fn canonical_ytyp_listfile_matches_wire_and_schema_registry() {
        let bytes = encoded_listfile(
            newengine_asset_format_nef8::ytyp::CONTENT_KIND,
            newengine_asset_format_nef8::ytyp::CONTENT_SCHEMA_VERSION,
        );
        let report = validate_list_file_contract(
            &bytes,
            newengine_asset_format_nef8::ytyp::CONTENT_KIND,
            newengine_asset_format_nef8::ytyp::CONTENT_SCHEMA_CONTRACT_SPEC,
        )
        .expect("canonical YTYP contract");
        assert_eq!(report.wire_version, newengine_assets_api::LIST_FILE_VERSION);
    }

    #[test]
    fn unsupported_nef8_wire_version_is_rejected_by_canonical_parser() {
        let mut bytes = encoded_listfile(
            newengine_asset_format_nef8::ytyp::CONTENT_KIND,
            newengine_asset_format_nef8::ytyp::CONTENT_SCHEMA_VERSION,
        );
        bytes[4] = (newengine_assets_api::LIST_FILE_VERSION + 1) as u8;
        let errors = validate_list_file_contract(
            &bytes,
            newengine_asset_format_nef8::ytyp::CONTENT_KIND,
            newengine_asset_format_nef8::ytyp::CONTENT_SCHEMA_CONTRACT_SPEC,
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.contains("unsupported NEF8 wire version")));
    }

    #[test]
    fn wrong_content_schema_version_is_rejected() {
        let bytes = encoded_listfile(
            newengine_asset_format_nef8::ytyp::CONTENT_KIND,
            newengine_asset_format_nef8::ytyp::CONTENT_SCHEMA_VERSION + 1,
        );
        let errors = validate_list_file_contract(
            &bytes,
            newengine_asset_format_nef8::ytyp::CONTENT_KIND,
            newengine_asset_format_nef8::ytyp::CONTENT_SCHEMA_CONTRACT_SPEC,
        )
        .unwrap_err();
        assert!(errors.iter().any(|e| e.contains("content schema version")));
    }

    fn collect_files(root: &std::path::Path, extension: &str, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files(&path, extension, out);
            } else if path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(extension))
            {
                out.push(path);
            }
        }
    }

    #[test]
    fn production_listfile_corpus_conforms_to_registered_contracts() {
        let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let neocore = crate_dir
            .parent()
            .and_then(std::path::Path::parent)
            .expect("neocore root");
        let repo_root = neocore
            .parent()
            .and_then(std::path::Path::parent)
            .expect("NorthStar root");
        let roots = [
            repo_root.join("Engine/Content"),
            repo_root.join("Projects"),
            repo_root.join("Shared"),
        ];
        let mut ytd = Vec::new();
        let mut ydd = Vec::new();
        let mut ytyp = Vec::new();
        let mut nemat = Vec::new();
        let mut neui = Vec::new();
        for root in &roots {
            collect_files(root, "ytd", &mut ytd);
            collect_files(root, "ydd", &mut ydd);
            collect_files(root, "ytyp", &mut ytyp);
            collect_files(root, "nemat", &mut nemat);
            collect_files(root, "neui", &mut neui);
        }
        for (name, files) in [
            ("YTD", &ytd),
            ("YDD", &ydd),
            ("YTYP", &ytyp),
            ("NEMAT", &nemat),
            ("NEUI", &neui),
        ] {
            assert!(
                !files.is_empty(),
                "production {name} corpus must not be empty"
            );
        }
        let mut errors = Vec::new();
        let mut validate_group =
            |files: &[std::path::PathBuf],
             kind: u32,
             spec: ContractSpec,
             readable_legacy_schema_versions: &[u16]| {
                for path in files {
                    match std::fs::read(path) {
                        Ok(bytes) => {
                            if let Err(items) = validate_list_file_contract_with_read_compatibility(
                                &bytes,
                                kind,
                                spec,
                                readable_legacy_schema_versions,
                            ) {
                                errors.push(format!("{}: {}", path.display(), items.join("; ")));
                            }
                        }
                        Err(error) => {
                            errors.push(format!("{}: read failed: {error}", path.display()))
                        }
                    }
                }
            };
        validate_group(
            &ytd,
            newengine_asset_format_nef8::ytd::CONTENT_KIND,
            newengine_asset_format_nef8::ytd::CONTENT_SCHEMA_CONTRACT_SPEC,
            newengine_asset_format_nef8::ytd::READABLE_CONTENT_SCHEMA_VERSIONS,
        );
        validate_group(
            &ydd,
            newengine_asset_format_nef8::ydd::CONTENT_KIND,
            newengine_asset_format_nef8::YDD_BINARY_CONTRACT_SPEC,
            newengine_asset_format_nef8::ydd::READABLE_CONTENT_SCHEMA_VERSIONS,
        );
        validate_group(
            &ytyp,
            newengine_asset_format_nef8::ytyp::CONTENT_KIND,
            newengine_asset_format_nef8::ytyp::CONTENT_SCHEMA_CONTRACT_SPEC,
            &[],
        );
        validate_group(
            &nemat,
            newengine_asset_format_nef8::nemat::CONTENT_KIND,
            newengine_asset_format_nef8::nemat::CONTENT_SCHEMA_CONTRACT_SPEC,
            &[],
        );
        validate_group(
            &neui,
            newengine_asset_format_nef8::neui::CONTENT_KIND,
            newengine_asset_format_nef8::neui::CONTENT_SCHEMA_CONTRACT_SPEC,
            &[],
        );
        assert!(
            errors.is_empty(),
            "asset contract conformance failed:\n{}",
            errors.join("\n")
        );
        eprintln!(
            "P3 asset corpus conformance: ytd={} ydd={} ytyp={} nemat={} neui={}",
            ytd.len(),
            ydd.len(),
            ytyp.len(),
            nemat.len(),
            neui.len()
        );
    }
}
