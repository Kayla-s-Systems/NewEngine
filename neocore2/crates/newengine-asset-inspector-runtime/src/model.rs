use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetInspectorMode {
    #[default]
    All,
    Runtime,
    Source,
}

impl AssetInspectorMode {
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Runtime => "RUNTIME",
            Self::Source => "SOURCE",
        }
    }

    #[inline]
    pub const fn accepts(self, is_directory: bool, source: bool) -> bool {
        is_directory
            || match self {
                Self::All => true,
                Self::Runtime => !source,
                Self::Source => source,
            }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InspectorEntry {
    pub name: String,
    pub logical_path: String,
    pub kind: String,
    pub extension: String,
    pub is_directory: bool,
    pub source_asset: bool,
    pub byte_len: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InspectorField {
    pub label: String,
    pub value: String,
    pub category: String,
}

impl InspectorField {
    #[inline]
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            category: "inspection".to_owned(),
        }
    }

    #[inline]
    pub fn categorized(
        category: impl Into<String>,
        label: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            category: category.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetInspectorReport {
    pub asset_ref: String,
    pub title: String,
    pub asset_kind: String,
    pub document_kind: String,
    pub decoder: String,
    pub summary: String,
    pub counterpart: Option<String>,
    pub fields: Vec<InspectorField>,
    pub diagnostics: Vec<String>,
}
