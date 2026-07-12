use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TagId(pub String);

impl TagId {
    #[inline]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TagDomain {
    Gameplay,
    State,
    Faction,
    Item,
    Weapon,
    Mission,
    Animation,
    Navigation,
    Debug,
    Custom(String),
}

impl Default for TagDomain {
    #[inline]
    fn default() -> Self {
        Self::Gameplay
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagDescriptorV1 {
    pub tag: TagId,
    #[serde(default)]
    pub domain: TagDomain,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub parent: Option<TagId>,
    #[serde(default)]
    pub aliases: Vec<TagId>,
}
