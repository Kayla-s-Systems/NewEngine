use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct InventoryLoadoutEntry {
    pub item: ItemId,
    pub quantity: u32,
    pub equip: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InventoryLoadout {
    pub id: ItemId,
    pub name: String,
    pub clear_existing: bool,
    pub entries: Vec<InventoryLoadoutEntry>,
}

impl InventoryLoadout {
    pub fn new(name: impl AsRef<str>) -> Result<Self, String> {
        let name = normalize_item_name(name.as_ref())
            .ok_or_else(|| "loadout name must contain at least one valid character".to_owned())?;
        Ok(Self {
            id: ItemId(stable_hash64(name.as_bytes())),
            name,
            clear_existing: true,
            entries: Vec::new(),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InventoryLoadoutCatalog {
    loadouts: BTreeMap<ItemId, InventoryLoadout>,
}

impl InventoryLoadoutCatalog {
    pub fn register(&mut self, loadout: InventoryLoadout) -> Result<ItemId, String> {
        if let Some(existing) = self.loadouts.get(&loadout.id) {
            if existing.name != loadout.name {
                return Err(format!(
                    "loadout id collision: '{}' and '{}'",
                    existing.name, loadout.name
                ));
            }
        }
        let id = loadout.id;
        self.loadouts.insert(id, loadout);
        Ok(id)
    }

    #[inline]
    pub fn get(&self, id: ItemId) -> Option<&InventoryLoadout> {
        self.loadouts.get(&id)
    }

    pub fn loadouts(&self) -> impl Iterator<Item = &InventoryLoadout> {
        self.loadouts.values()
    }

    pub fn fps_defaults() -> Self {
        let mut loadout =
            InventoryLoadout::new(DEFAULT_FPS_LOADOUT_NAME).expect("valid built-in loadout name");
        loadout.entries = vec![
            InventoryLoadoutEntry {
                item: default_rifle_item_id(),
                quantity: 1,
                equip: true,
            },
            InventoryLoadoutEntry {
                item: default_rifle_ammo_id(),
                quantity: 90,
                equip: false,
            },
            InventoryLoadoutEntry {
                item: default_medkit_item_id(),
                quantity: 2,
                equip: false,
            },
        ];
        let mut catalog = Self::default();
        catalog.register(loadout).expect("unique built-in loadout");
        catalog
    }
}
