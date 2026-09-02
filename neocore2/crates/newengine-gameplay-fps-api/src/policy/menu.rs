#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsCharacterMenuPolicySnapshot {
    pub schema: String,
    pub version: u32,
    /// Semantic action consumed by the menu. Physical key mapping remains input-profile owned.
    pub toggle_action: String,
    pub title: String,
    /// Shared fallback catalog. A non-empty project `characters` catalog remains authoritative.
    pub characters: Vec<FpsPlayableCharacterPolicy>,
}

impl Default for FpsCharacterMenuPolicySnapshot {
    fn default() -> Self {
        Self {
            schema: FPS_CHARACTER_MENU_POLICY_SCHEMA.to_owned(),
            version: FPS_CHARACTER_MENU_POLICY_VERSION,
            toggle_action: crate::action::CHARACTER_SELECT_TOGGLE.to_owned(),
            title: "Character".to_owned(),
            characters: Vec::new(),
        }
    }
}

impl FpsCharacterMenuPolicySnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FPS_CHARACTER_MENU_POLICY_SCHEMA {
            return Err(format!(
                "FPS character-menu policy schema mismatch: expected '{}' got '{}'",
                FPS_CHARACTER_MENU_POLICY_SCHEMA, self.schema
            ));
        }
        if self.version != FPS_CHARACTER_MENU_POLICY_VERSION {
            return Err(format!(
                "FPS character-menu policy version mismatch: expected {} got {}",
                FPS_CHARACTER_MENU_POLICY_VERSION, self.version
            ));
        }
        validate_action_id("character_menu.toggle_action", &self.toggle_action)?;
        if self.title.trim().is_empty() || self.title.len() > 96 {
            return Err("FPS character-menu title must contain 1..=96 bytes".to_owned());
        }
        let mut ids = BTreeSet::new();
        let mut aliases = BTreeSet::new();
        for character in &self.characters {
            character.validate_menu_entry()?;
            let id = character.id.trim().to_ascii_lowercase();
            if !ids.insert(id.clone()) {
                return Err(format!(
                    "duplicate Shared FPS character-menu character id '{}'",
                    character.id
                ));
            }
            for alias in &character.aliases {
                let alias = alias.trim().to_ascii_lowercase();
                if alias == id || !aliases.insert(alias.clone()) {
                    return Err(format!(
                        "duplicate/ambiguous Shared FPS character-menu alias '{}'",
                        alias
                    ));
                }
            }
        }
        Ok(())
    }
}

pub trait FpsCharacterMenuPolicyProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn load_snapshot(&self) -> Result<Arc<FpsCharacterMenuPolicySnapshot>, String>;
}

fn validate_action_id(label: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 128 {
        return Err(format!("{label} must contain 1..=128 bytes"));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':'))
    {
        return Err(format!(
            "{label} contains unsupported characters: '{value}'"
        ));
    }
    Ok(())
}
