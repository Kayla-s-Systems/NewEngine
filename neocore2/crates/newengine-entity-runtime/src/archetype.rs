#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock, RwLock};

use newengine_ecs::{EntityId, World};
use newengine_entity_api::{
    EntityArchetypeDefinition, EntityArchetypeDescriptor, EntitySpawnRequest,
};

/// ECS composition factory selected by stable authored archetype id.
pub trait EntityArchetypeFactory: Send + Sync {
    fn id(&self) -> &'static str;

    fn owner(&self) -> &'static str {
        "engine.entity"
    }

    fn description(&self) -> &'static str {
        "Entity archetype factory"
    }

    fn spawn(
        &self,
        world: &mut World,
        request: &EntitySpawnRequest,
        instance_index: usize,
    ) -> Result<EntityId, String>;
}

#[derive(Default)]
pub struct EntityArchetypeRegistry {
    factories: RwLock<BTreeMap<String, Arc<dyn EntityArchetypeFactory>>>,
    definitions: RwLock<BTreeMap<String, EntityArchetypeDefinition>>,
}

impl EntityArchetypeRegistry {
    pub fn with_builtins() -> Self {
        let registry = Self::default();
        registry
            .register(Arc::new(EmptyEntityArchetype))
            .expect("builtin entity.empty archetype must register");
        registry
    }

    pub fn register(&self, factory: Arc<dyn EntityArchetypeFactory>) -> Result<(), String> {
        let id = normalize_archetype_id(factory.id())?;
        if self
            .definitions
            .read()
            .map_err(|_| "entity archetype definition registry lock poisoned".to_owned())?
            .contains_key(&id)
        {
            return Err(format!(
                "entity archetype id '{id}' is already owned by an authored definition"
            ));
        }
        let mut factories = self
            .factories
            .write()
            .map_err(|_| "entity archetype registry lock poisoned".to_owned())?;
        if factories.contains_key(&id) {
            return Err(format!("entity archetype factory already registered: {id}"));
        }
        factories.insert(id, factory);
        Ok(())
    }

    pub fn register_definition(
        &self,
        mut definition: EntityArchetypeDefinition,
    ) -> Result<(), String> {
        let id = normalize_archetype_id(&definition.id)?;
        let base = normalize_archetype_id(&definition.base_archetype)?;
        if id == base {
            return Err(format!(
                "entity archetype definition '{id}' cannot inherit itself"
            ));
        }
        if self
            .factories
            .read()
            .map_err(|_| "entity archetype registry lock poisoned".to_owned())?
            .contains_key(&id)
        {
            return Err(format!(
                "entity archetype id '{id}' is already owned by a concrete factory"
            ));
        }
        definition.id = id.clone();
        definition.base_archetype = base;
        definition.tags = normalize_tags(definition.tags);
        definition.owner = definition.owner.trim().to_owned();
        definition.description = definition.description.trim().to_owned();
        definition.definition_ref = definition
            .definition_ref
            .map(|value| value.trim().replace('\\', "/"))
            .filter(|value| !value.is_empty());
        definition.default_owner = definition
            .default_owner
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if !definition.default_properties.is_object() && !definition.default_properties.is_null() {
            return Err(format!(
                "entity archetype definition '{}' default_properties must be an object or null",
                definition.id
            ));
        }
        let mut definitions = self
            .definitions
            .write()
            .map_err(|_| "entity archetype definition registry lock poisoned".to_owned())?;
        if definitions.contains_key(&id) {
            return Err(format!(
                "entity archetype definition already registered: {id}"
            ));
        }
        definitions.insert(id, definition);
        Ok(())
    }

    pub fn unregister_definition(&self, archetype_id: &str) -> bool {
        let Ok(id) = normalize_archetype_id(archetype_id) else {
            return false;
        };
        self.definitions
            .write()
            .map(|mut definitions| definitions.remove(&id).is_some())
            .unwrap_or(false)
    }

    pub fn unregister(&self, archetype_id: &str) -> bool {
        let Ok(id) = normalize_archetype_id(archetype_id) else {
            return false;
        };
        self.factories
            .write()
            .map(|mut factories| factories.remove(&id).is_some())
            .unwrap_or(false)
    }

    pub fn spawn(
        &self,
        world: &mut World,
        request: &EntitySpawnRequest,
        instance_index: usize,
    ) -> Result<EntityId, String> {
        let mut visited = Vec::new();
        self.spawn_resolved(world, request.clone(), instance_index, &mut visited)
    }

    fn spawn_resolved(
        &self,
        world: &mut World,
        request: EntitySpawnRequest,
        instance_index: usize,
        visited: &mut Vec<String>,
    ) -> Result<EntityId, String> {
        const MAX_DEFINITION_DEPTH: usize = 32;
        let id = normalize_archetype_id(&request.archetype)?;
        if visited.len() >= MAX_DEFINITION_DEPTH {
            return Err(format!(
                "entity archetype inheritance exceeded {MAX_DEFINITION_DEPTH} levels: {}",
                visited.join(" -> ")
            ));
        }
        if visited.iter().any(|seen| seen == &id) {
            visited.push(id.clone());
            return Err(format!(
                "entity archetype inheritance cycle: {}",
                visited.join(" -> ")
            ));
        }

        if let Some(factory) = self
            .factories
            .read()
            .map_err(|_| "entity archetype registry lock poisoned".to_owned())?
            .get(&id)
            .cloned()
        {
            return factory.spawn(world, &request, instance_index);
        }

        let definition = self
            .definitions
            .read()
            .map_err(|_| "entity archetype definition registry lock poisoned".to_owned())?
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("unknown entity archetype '{id}'"))?;

        visited.push(id);
        let mut inherited = request;
        inherited.archetype = definition.base_archetype.clone();
        inherited.properties =
            merge_json_objects(definition.default_properties.clone(), inherited.properties);
        inherited.tags = merge_tags(definition.tags, inherited.tags);
        if inherited
            .owner
            .as_deref()
            .is_none_or(|owner| owner.trim().is_empty())
        {
            inherited.owner = definition.default_owner;
        }
        let result = self.spawn_resolved(world, inherited, instance_index, visited);
        visited.pop();
        result
    }

    pub fn descriptors(&self) -> Vec<EntityArchetypeDescriptor> {
        let Ok(factories) = self.factories.read() else {
            return Vec::new();
        };
        let mut descriptors = factories
            .values()
            .map(|factory| EntityArchetypeDescriptor {
                id: factory.id().to_owned(),
                owner: factory.owner().to_owned(),
                description: factory.description().to_owned(),
                source_kind: "factory".to_owned(),
                base_archetype: None,
                definition_ref: None,
            })
            .collect::<Vec<_>>();
        drop(factories);
        if let Ok(definitions) = self.definitions.read() {
            descriptors.extend(
                definitions
                    .values()
                    .map(|definition| EntityArchetypeDescriptor {
                        id: definition.id.clone(),
                        owner: definition.owner.clone(),
                        description: definition.description.clone(),
                        source_kind: "authored_definition".to_owned(),
                        base_archetype: Some(definition.base_archetype.clone()),
                        definition_ref: definition.definition_ref.clone(),
                    }),
            );
        }
        descriptors.sort_by(|a, b| a.id.cmp(&b.id));
        descriptors
    }
}

pub fn default_entity_archetype_registry() -> Arc<EntityArchetypeRegistry> {
    static REGISTRY: OnceLock<Arc<EntityArchetypeRegistry>> = OnceLock::new();
    Arc::clone(REGISTRY.get_or_init(|| Arc::new(EntityArchetypeRegistry::with_builtins())))
}

pub fn register_entity_archetype(factory: Arc<dyn EntityArchetypeFactory>) -> Result<(), String> {
    default_entity_archetype_registry().register(factory)
}

#[inline]
pub fn register_entity_archetype_definition(
    definition: EntityArchetypeDefinition,
) -> Result<(), String> {
    default_entity_archetype_registry().register_definition(definition)
}

#[derive(Debug, Clone)]
pub(crate) struct EntityRuntimeMetadata {
    pub archetype: String,
    pub tags: Vec<String>,
    pub owner: Option<String>,
}

struct EmptyEntityArchetype;

impl EntityArchetypeFactory for EmptyEntityArchetype {
    fn id(&self) -> &'static str {
        "entity.empty"
    }

    fn owner(&self) -> &'static str {
        "newengine-entity-runtime"
    }

    fn description(&self) -> &'static str {
        "Minimal empty ECS entity for tools/tests and explicit low-level construction"
    }

    fn spawn(
        &self,
        world: &mut World,
        _request: &EntitySpawnRequest,
        _instance_index: usize,
    ) -> Result<EntityId, String> {
        Ok(world.spawn())
    }
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if !tag.is_empty() && !out.iter().any(|existing| existing == tag) {
            out.push(tag.to_owned());
        }
    }
    out
}

fn merge_tags(base: Vec<String>, overlay: Vec<String>) -> Vec<String> {
    normalize_tags(base.into_iter().chain(overlay).collect())
}

fn merge_json_objects(
    mut base: serde_json::Value,
    overlay: serde_json::Value,
) -> serde_json::Value {
    if base.is_null() {
        base = serde_json::Value::Object(serde_json::Map::new());
    }
    if overlay.is_null() {
        return base;
    }
    match (&mut base, overlay) {
        (serde_json::Value::Object(base), serde_json::Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(existing) if existing.is_object() && value.is_object() => {
                        *existing = merge_json_objects(existing.take(), value);
                    }
                    _ => {
                        base.insert(key, value);
                    }
                }
            }
            serde_json::Value::Object(base.clone())
        }
        (_, overlay) => overlay,
    }
}

fn normalize_archetype_id(value: &str) -> Result<String, String> {
    let id = value.trim().to_ascii_lowercase();
    if id.is_empty() || id.contains(char::is_whitespace) || id.contains('/') || id.contains('\\') {
        return Err(format!("invalid entity archetype id '{value}'"));
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn builtin_registry_exposes_explicit_empty_archetype() {
        let registry = EntityArchetypeRegistry::with_builtins();
        assert!(registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.id == "entity.empty"));
    }

    struct CaptureFactory {
        captured: Arc<Mutex<Option<EntitySpawnRequest>>>,
    }

    impl EntityArchetypeFactory for CaptureFactory {
        fn id(&self) -> &'static str {
            "test.capture"
        }

        fn spawn(
            &self,
            world: &mut World,
            request: &EntitySpawnRequest,
            _instance_index: usize,
        ) -> Result<EntityId, String> {
            *self.captured.lock().expect("capture lock") = Some(request.clone());
            Ok(world.spawn())
        }
    }

    #[test]
    fn authored_definition_inherits_factory_and_merges_properties() {
        let registry = EntityArchetypeRegistry::with_builtins();
        let captured = Arc::new(Mutex::new(None));
        registry
            .register(Arc::new(CaptureFactory {
                captured: Arc::clone(&captured),
            }))
            .expect("capture factory");
        registry
            .register_definition(EntityArchetypeDefinition {
                id: "npc.guard".to_owned(),
                base_archetype: "test.capture".to_owned(),
                owner: "game.test".to_owned(),
                description: "authored guard".to_owned(),
                definition_ref: Some("game:/definitions/npc.guard.ytyp".to_owned()),
                default_properties: serde_json::json!({
                    "health": 100,
                    "combat": { "accuracy": 0.5, "burst": 3 }
                }),
                tags: vec!["npc".to_owned(), "guard".to_owned()],
                default_owner: Some("gameplay".to_owned()),
            })
            .expect("definition");

        let mut world = World::new();
        registry
            .spawn(
                &mut world,
                &EntitySpawnRequest {
                    archetype: "npc.guard".to_owned(),
                    properties: serde_json::json!({
                        "health": 140,
                        "combat": { "accuracy": 0.8 }
                    }),
                    tags: vec!["elite".to_owned()],
                    ..EntitySpawnRequest::default()
                },
                0,
            )
            .expect("spawn through definition");

        let request = captured
            .lock()
            .expect("capture lock")
            .clone()
            .expect("captured request");
        assert_eq!(request.archetype, "test.capture");
        assert_eq!(request.properties["health"], 140);
        assert_eq!(request.properties["combat"]["accuracy"], 0.8);
        assert_eq!(request.properties["combat"]["burst"], 3);
        assert_eq!(request.tags, vec!["npc", "guard", "elite"]);
        assert_eq!(request.owner.as_deref(), Some("gameplay"));

        let descriptor = registry
            .descriptors()
            .into_iter()
            .find(|it| it.id == "npc.guard")
            .expect("authored descriptor");
        assert_eq!(descriptor.source_kind, "authored_definition");
        assert_eq!(descriptor.base_archetype.as_deref(), Some("test.capture"));
    }

    #[test]
    fn authored_definition_cycle_is_rejected_at_spawn() {
        let registry = EntityArchetypeRegistry::with_builtins();
        registry
            .register_definition(EntityArchetypeDefinition {
                id: "test.a".to_owned(),
                base_archetype: "test.b".to_owned(),
                ..EntityArchetypeDefinition::default()
            })
            .unwrap();
        registry
            .register_definition(EntityArchetypeDefinition {
                id: "test.b".to_owned(),
                base_archetype: "test.a".to_owned(),
                ..EntityArchetypeDefinition::default()
            })
            .unwrap();
        let error = registry
            .spawn(
                &mut World::new(),
                &EntitySpawnRequest {
                    archetype: "test.a".to_owned(),
                    ..EntitySpawnRequest::default()
                },
                0,
            )
            .expect_err("cycle must fail");
        assert!(error.contains("inheritance cycle"));
    }
}
