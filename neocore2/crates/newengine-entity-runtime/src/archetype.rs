#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock, RwLock};

use newengine_ecs::{EntityId, World};
use newengine_entity_api::{EntityArchetypeDescriptor, EntitySpawnRequest};

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
        let mut factories = self
            .factories
            .write()
            .map_err(|_| "entity archetype registry lock poisoned".to_owned())?;
        factories.insert(id, factory);
        Ok(())
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
        let id = normalize_archetype_id(&request.archetype)?;
        let factory = self
            .factories
            .read()
            .map_err(|_| "entity archetype registry lock poisoned".to_owned())?
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("unknown entity archetype '{id}'"))?;
        factory.spawn(world, request, instance_index)
    }

    pub fn descriptors(&self) -> Vec<EntityArchetypeDescriptor> {
        let Ok(factories) = self.factories.read() else {
            return Vec::new();
        };
        factories
            .values()
            .map(|factory| EntityArchetypeDescriptor {
                id: factory.id().to_owned(),
                owner: factory.owner().to_owned(),
                description: factory.description().to_owned(),
            })
            .collect()
    }
}

pub fn default_entity_archetype_registry() -> Arc<EntityArchetypeRegistry> {
    static REGISTRY: OnceLock<Arc<EntityArchetypeRegistry>> = OnceLock::new();
    Arc::clone(REGISTRY.get_or_init(|| Arc::new(EntityArchetypeRegistry::with_builtins())))
}

pub fn register_entity_archetype(factory: Arc<dyn EntityArchetypeFactory>) -> Result<(), String> {
    default_entity_archetype_registry().register(factory)
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

    #[test]
    fn builtin_registry_exposes_explicit_empty_archetype() {
        let registry = EntityArchetypeRegistry::with_builtins();
        assert!(registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.id == "entity.empty"));
    }
}
