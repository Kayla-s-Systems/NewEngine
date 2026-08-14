use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use newengine_replication_api::{
    ReplicatedComponentDescriptor, ReplicatedEntityProfile, ReplicatedMessageDescriptor,
    ReplicationDefinitionBundleV1, ReplicationRegistrySnapshot, REPLICATION_DESCRIPTOR_CONTRACT,
};
use parking_lot::RwLock;

#[derive(Clone, Default)]
pub struct ReplicationDescriptorRegistry {
    inner: Arc<RwLock<State>>,
}

#[derive(Default)]
struct State {
    generation: u64,
    components: BTreeMap<String, ReplicatedComponentDescriptor>,
    profiles: BTreeMap<String, ReplicatedEntityProfile>,
    messages: BTreeMap<String, ReplicatedMessageDescriptor>,
}

impl ReplicationDescriptorRegistry {
    pub fn register_component(
        &self,
        mut descriptor: ReplicatedComponentDescriptor,
    ) -> Result<(), String> {
        descriptor.component_id = descriptor.component_id.trim().to_owned();
        descriptor.owner = descriptor.owner.trim().to_owned();
        descriptor.validate().map_err(|errors| errors.join("; "))?;
        let mut state = self.inner.write();
        if state.components.contains_key(&descriptor.component_id) {
            return Err(format!(
                "replicated component already registered: {}",
                descriptor.component_id
            ));
        }
        state
            .components
            .insert(descriptor.component_id.clone(), descriptor);
        state.generation = state.generation.wrapping_add(1);
        Ok(())
    }

    pub fn register_entity_profile(
        &self,
        mut profile: ReplicatedEntityProfile,
    ) -> Result<(), String> {
        profile.id = profile.id.trim().to_owned();
        if profile.id.is_empty() || profile.version == 0 {
            return Err(
                "replicated entity profile requires non-empty id and version >= 1".to_owned(),
            );
        }
        let state = self.inner.read();
        let missing = profile
            .components
            .iter()
            .filter(|id| !state.components.contains_key(id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        drop(state);
        if !missing.is_empty() {
            return Err(format!(
                "replicated entity profile '{}' references missing component(s): [{}]",
                profile.id,
                missing.join(", ")
            ));
        }
        profile.components.sort();
        profile.components.dedup();
        let mut state = self.inner.write();
        if state.profiles.contains_key(&profile.id) {
            return Err(format!(
                "replicated entity profile already registered: {}",
                profile.id
            ));
        }
        state.profiles.insert(profile.id.clone(), profile);
        state.generation = state.generation.wrapping_add(1);
        Ok(())
    }

    pub fn register_bundle(&self, mut bundle: ReplicationDefinitionBundleV1) -> Result<(), String> {
        bundle.validate_header()?;
        let bundle_owner = bundle.owner.trim().to_owned();
        for component in &mut bundle.components {
            if component.owner.trim().is_empty() {
                component.owner = bundle_owner.clone();
            }
            self.register_component(component.clone())?;
        }
        for profile in &mut bundle.entity_profiles {
            if profile.owner.trim().is_empty() {
                profile.owner = bundle_owner.clone();
            }
            self.register_entity_profile(profile.clone())?;
        }
        for message in bundle.messages {
            self.register_message(message)?;
        }
        Ok(())
    }

    pub fn register_message(
        &self,
        mut descriptor: ReplicatedMessageDescriptor,
    ) -> Result<(), String> {
        descriptor.message_id = descriptor.message_id.trim().to_owned();
        if descriptor.message_id.is_empty()
            || descriptor.version == 0
            || descriptor.max_rate_hz == 0
        {
            return Err(
                "replicated message requires non-empty id, version >= 1 and max_rate_hz > 0"
                    .to_owned(),
            );
        }
        let mut state = self.inner.write();
        if state.messages.contains_key(&descriptor.message_id) {
            return Err(format!(
                "replicated message already registered: {}",
                descriptor.message_id
            ));
        }
        state
            .messages
            .insert(descriptor.message_id.clone(), descriptor);
        state.generation = state.generation.wrapping_add(1);
        Ok(())
    }

    pub fn snapshot(&self) -> ReplicationRegistrySnapshot {
        let state = self.inner.read();
        ReplicationRegistrySnapshot {
            contract: REPLICATION_DESCRIPTOR_CONTRACT.to_owned(),
            generation: state.generation,
            components: state.components.values().cloned().collect(),
            entity_profiles: state.profiles.values().cloned().collect(),
            messages: state.messages.values().cloned().collect(),
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ReplicationDefinitionLoadReportV1 {
    pub schema: String,
    pub files: Vec<String>,
    pub components: usize,
    pub entity_profiles: usize,
    pub messages: usize,
    pub warnings: Vec<String>,
}

pub fn load_replication_definitions_from_roots(
    project_root: &Path,
    definition_roots: &[PathBuf],
    registry: &ReplicationDescriptorRegistry,
) -> Result<ReplicationDefinitionLoadReportV1, String> {
    let mut files = Vec::new();
    for root in definition_roots {
        let root = if root.is_absolute() {
            root.clone()
        } else {
            project_root.join(root)
        };
        collect_replication_definition_files(&root, &mut files);
    }
    files.sort();
    files.dedup();

    let mut report = ReplicationDefinitionLoadReportV1 {
        schema: "newengine.replication.definition_load_report.v1".to_owned(),
        ..Default::default()
    };
    for path in files {
        let bytes = std::fs::read(&path).map_err(|error| {
            format!("read replication definition '{}': {error}", path.display())
        })?;
        let mut bundle: ReplicationDefinitionBundleV1 =
            serde_json::from_slice(&bytes).map_err(|error| {
                format!("parse replication definition '{}': {error}", path.display())
            })?;
        if bundle.owner.trim().is_empty() {
            let relative = path.strip_prefix(project_root).unwrap_or(path.as_path());
            bundle.owner = format!("project:{}", relative.to_string_lossy().replace('\\', "/"));
        }
        let component_count = bundle.components.len();
        let profile_count = bundle.entity_profiles.len();
        let message_count = bundle.messages.len();
        registry.register_bundle(bundle).map_err(|error| {
            format!(
                "register replication definition '{}': {error}",
                path.display()
            )
        })?;
        report.files.push(path.to_string_lossy().replace('\\', "/"));
        report.components += component_count;
        report.entity_profiles += profile_count;
        report.messages += message_count;
    }
    Ok(report)
}

fn collect_replication_definition_files(root: &Path, out: &mut Vec<PathBuf>) {
    if root.is_file() {
        if is_replication_definition_file(root) {
            out.push(root.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_replication_definition_files(&path, out);
        } else if is_replication_definition_file(&path) {
            out.push(path);
        }
    }
}

fn is_replication_definition_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase().ends_with(".replication.json"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_replication_api::{ReplicatedFieldDescriptor, ReplicationWireType};

    #[test]
    fn profile_cannot_reference_unregistered_component() {
        let registry = ReplicationDescriptorRegistry::default();
        assert!(registry
            .register_entity_profile(ReplicatedEntityProfile {
                id: "game.player".into(),
                version: 1,
                components: vec!["game.transform".into()],
                ..Default::default()
            })
            .is_err());
    }

    #[test]
    fn component_then_profile_builds_stable_snapshot() {
        let registry = ReplicationDescriptorRegistry::default();
        registry
            .register_component(ReplicatedComponentDescriptor {
                component_id: "game.transform".into(),
                fields: vec![ReplicatedFieldDescriptor {
                    field_id: 1,
                    name: "position".into(),
                    wire_type: ReplicationWireType::Vec3F32,
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        registry
            .register_entity_profile(ReplicatedEntityProfile {
                id: "game.player".into(),
                version: 1,
                components: vec!["game.transform".into()],
                ..Default::default()
            })
            .unwrap();
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.components.len(), 1);
        assert_eq!(snapshot.entity_profiles.len(), 1);
        assert_eq!(snapshot.generation, 2);
    }

    #[test]
    fn authored_bundle_loads_from_project_definition_root() {
        let root =
            std::env::temp_dir().join(format!("newengine-replication-defs-{}", std::process::id()));
        let definitions = root.join("definitions/network");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&definitions).unwrap();
        std::fs::write(
            definitions.join("player.replication.json"),
            r#"{
          "schema": "newengine.replication.definitions.v1",
          "components": [{
            "component_id": "game.transform",
            "fields": [{"field_id":1,"name":"position","wire_type":"vec3_f32"}]
          }],
          "entity_profiles": [{"id":"game.player","version":1,"components":["game.transform"]}],
          "messages": [{"message_id":"game.weapon.fired","version":1,"max_rate_hz":30}]
        }"#,
        )
        .unwrap();
        let registry = ReplicationDescriptorRegistry::default();
        let report = load_replication_definitions_from_roots(
            &root,
            &[PathBuf::from("definitions")],
            &registry,
        )
        .unwrap();
        assert_eq!(report.files.len(), 1);
        assert_eq!(report.components, 1);
        assert_eq!(registry.snapshot().entity_profiles.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}
