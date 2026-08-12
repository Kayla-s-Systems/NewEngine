use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemCatalog {
    definitions: BTreeMap<ItemId, ItemDefinition>,
    names: BTreeMap<String, ItemId>,
}

impl ItemCatalog {
    pub fn register(&mut self, definition: ItemDefinition) -> Result<ItemId, String> {
        if let Some(existing) = self.definitions.get(&definition.id) {
            if existing.name != definition.name {
                return Err(format!(
                    "item id collision: '{}' and '{}' resolve to {:016x}",
                    existing.name, definition.name, definition.id.0
                ));
            }
        }
        if let Some(existing_id) = self.names.get(&definition.name) {
            if *existing_id != definition.id {
                return Err(format!(
                    "item name '{}' already belongs to a different id",
                    definition.name
                ));
            }
        }
        let id = definition.id;
        self.names.insert(definition.name.clone(), id);
        self.definitions.insert(id, definition);
        Ok(id)
    }

    #[inline]
    pub fn get(&self, id: ItemId) -> Option<&ItemDefinition> {
        self.definitions.get(&id)
    }

    pub fn find(&self, name: &str) -> Option<&ItemDefinition> {
        let normalized = normalize_item_name(name)?;
        let id = self.names.get(&normalized)?;
        self.definitions.get(id)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &ItemDefinition> {
        self.definitions.values()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}
