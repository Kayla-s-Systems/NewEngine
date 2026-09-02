#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_context_handles_have_distinct_instance_identity() {
        let a = create_host_context();
        let b = create_host_context();
        assert_ne!(a.identity(), b.identity());
        assert_ne!(a.instance_id(), b.instance_id());
    }

    #[test]
    fn default_host_context_does_not_capture_process_environment() {
        let context = create_host_context();
        assert!(environment_snapshot_utf8().is_empty());
        assert_eq!(current_host_context().identity(), context.identity());
    }

    #[test]
    fn runtime_contract_catalog_is_instance_scoped() {
        let a = create_host_context();
        let b = create_host_context();
        let contract = newengine_runtime_contract_catalog::RuntimeContractSpec {
            key: "test.instance.contract".to_owned(),
            kind: newengine_runtime_contract_catalog::ContractKind::Protocol,
            version: newengine_runtime_contract_catalog::ContractVersion::major(1),
            compatibility: newengine_runtime_contract_catalog::ContractCompatibility::SameMajor,
            owner: "test.instance.owner".to_owned(),
            advertised_id: Some("test.instance.contract.v1".to_owned()),
        };
        {
            let mut catalog = a.inner.runtime_contract_catalog.lock().unwrap();
            catalog
                .replace_plugin_contracts("test.instance.owner", vec![contract])
                .unwrap();
        }
        assert!(a.runtime_contract("test.instance.contract").is_some());
        assert!(b.runtime_contract("test.instance.contract").is_none());
        assert!(a.runtime_contract("render.provider.abi").is_some());
        assert!(b.runtime_contract("render.provider.abi").is_some());
    }

    #[test]
    fn environment_snapshots_are_instance_scoped() {
        let a = create_host_context();
        a.replace_environment_snapshot([(
            OsString::from("NEWENGINE_TEST_INSTANCE_ENV"),
            OsString::from("alpha"),
        )]);
        let b = create_host_context();
        b.replace_environment_snapshot([(
            OsString::from("NEWENGINE_TEST_INSTANCE_ENV"),
            OsString::from("beta"),
        )]);

        assert_eq!(
            a.environment_var("NEWENGINE_TEST_INSTANCE_ENV").as_deref(),
            Some("alpha")
        );
        assert_eq!(
            b.environment_var("NEWENGINE_TEST_INSTANCE_ENV").as_deref(),
            Some("beta")
        );

        activate_host_context(&a);
        assert_eq!(
            environment_var("NEWENGINE_TEST_INSTANCE_ENV").as_deref(),
            Some("alpha")
        );
        activate_host_context(&b);
        assert_eq!(
            environment_var("NEWENGINE_TEST_INSTANCE_ENV").as_deref(),
            Some("beta")
        );
    }

    #[test]
    fn explicit_environment_snapshots_and_overrides_are_instance_scoped() {
        let a = create_host_context_with_environment_snapshot([(
            OsString::from("NEWENGINE_TEST_EXPLICIT_ENV"),
            OsString::from("alpha"),
        )]);
        let b = create_host_context_with_environment_snapshot([(
            OsString::from("NEWENGINE_TEST_EXPLICIT_ENV"),
            OsString::from("beta"),
        )]);

        a.set_environment_var("NEWENGINE_TEST_EXPLICIT_ENV", "gamma");
        assert_eq!(
            a.environment_var("NEWENGINE_TEST_EXPLICIT_ENV").as_deref(),
            Some("gamma")
        );
        assert_eq!(
            b.environment_var("NEWENGINE_TEST_EXPLICIT_ENV").as_deref(),
            Some("beta")
        );

        a.remove_environment_var("NEWENGINE_TEST_EXPLICIT_ENV");
        assert!(a.environment_var("NEWENGINE_TEST_EXPLICIT_ENV").is_none());
        assert_eq!(
            b.environment_var("NEWENGINE_TEST_EXPLICIT_ENV").as_deref(),
            Some("beta")
        );
    }

    #[test]
    fn implicit_unbound_thread_context_has_no_process_environment_snapshot() {
        let snapshot = std::thread::spawn(environment_snapshot_utf8)
            .join()
            .expect("unbound HostContext probe thread panicked");
        assert!(snapshot.is_empty());
    }

    #[test]
    fn scoped_host_context_restores_previous_instance() {
        let outer = create_host_context();
        let inner = create_host_context();
        activate_host_context(&outer);
        assert_eq!(current_host_context().identity(), outer.identity());

        with_host_context(&inner, || {
            assert_eq!(current_host_context().identity(), inner.identity());
        });

        assert_eq!(current_host_context().identity(), outer.identity());
    }
}
