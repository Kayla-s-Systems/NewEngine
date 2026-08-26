#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};

pub use newengine_contract_api::{
    ContractCompatibility, ContractKind, ContractSpec, ContractVersion,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeContractAuthority {
    Engine,
    Plugin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeContractSpec {
    pub key: String,
    pub kind: ContractKind,
    pub version: ContractVersion,
    pub compatibility: ContractCompatibility,
    pub owner: String,
    pub advertised_id: Option<String>,
}

impl RuntimeContractSpec {
    pub fn from_engine(spec: &ContractSpec) -> Self {
        Self {
            key: spec.key.to_owned(),
            kind: spec.kind,
            version: spec.version,
            compatibility: spec.compatibility,
            owner: spec.owner.to_owned(),
            advertised_id: spec.advertised_id.map(str::to_owned),
        }
    }

    pub fn from_plugin_declaration(
        owner: &str,
        declaration: newengine_plugin_api::RuntimeContractDeclaration,
    ) -> Self {
        Self {
            key: declaration.key,
            kind: declaration.kind,
            version: declaration.version,
            compatibility: declaration.compatibility,
            owner: owner.to_owned(),
            advertised_id: declaration.advertised_id,
        }
    }

    #[inline]
    pub fn accepts_version(&self, offered: ContractVersion) -> bool {
        self.compatibility.accepts(self.version, offered)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeContractEntry {
    pub authority: RuntimeContractAuthority,
    pub spec: RuntimeContractSpec,
}

#[derive(Clone, Debug)]
pub struct RuntimeContractCatalog {
    by_key: BTreeMap<String, RuntimeContractEntry>,
    by_advertised_id: BTreeMap<String, String>,
}

impl Default for RuntimeContractCatalog {
    fn default() -> Self {
        Self::with_engine_contracts()
    }
}

impl RuntimeContractCatalog {
    pub fn with_engine_contracts() -> Self {
        let mut catalog = Self {
            by_key: BTreeMap::new(),
            by_advertised_id: BTreeMap::new(),
        };
        for spec in newengine_contract_registry::contracts() {
            catalog.insert_unchecked(RuntimeContractEntry {
                authority: RuntimeContractAuthority::Engine,
                spec: RuntimeContractSpec::from_engine(spec),
            });
        }
        catalog
    }

    #[inline]
    pub fn contract(&self, key: &str) -> Option<&RuntimeContractEntry> {
        self.by_key.get(key)
    }

    pub fn contract_by_advertised_id(&self, id: &str) -> Option<&RuntimeContractEntry> {
        let key = self.by_advertised_id.get(id)?;
        self.by_key.get(key)
    }

    /// Resolves either the stable registry key or an advertised boundary id to the
    /// same catalog entry. Consumers should canonicalize references through this
    /// method before comparing contracts across independently-authored metadata.
    pub fn resolve_contract_reference(&self, reference: &str) -> Option<&RuntimeContractEntry> {
        let reference = reference.trim();
        if reference.is_empty() {
            return None;
        }
        self.contract(reference)
            .or_else(|| self.contract_by_advertised_id(reference))
    }

    /// Canonical version-neutral registry key for a key or advertised-id reference.
    pub fn canonical_contract_key(&self, reference: &str) -> Option<&str> {
        self.resolve_contract_reference(reference)
            .map(|entry| entry.spec.key.as_str())
    }

    /// Validates a concrete major version advertised by provider metadata against
    /// the catalog's normative compatibility policy.
    pub fn validate_offered_major(
        &self,
        reference: &str,
        offered_major: u32,
    ) -> Result<&RuntimeContractEntry, String> {
        let entry = self
            .resolve_contract_reference(reference)
            .ok_or_else(|| format!("unknown runtime contract reference '{reference}'"))?;
        let major = u16::try_from(offered_major).map_err(|_| {
            format!(
                "runtime contract '{}' offered major {} exceeds u16",
                entry.spec.key, offered_major
            )
        })?;
        let offered = ContractVersion::major(major);
        if !entry.spec.accepts_version(offered) {
            return Err(format!(
                "runtime contract '{}' rejects offered version {} under compatibility {:?}; registered version is {}",
                entry.spec.key, offered, entry.spec.compatibility, entry.spec.version
            ));
        }
        Ok(entry)
    }

    /// Validates that a requirement's major-version interval intersects the set of
    /// versions accepted by the registered contract. Editor requirement metadata
    /// uses major ranges, so this deliberately validates at major granularity.
    pub fn validate_required_major_range(
        &self,
        reference: &str,
        min_major: u32,
        max_major: Option<u32>,
    ) -> Result<&RuntimeContractEntry, String> {
        if max_major.is_some_and(|max| max < min_major) {
            return Err(format!(
                "runtime contract requirement '{reference}' has invalid major range {min_major}..{:?}",
                max_major
            ));
        }
        let entry = self
            .resolve_contract_reference(reference)
            .ok_or_else(|| format!("unknown runtime contract reference '{reference}'"))?;
        let registered_major = u32::from(entry.spec.version.major);
        let intersects = match entry.spec.compatibility {
            ContractCompatibility::Exact | ContractCompatibility::SameMajor => {
                registered_major >= min_major && max_major.is_none_or(|max| registered_major <= max)
            }
            ContractCompatibility::AtLeast => {
                max_major.is_none_or(|max| max >= registered_major.max(min_major))
            }
        };
        if !intersects {
            return Err(format!(
                "runtime contract '{}' registered version {} compatibility {:?} does not intersect required major range {}..{}",
                entry.spec.key,
                entry.spec.version,
                entry.spec.compatibility,
                min_major,
                max_major
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "*".to_owned())
            ));
        }
        Ok(entry)
    }

    pub fn list(&self) -> Vec<RuntimeContractEntry> {
        self.by_key.values().cloned().collect()
    }

    pub fn contracts_by_owner(&self, owner: &str) -> Vec<RuntimeContractSpec> {
        self.by_key
            .values()
            .filter(|entry| {
                entry.authority == RuntimeContractAuthority::Plugin && entry.spec.owner == owner
            })
            .map(|entry| entry.spec.clone())
            .collect()
    }

    pub fn validate_plugin_publication(
        &self,
        owner: &str,
        contracts: &[RuntimeContractSpec],
    ) -> Result<(), String> {
        let owner = owner.trim();
        if owner.is_empty() {
            return Err("runtime contract publication owner is empty".to_owned());
        }
        let mut incoming_keys = BTreeSet::new();
        let mut incoming_ids = BTreeSet::new();
        for spec in contracts {
            validate_spec(spec)?;
            if spec.owner != owner {
                return Err(format!(
                    "runtime contract owner mismatch key='{}' declaration_owner='{}' provider_owner='{}'",
                    spec.key, spec.owner, owner
                ));
            }
            if !incoming_keys.insert(spec.key.as_str()) {
                return Err(format!("runtime contract declared twice: '{}'", spec.key));
            }
            if let Some(id) = spec.advertised_id.as_deref() {
                if !incoming_ids.insert(id) {
                    return Err(format!(
                        "runtime contract advertised id declared twice: '{id}'"
                    ));
                }
            }
            if let Some(existing) = self.by_key.get(&spec.key) {
                match existing.authority {
                    RuntimeContractAuthority::Engine => {
                        return Err(format!(
                            "plugin contract '{}' cannot override normative engine contract owned by '{}'",
                            spec.key, existing.spec.owner
                        ));
                    }
                    RuntimeContractAuthority::Plugin if existing.spec.owner != owner => {
                        return Err(format!(
                            "plugin contract key collision key='{}' existing_owner='{}' contender='{}'",
                            spec.key, existing.spec.owner, owner
                        ));
                    }
                    RuntimeContractAuthority::Plugin => {}
                }
            }
            if let Some(id) = spec.advertised_id.as_deref() {
                if let Some(existing_key) = self.by_advertised_id.get(id) {
                    let existing = self
                        .by_key
                        .get(existing_key)
                        .expect("catalog index coherent");
                    let replaceable_same_owner = existing.authority
                        == RuntimeContractAuthority::Plugin
                        && existing.spec.owner == owner;
                    if !replaceable_same_owner && existing.spec.key != spec.key {
                        return Err(format!(
                            "runtime contract advertised id collision id='{}' existing_key='{}' contender='{}'",
                            id, existing.spec.key, spec.key
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn replace_plugin_contracts(
        &mut self,
        owner: &str,
        contracts: Vec<RuntimeContractSpec>,
    ) -> Result<(), String> {
        self.validate_plugin_publication(owner, &contracts)?;
        self.replace_plugin_contracts_after_validation(owner, contracts);
        Ok(())
    }

    /// Commits a publication that was validated against this same exclusively-held
    /// catalog state. Provider transactions use this after entering their odd topology
    /// generation so no fallible operation can strand the generation in-progress.
    pub fn replace_plugin_contracts_after_validation(
        &mut self,
        owner: &str,
        contracts: Vec<RuntimeContractSpec>,
    ) {
        debug_assert!(self.validate_plugin_publication(owner, &contracts).is_ok());
        self.remove_plugin_contracts(owner);
        for spec in contracts {
            self.insert_unchecked(RuntimeContractEntry {
                authority: RuntimeContractAuthority::Plugin,
                spec,
            });
        }
    }

    pub fn remove_plugin_contracts(&mut self, owner: &str) -> usize {
        let keys = self
            .by_key
            .iter()
            .filter(|(_, entry)| {
                entry.authority == RuntimeContractAuthority::Plugin && entry.spec.owner == owner
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in &keys {
            if let Some(entry) = self.by_key.remove(key) {
                if let Some(id) = entry.spec.advertised_id {
                    self.by_advertised_id.remove(&id);
                }
            }
        }
        keys.len()
    }

    fn insert_unchecked(&mut self, entry: RuntimeContractEntry) {
        if let Some(id) = entry.spec.advertised_id.as_ref() {
            self.by_advertised_id
                .insert(id.clone(), entry.spec.key.clone());
        }
        self.by_key.insert(entry.spec.key.clone(), entry);
    }
}

pub fn contracts_from_plugin_descriptor(
    descriptor: &newengine_plugin_api::PluginDescriptor,
) -> Result<Vec<RuntimeContractSpec>, String> {
    let owner = descriptor.id.as_str();
    let mut out = Vec::new();
    for capability in descriptor.capabilities.iter() {
        match newengine_plugin_api::runtime_contract_declaration(capability) {
            Ok(Some(declaration)) => {
                out.push(RuntimeContractSpec::from_plugin_declaration(
                    owner,
                    declaration,
                ));
            }
            Ok(None) => {}
            Err(error) => return Err(error),
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(out)
}

fn validate_spec(spec: &RuntimeContractSpec) -> Result<(), String> {
    if spec.key.trim().is_empty() {
        return Err("runtime contract key is empty".to_owned());
    }
    if spec.owner.trim().is_empty() {
        return Err(format!("runtime contract '{}' has empty owner", spec.key));
    }
    if spec.version.major == 0 {
        return Err(format!(
            "runtime contract '{}' has zero major version",
            spec.key
        ));
    }
    if let Some(id) = spec.advertised_id.as_deref() {
        if id.trim().is_empty() {
            return Err(format!(
                "runtime contract '{}' has empty advertised id",
                spec.key
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin_spec(owner: &str, key: &str, id: &str) -> RuntimeContractSpec {
        RuntimeContractSpec {
            key: key.to_owned(),
            kind: ContractKind::Protocol,
            version: ContractVersion::major(1),
            compatibility: ContractCompatibility::SameMajor,
            owner: owner.to_owned(),
            advertised_id: Some(id.to_owned()),
        }
    }

    #[test]
    fn engine_contracts_are_seeded_and_immutable_to_plugins() {
        let mut catalog = RuntimeContractCatalog::default();
        assert_eq!(
            catalog.contract("render.provider.abi").unwrap().authority,
            RuntimeContractAuthority::Engine
        );
        let spec = plugin_spec("acme", "render.provider.abi", "acme.override");
        assert!(catalog
            .replace_plugin_contracts("acme", vec![spec])
            .is_err());
    }

    #[test]
    fn plugin_contracts_can_be_replaced_by_same_owner() {
        let mut catalog = RuntimeContractCatalog::default();
        catalog
            .replace_plugin_contracts("acme", vec![plugin_spec("acme", "acme.foo", "acme.foo.v1")])
            .unwrap();
        let mut replacement = plugin_spec("acme", "acme.foo", "acme.foo.v2");
        replacement.version = ContractVersion::major(2);
        catalog
            .replace_plugin_contracts("acme", vec![replacement.clone()])
            .unwrap();
        assert_eq!(catalog.contract("acme.foo").unwrap().spec, replacement);
        assert!(catalog.contract_by_advertised_id("acme.foo.v1").is_none());
    }

    #[test]
    fn plugin_contract_key_and_advertised_id_collisions_are_rejected() {
        let mut catalog = RuntimeContractCatalog::default();
        catalog
            .replace_plugin_contracts("acme", vec![plugin_spec("acme", "acme.foo", "shared.v1")])
            .unwrap();
        assert!(catalog
            .replace_plugin_contracts("other", vec![plugin_spec("other", "acme.foo", "other.v1")],)
            .is_err());
        assert!(catalog
            .replace_plugin_contracts(
                "other",
                vec![plugin_spec("other", "other.foo", "shared.v1")],
            )
            .is_err());
    }

    #[test]
    fn references_resolve_by_key_or_advertised_id_and_validate_versions() {
        let catalog = RuntimeContractCatalog::default();
        let by_key = catalog
            .resolve_contract_reference("asset.decode.protocol")
            .expect("asset decode contract by key");
        let by_id = catalog
            .resolve_contract_reference("asset.decode_v1")
            .expect("asset decode contract by advertised id");
        assert_eq!(by_key.spec.key, by_id.spec.key);
        assert_eq!(
            catalog.canonical_contract_key("asset.decode_v1"),
            Some("asset.decode.protocol")
        );
        assert!(catalog.validate_offered_major("asset.decode_v1", 1).is_ok());
        assert!(catalog
            .validate_offered_major("asset.decode_v1", 2)
            .is_err());
        assert!(catalog
            .validate_required_major_range("asset.decode.protocol", 1, Some(1))
            .is_ok());
        assert!(catalog
            .validate_required_major_range("asset.decode.protocol", 2, Some(3))
            .is_err());
        assert!(catalog
            .validate_required_major_range("unknown.contract", 1, None)
            .is_err());
    }

    #[test]
    fn descriptor_contract_declarations_are_owned_by_plugin_id() {
        let descriptor = newengine_plugin_api::PluginDescriptor::builder(
            "acme.plugin",
            "Acme",
            "1.0.0",
            newengine_plugin_api::PluginKind::Runtime,
        )
        .push(
            newengine_plugin_api::RuntimeContractDeclaration::new(
                "acme.streaming.protocol",
                ContractKind::Protocol,
                ContractVersion::major(1),
                ContractCompatibility::SameMajor,
            )
            .advertised_id("acme.streaming.v1")
            .into_capability(),
        )
        .build();
        let contracts = contracts_from_plugin_descriptor(&descriptor).unwrap();
        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0].owner, "acme.plugin");
        assert_eq!(contracts[0].key, "acme.streaming.protocol");
    }
}
