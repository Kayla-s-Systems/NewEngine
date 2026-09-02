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
    pub format_module_id: String,
}

/// Validate a canonical NEF8/ListFile envelope against the authoritative
/// StarVault `AssetFileTypeDescriptor`. Asset-format schema policy is not part
/// of the core Contract Registry.
pub fn validate_list_file_descriptor_contract(
    bytes: &[u8],
    descriptor: &newengine_assets_api::AssetFileTypeDescriptor,
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
    let Some(expected_content_kind) = descriptor.content_kind else {
        errors.push(format!(
            "format module '{}' does not declare NEF8 content_kind",
            descriptor.module_id
        ));
        return Err(errors);
    };
    if header.content_kind != expected_content_kind {
        errors.push(format!(
            "ListFile content kind mismatch module='{}': got={} expected={}",
            descriptor.module_id, header.content_kind, expected_content_kind
        ));
    }
    if let Some(current) = descriptor.content_schema_version {
        let readable = descriptor
            .readable_content_schema_versions
            .iter()
            .copied()
            .any(|version| version == header.content_schema_version);
        if header.content_schema_version != current && !readable {
            errors.push(format!(
                "ListFile content schema version {} is incompatible with format module '{}' current={} readable={:?}",
                header.content_schema_version,
                descriptor.module_id,
                current,
                descriptor.readable_content_schema_versions,
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(ListFileContractConformance {
        wire_version: header.version,
        content_kind: header.content_kind,
        content_schema_version: header.content_schema_version,
        schema_contract_key: descriptor.schema_contract.clone(),
        format_module_id: descriptor.module_id.clone(),
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
        assert!(errors.iter().any(|e| e.contains(&format!(
            "expected='{}'",
            newengine_render_api::RENDER_PROVIDER_ABI_ID
        ))));
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

    fn provider_descriptor(
        kind: u32,
        schema: u16,
    ) -> newengine_assets_api::AssetFileTypeDescriptor {
        newengine_assets_api::AssetFileTypeDescriptor {
            module_id: "test.provider.format".to_owned(),
            extension: "provider".to_owned(),
            asset_kind: "provider_asset".to_owned(),
            content_kind: Some(kind),
            content_schema_version: Some(schema),
            readable_content_schema_versions: vec![schema],
            schema_contract: "provider.schema".to_owned(),
            handler_service: "asset.codec.listfile".to_owned(),
            semantic_gateway: "engine.test".to_owned(),
            gateway: "engine.test".to_owned(),
            runtime_ready: true,
            vfs_backed: true,
            native_container: true,
            ..Default::default()
        }
    }

    #[test]
    fn provider_declared_format_contract_is_conformant_without_core_registration() {
        let descriptor = provider_descriptor(0xE001, 7);
        let bytes = encoded_listfile(0xE001, 7);
        let report = validate_list_file_descriptor_contract(&bytes, &descriptor)
            .expect("provider descriptor conformance");
        assert_eq!(report.content_kind, 0xE001);
        assert_eq!(report.content_schema_version, 7);
        assert_eq!(report.format_module_id, "test.provider.format");
    }

    #[test]
    fn descriptor_content_kind_mismatch_is_rejected() {
        let descriptor = provider_descriptor(0xE001, 7);
        let bytes = encoded_listfile(0xE002, 7);
        let errors = validate_list_file_descriptor_contract(&bytes, &descriptor).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.contains("content kind mismatch")));
    }

    #[test]
    fn descriptor_schema_policy_is_authoritative() {
        let descriptor = provider_descriptor(0xE001, 7);
        let bytes = encoded_listfile(0xE001, 9);
        let errors = validate_list_file_descriptor_contract(&bytes, &descriptor).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.contains("content schema version")));
    }
}
