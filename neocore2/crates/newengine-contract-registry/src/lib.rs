#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeSet;

pub use newengine_contract_api::{
    ContractCompatibility, ContractKind, ContractSpec, ContractVersion,
};

/// Normative compile-time Engine contract set. Runtime/plugin-owned contracts belong
/// in `newengine-runtime-contract-catalog` and must never mutate this trust root.
pub const ENGINE_CONTRACTS: &[ContractSpec] = &[
    newengine_assets_api::NEF8_WIRE_CONTRACT_SPEC,
    newengine_asset_format_nef8::YDD_BINARY_CONTRACT_SPEC,
    newengine_asset_format_nef8::ytd::CONTENT_SCHEMA_CONTRACT_SPEC,
    newengine_asset_format_nef8::nemat::CONTENT_SCHEMA_CONTRACT_SPEC,
    newengine_asset_format_nef8::nemat::AUTHORED_XML_CONTRACT_SPEC,
    newengine_asset_format_nef8::neui::CONTENT_SCHEMA_CONTRACT_SPEC,
    newengine_asset_format_nef8::ytyp::CONTENT_SCHEMA_CONTRACT_SPEC,
    newengine_project_api::PROJECT_MANIFEST_CONTRACT_SPEC,
    newengine_project_api::PROJECT_RUNTIME_PROFILE_ABI_CONTRACT_SPEC,
    newengine_game_module_api::GAME_MODULE_CONTRACT_SPEC,
    newengine_audio_api::AUDIO_PROVIDER_ABI_CONTRACT_SPEC,
    newengine_render_api::RENDER_PROVIDER_ABI_CONTRACT_SPEC,
    newengine_physics_api::PHYSICS_PROVIDER_ABI_CONTRACT_SPEC,
    newengine_ui_api::UI_PROVIDER_ABI_CONTRACT_SPEC,
    newengine_ui_api::UI_PROTOCOL_CONTRACT_SPEC,
    newengine_scripting_api::SCRIPTING_BINARY_PROTOCOL_CONTRACT_SPEC,
    newengine_scripting_api::SCRIPTING_WIRE_CONTRACT_SPEC,
];

/// Backward-compatible name for the normative Engine Contract Registry.
pub const CORE_CONTRACTS: &[ContractSpec] = ENGINE_CONTRACTS;

#[inline]
pub const fn contracts() -> &'static [ContractSpec] {
    ENGINE_CONTRACTS
}

pub fn contract(key: &str) -> Option<&'static ContractSpec> {
    ENGINE_CONTRACTS.iter().find(|spec| spec.key == key)
}

/// Resolve a contract by the stable token advertised on a provider/wire boundary.
pub fn contract_by_advertised_id(advertised_id: &str) -> Option<&'static ContractSpec> {
    ENGINE_CONTRACTS
        .iter()
        .find(|spec| spec.advertised_id == Some(advertised_id))
}

pub fn validate_registry() -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let mut keys = BTreeSet::new();
    let mut advertised = BTreeSet::new();
    for spec in ENGINE_CONTRACTS {
        if spec.key.trim().is_empty() {
            errors.push("contract registry contains empty key".to_owned());
        } else if !keys.insert(spec.key) {
            errors.push(format!("duplicate contract key '{}'", spec.key));
        }
        if spec.owner.trim().is_empty() {
            errors.push(format!("contract '{}' has empty owner", spec.key));
        }
        if spec.version.major == 0 {
            errors.push(format!("contract '{}' has zero major version", spec.key));
        }
        if let Some(id) = spec.advertised_id {
            if id.trim().is_empty() {
                errors.push(format!("contract '{}' has empty advertised id", spec.key));
            } else if !advertised.insert(id) {
                errors.push(format!("duplicate advertised contract id '{id}'"));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_unique_and_well_formed() {
        if let Err(errors) = validate_registry() {
            panic!("contract registry invalid: {}", errors.join("; "));
        }
    }

    #[test]
    fn roadmap_contracts_are_registered() {
        for key in [
            "asset.nef8.wire",
            "asset.ydd.body",
            "asset.ytd.schema",
            "asset.nemat.schema",
            "asset.nemat.authored_xml",
            "asset.neui.schema",
            "asset.ytyp.schema",
            "project.manifest",
            "runtime.profile.abi",
            "game.module.contract",
            "audio.provider.abi",
            "render.provider.abi",
            "physics.provider.abi",
            "ui.provider.abi",
            "ui.provider.protocol",
            "scripting.binary.protocol",
            "scripting.binary.wire",
        ] {
            assert!(contract(key).is_some(), "missing registry contract {key}");
        }
    }

    #[test]
    fn registered_versions_match_authoritative_constants() {
        assert_eq!(
            contract("asset.nef8.wire").unwrap().version.major,
            newengine_assets_api::LIST_FILE_VERSION
        );
        assert_eq!(
            contract("asset.ydd.body").unwrap().version.major as u32,
            newengine_asset_format_nef8::YDD_BINARY_SCHEMA_VERSION
        );
        assert_eq!(
            contract("scripting.binary.wire").unwrap().version.major,
            newengine_scripting_api::SCRIPTING_WIRE_VERSION_V1
        );
        assert_eq!(
            contract("audio.provider.abi").unwrap().version.major,
            newengine_audio_api::AUDIO_PROVIDER_ABI_VERSION
        );
    }
    #[test]
    fn advertised_id_lookup_resolves_provider_abi() {
        let spec = contract_by_advertised_id(newengine_render_api::RENDER_PROVIDER_ABI_ID)
            .expect("render provider ABI by advertised id");
        assert_eq!(spec.key, "render.provider.abi");
        assert_eq!(spec.kind, ContractKind::Abi);
    }
}
