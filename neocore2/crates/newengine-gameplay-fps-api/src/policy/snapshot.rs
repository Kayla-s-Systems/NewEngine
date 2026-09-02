#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsGameplayPolicySnapshot {
    pub schema: String,
    pub version: u32,
    /// Authored item/weapon/loadout package. The FPS content compiler owns the
    /// conversion from this DTO into generic engine inventory components.
    pub content: serde_json::Value,
    pub required_content: FpsRequiredContentPolicy,
    /// Project-authored playable character catalog. The FPS runtime only applies these descriptors.
    pub characters: Vec<FpsPlayableCharacterPolicy>,
    pub player: FpsPlayerPolicy,
    pub combat: FpsCombatPolicy,
    pub mission: FpsMissionPolicy,
    /// Generic project-owned event routing.
    pub event_subscriptions: Vec<FpsProjectEventSubscription>,
    pub callbacks: FpsCallbackExports,
}

impl Default for FpsGameplayPolicySnapshot {
    fn default() -> Self {
        Self {
            schema: FPS_GAMEPLAY_POLICY_SCHEMA.to_owned(),
            version: FPS_GAMEPLAY_POLICY_VERSION,
            content: serde_json::json!({
                "schema": "newengine.items.package.v1",
                "version": 1,
                "items": [],
                "loadouts": []
            }),
            required_content: FpsRequiredContentPolicy::default(),
            characters: Vec::new(),
            player: FpsPlayerPolicy::default(),
            combat: FpsCombatPolicy::default(),
            mission: FpsMissionPolicy::default(),
            event_subscriptions: Vec::new(),
            callbacks: FpsCallbackExports::default(),
        }
    }
}

impl FpsGameplayPolicySnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FPS_GAMEPLAY_POLICY_SCHEMA {
            return Err(format!(
                "FPS gameplay policy schema mismatch: expected '{}' got '{}'",
                FPS_GAMEPLAY_POLICY_SCHEMA, self.schema
            ));
        }
        if self.version != FPS_GAMEPLAY_POLICY_VERSION {
            return Err(format!(
                "FPS gameplay policy version mismatch: expected {} got {}",
                FPS_GAMEPLAY_POLICY_VERSION, self.version
            ));
        }
        if !self.content.is_object() {
            return Err("FPS gameplay policy content must be an item-package object".to_owned());
        }
        self.required_content.validate()?;
        let mut character_ids = BTreeSet::new();
        let mut character_aliases = BTreeSet::new();
        for character in &self.characters {
            character.validate()?;
            let id_key = character.id.trim().to_ascii_lowercase();
            if !character_ids.insert(id_key.clone()) {
                return Err(format!(
                    "duplicate FPS playable character id '{}'",
                    character.id
                ));
            }
            for alias in &character.aliases {
                let alias_key = alias.trim().to_ascii_lowercase();
                if alias_key == id_key || !character_aliases.insert(alias_key.clone()) {
                    return Err(format!(
                        "duplicate/ambiguous FPS playable character alias '{}'",
                        alias
                    ));
                }
            }
        }
        self.player.validate()?;
        self.combat.validate()?;
        self.mission.validate()?;
        let mut subscriptions = BTreeSet::new();
        for subscription in &self.event_subscriptions {
            subscription.validate()?;
            let key = (
                subscription.event.trim().to_ascii_lowercase(),
                subscription.operation.trim().to_ascii_lowercase(),
            );
            if !subscriptions.insert(key) {
                return Err(format!(
                    "duplicate FPS project event subscription event='{}' operation='{}'",
                    subscription.event, subscription.operation
                ));
            }
        }
        self.callbacks.validate()?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsRequiredContentPolicy {
    pub default_loadout: String,
    pub primary_weapon: String,
    pub primary_ammo: String,
    pub medkit: String,
}

impl Default for FpsRequiredContentPolicy {
    fn default() -> Self {
        Self {
            default_loadout: String::new(),
            primary_weapon: String::new(),
            primary_ammo: String::new(),
            medkit: String::new(),
        }
    }
}

impl FpsRequiredContentPolicy {
    fn validate(&self) -> Result<(), String> {
        for (label, value) in [
            ("default_loadout", &self.default_loadout),
            ("primary_weapon", &self.primary_weapon),
            ("primary_ammo", &self.primary_ammo),
            ("medkit", &self.medkit),
        ] {
            if value.trim().is_empty() {
                return Err(format!(
                    "FPS required content id '{label}' must not be empty"
                ));
            }
        }
        Ok(())
    }
}
