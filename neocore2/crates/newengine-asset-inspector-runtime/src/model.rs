use serde::{Deserialize, Serialize};

const PARENT_NAVIGATION_KIND: &str = "navigation.parent";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetInspectorMode {
    #[default]
    All,
    Assets,
    Folders,
}

impl AssetInspectorMode {
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Assets => "ASSETS",
            Self::Folders => "FOLDERS",
        }
    }

    #[inline]
    pub const fn accepts(self, is_directory: bool) -> bool {
        match self {
            Self::All => true,
            Self::Assets => !is_directory,
            Self::Folders => is_directory,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InspectorEntry {
    pub name: String,
    pub logical_path: String,
    pub kind: String,
    pub asset_kind: String,
    pub semantic_gateway: String,
    pub is_directory: bool,
    /// Provider descriptor exposes addressable child assets/entries.
    pub is_container: bool,
    pub container_entry: bool,
    pub byte_len: Option<u64>,
}

impl InspectorEntry {
    pub fn parent_navigation(logical_path: impl Into<String>) -> Self {
        Self {
            name: "../".to_owned(),
            logical_path: logical_path.into(),
            kind: PARENT_NAVIGATION_KIND.to_owned(),
            asset_kind: "parent directory".to_owned(),
            is_directory: true,
            ..Self::default()
        }
    }

    #[inline]
    pub fn is_parent_navigation(&self) -> bool {
        self.kind == PARENT_NAVIGATION_KIND
    }

    #[inline]
    pub fn marker(&self) -> &'static str {
        if self.is_parent_navigation() {
            "UP"
        } else if self.is_directory {
            "DIR"
        } else if self.container_entry {
            "ENT"
        } else if self.is_container {
            "CNT"
        } else {
            "AST"
        }
    }

    #[inline]
    pub fn detail(&self) -> String {
        if self.is_parent_navigation() {
            return "parent directory".to_owned();
        }
        let primary = if self.asset_kind.trim().is_empty() {
            self.kind.as_str()
        } else {
            self.asset_kind.as_str()
        };
        match self.byte_len {
            Some(bytes) => format!("{primary} | {bytes} bytes"),
            None => primary.to_owned(),
        }
    }
}
