mod publication;
mod shutdown;
mod transaction;

pub(crate) use publication::{restore_provider_publication, snapshot_provider_publication};
pub(crate) use shutdown::{
    quiesce_provider_publication, shutdown_provider_publication_services,
    wait_for_provider_publication_quiescence,
};
pub use shutdown::{shutdown_services_by_owner, unregister_by_owner};
pub use transaction::ProviderRegistrationTransaction;
pub(crate) use transaction::{
    begin_provider_transaction, commit_provider_transaction, rollback_provider_transaction,
    stage_event_sink_registration, stage_gateway_route_registration,
    stage_plugin_descriptor_registration, stage_service_registration,
    staged_plugin_declares_service, validate_provider_transaction,
};

#[cfg(test)]
use super::state::{GatewayProviderRouteEntry, ProviderTransactionState};
#[cfg(test)]
use transaction::validate_staged_route_contracts;

#[cfg(test)]
mod contract_catalog_tests {
    use super::*;
    use newengine_runtime_contract_catalog::{
        ContractCompatibility, ContractKind, ContractVersion, RuntimeContractAuthority,
        RuntimeContractCatalog, RuntimeContractSpec,
    };

    fn route_with_abi(owner: &str, provider_abi: &str) -> GatewayProviderRouteEntry {
        GatewayProviderRouteEntry {
            gateway_id: "engine.test".to_owned(),
            service_kind: "test".to_owned(),
            provider_service_id: "test.service".to_owned(),
            provider_route_id: "engine.test.provider".to_owned(),
            provider_abi: Some(provider_abi.to_owned()),
            provider_owner_id: owner.to_owned(),
            backend_capability_id: "test.backend".to_owned(),
            backend_priority: 1,
            system_tags: Vec::new(),
            origin: crate::service_gateway::GatewayProviderOrigin::GamePlugin,
        }
    }

    fn descriptor(owner: &str, version: u16) -> newengine_plugin_api::PluginDescriptor {
        newengine_plugin_api::PluginDescriptor::builder(
            owner,
            "Contract Test",
            "1.0.0",
            newengine_plugin_api::PluginKind::Runtime,
        )
        .push(
            newengine_plugin_api::RuntimeContractDeclaration::new(
                "test.plugin.protocol",
                ContractKind::Protocol,
                ContractVersion::major(version),
                ContractCompatibility::SameMajor,
            )
            .advertised_id(format!("test.plugin.protocol.v{version}"))
            .into_capability(),
        )
        .build()
    }

    #[test]
    fn staged_provider_abi_resolves_from_same_transaction_contract() {
        let owner = "test.contract-route";
        let mut tx = ProviderTransactionState {
            owner_plugin_id: owner.to_owned(),
            ..ProviderTransactionState::default()
        };
        tx.staged_contracts.push(RuntimeContractSpec {
            key: "test.contract-route.abi".to_owned(),
            kind: ContractKind::Abi,
            version: ContractVersion::major(1),
            compatibility: ContractCompatibility::SameMajor,
            owner: owner.to_owned(),
            advertised_id: Some("test.contract-route/v1".to_owned()),
        });
        tx.staged_gateway_routes.insert(
            "engine.test::test.service".to_owned(),
            route_with_abi(owner, "test.contract-route/v1"),
        );

        assert!(validate_staged_route_contracts(&tx, &RuntimeContractCatalog::default()).is_ok());
    }

    #[test]
    fn provider_abi_must_resolve_to_known_abi_contract() {
        let owner = "test.contract-route-invalid";
        let mut tx = ProviderTransactionState {
            owner_plugin_id: owner.to_owned(),
            ..ProviderTransactionState::default()
        };
        tx.staged_gateway_routes.insert(
            "engine.test::test.service".to_owned(),
            route_with_abi(owner, "test.contract-route/missing"),
        );
        assert!(validate_staged_route_contracts(&tx, &RuntimeContractCatalog::default()).is_err());

        tx.staged_contracts.push(RuntimeContractSpec {
            key: "test.contract-route.protocol".to_owned(),
            kind: ContractKind::Protocol,
            version: ContractVersion::major(1),
            compatibility: ContractCompatibility::SameMajor,
            owner: owner.to_owned(),
            advertised_id: Some("test.contract-route/missing".to_owned()),
        });
        let error = validate_staged_route_contracts(&tx, &RuntimeContractCatalog::default())
            .expect_err("provider_abi must be an ABI contract");
        assert!(error.contains("expected kind='abi'"));
    }

    #[test]
    fn runtime_contract_is_atomic_with_provider_transaction() {
        let context = crate::host_context::create_host_context();
        crate::host_context::activate_host_context(&context);
        let owner = "test.contract-provider";

        begin_provider_transaction(owner).unwrap();
        stage_plugin_descriptor_registration(
            owner,
            descriptor(owner, 1),
            None,
            crate::service_gateway::GatewayProviderOrigin::GamePlugin,
        )
        .unwrap();

        assert!(crate::host_context::runtime_contract("test.plugin.protocol").is_none());
        validate_provider_transaction(owner).unwrap();
        commit_provider_transaction(owner).unwrap();

        let published = crate::host_context::runtime_contract("test.plugin.protocol").unwrap();
        assert_eq!(published.authority, RuntimeContractAuthority::Plugin);
        assert_eq!(published.spec.owner, owner);
        assert_eq!(published.spec.version, ContractVersion::major(1));
    }

    #[test]
    fn rollback_does_not_publish_staged_contract() {
        let context = crate::host_context::create_host_context();
        crate::host_context::activate_host_context(&context);
        let owner = "test.contract-rollback";

        begin_provider_transaction(owner).unwrap();
        stage_plugin_descriptor_registration(
            owner,
            descriptor(owner, 1),
            None,
            crate::service_gateway::GatewayProviderOrigin::GamePlugin,
        )
        .unwrap();
        rollback_provider_transaction(owner);

        assert!(crate::host_context::runtime_contract("test.plugin.protocol").is_none());
    }

    #[test]
    fn hot_reload_contract_swap_and_rollback_restore_previous_catalog() {
        let context = crate::host_context::create_host_context();
        crate::host_context::activate_host_context(&context);
        let owner = "test.contract-hot-reload";

        begin_provider_transaction(owner).unwrap();
        stage_plugin_descriptor_registration(
            owner,
            descriptor(owner, 1),
            None,
            crate::service_gateway::GatewayProviderOrigin::GamePlugin,
        )
        .unwrap();
        commit_provider_transaction(owner).unwrap();
        let previous = snapshot_provider_publication(owner);

        begin_provider_transaction(owner).unwrap();
        stage_plugin_descriptor_registration(
            owner,
            descriptor(owner, 2),
            None,
            crate::service_gateway::GatewayProviderOrigin::GamePlugin,
        )
        .unwrap();
        commit_provider_transaction(owner).unwrap();
        assert_eq!(
            crate::host_context::runtime_contract("test.plugin.protocol")
                .unwrap()
                .spec
                .version,
            ContractVersion::major(2)
        );
        assert!(
            crate::host_context::runtime_contract_by_advertised_id("test.plugin.protocol.v1")
                .is_none()
        );

        restore_provider_publication(owner, previous);
        assert_eq!(
            crate::host_context::runtime_contract("test.plugin.protocol")
                .unwrap()
                .spec
                .version,
            ContractVersion::major(1)
        );
        assert!(
            crate::host_context::runtime_contract_by_advertised_id("test.plugin.protocol.v1")
                .is_some()
        );
        assert!(
            crate::host_context::runtime_contract_by_advertised_id("test.plugin.protocol.v2")
                .is_none()
        );
    }

    #[test]
    fn unregister_removes_plugin_contract_but_keeps_normative_contracts() {
        let context = crate::host_context::create_host_context();
        crate::host_context::activate_host_context(&context);
        let owner = "test.contract-unregister";

        begin_provider_transaction(owner).unwrap();
        stage_plugin_descriptor_registration(
            owner,
            descriptor(owner, 1),
            None,
            crate::service_gateway::GatewayProviderOrigin::GamePlugin,
        )
        .unwrap();
        commit_provider_transaction(owner).unwrap();
        unregister_by_owner(owner);

        assert!(crate::host_context::runtime_contract("test.plugin.protocol").is_none());
        assert!(crate::host_context::runtime_contract("render.provider.abi").is_some());
    }
}
