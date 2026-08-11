use serde::{Deserialize, Serialize};

use crate::normalization::{normalize_non_empty_strings, normalize_optional_string};
use crate::UiNodeActionRoute;

pub const ENGINE_PRIMARY_UI_DOCUMENT_ID: &str = "engine.ui.primary";
/// Canonical authored primary UI source. The preferred path is asset-backed `.neui`
/// compiled by `engine.assets.ui`; runtime may also provide a generated/streamed
/// `UiNodeNavigationDocument` through the same DTO contract when the asset is absent.
/// Runtime JSON navigation assets are intentionally not supported as compatibility fallback.
pub const ENGINE_PRIMARY_UI_SURFACE_REF: &str = "assets/ui/engine/main_menu.neui@surface";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeNavigationTone {
    Normal,
    Accent,
    Danger,
    Disabled,
}

impl Default for UiNodeNavigationTone {
    #[inline]
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiNodeNavigationDocument {
    pub id: String,
    #[serde(default = "default_version")]
    pub version: u32,
    pub surface_id: String,
    #[serde(default = "default_root_page")]
    pub root_page: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub footer_lines: Vec<String>,
    #[serde(default)]
    pub pages: Vec<UiNodeNavigationPage>,
}

impl UiNodeNavigationDocument {
    #[inline]
    pub fn from_json_str(src: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str::<Self>(src).map(Self::canonicalized)
    }

    pub fn canonicalized(mut self) -> Self {
        self.id = self.id.trim().to_owned();
        self.surface_id = self.surface_id.trim().to_owned();
        self.root_page = self.root_page.trim().to_owned();
        self.title = self.title.trim().to_owned();
        self.subtitle = self.subtitle.trim().to_owned();
        self.footer_lines = normalize_non_empty_strings(self.footer_lines);
        for page in &mut self.pages {
            page.canonicalize_in_place();
        }
        self
    }

    #[inline]
    pub fn page(&self, page_id: &str) -> Option<&UiNodeNavigationPage> {
        self.pages.iter().find(|page| page.id == page_id)
    }

    #[inline]
    pub fn root(&self) -> Option<&UiNodeNavigationPage> {
        self.page(&self.root_page)
    }

    #[inline]
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("ui navigation document id is empty".to_owned());
        }
        if self.surface_id.is_empty() {
            return Err(format!(
                "ui navigation document '{}' surface_id is empty",
                self.id
            ));
        }
        if self.root_page.is_empty() {
            return Err(format!(
                "ui navigation document '{}' root_page is empty",
                self.id
            ));
        }
        if self.root().is_none() {
            return Err(format!(
                "ui navigation document '{}' root_page '{}' is missing",
                self.id, self.root_page
            ));
        }
        for page in &self.pages {
            page.validate(self)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiNodeNavigationPage {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub parent_page: Option<String>,
    #[serde(default)]
    pub footer_lines: Vec<String>,
    #[serde(default)]
    pub items: Vec<UiNodeNavigationItem>,
    #[serde(default)]
    pub back_route: Option<UiNodeActionRoute>,
}

impl UiNodeNavigationPage {
    fn canonicalize_in_place(&mut self) {
        self.id = self.id.trim().to_owned();
        self.title = self.title.trim().to_owned();
        self.subtitle = self.subtitle.trim().to_owned();
        self.parent_page = self
            .parent_page
            .take()
            .and_then(|value| normalize_optional_string(&value));
        self.footer_lines = normalize_non_empty_strings(std::mem::take(&mut self.footer_lines));
        if let Some(route) = &mut self.back_route {
            route.canonicalize_in_place();
        }
        for item in &mut self.items {
            item.canonicalize_in_place();
        }
    }

    fn validate(&self, document: &UiNodeNavigationDocument) -> Result<(), String> {
        if self.id.is_empty() {
            return Err(format!(
                "ui navigation document '{}' contains page with empty id",
                document.id
            ));
        }
        if let Some(parent) = self.parent_page.as_deref() {
            if document.page(parent).is_none() {
                return Err(format!(
                    "ui navigation page '{}' references missing parent_page '{}'",
                    self.id, parent
                ));
            }
        }
        for item in &self.items {
            item.validate(self)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiNodeNavigationItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub emphasized: bool,
    #[serde(default)]
    pub tone: UiNodeNavigationTone,
    #[serde(default)]
    pub dynamic_value: Option<String>,
    #[serde(default)]
    pub action: Option<UiNodeActionRoute>,
    #[serde(default)]
    pub nav_left: Option<UiNodeActionRoute>,
    #[serde(default)]
    pub nav_right: Option<UiNodeActionRoute>,
}

impl UiNodeNavigationItem {
    fn canonicalize_in_place(&mut self) {
        self.id = self.id.trim().to_owned();
        self.label = self.label.trim().to_owned();
        self.value = self
            .value
            .take()
            .and_then(|value| normalize_optional_string(&value));
        self.detail = self
            .detail
            .take()
            .and_then(|value| normalize_optional_string(&value));
        self.dynamic_value = self
            .dynamic_value
            .take()
            .and_then(|value| normalize_optional_string(&value));
        for route in [&mut self.action, &mut self.nav_left, &mut self.nav_right]
            .into_iter()
            .flatten()
        {
            route.canonicalize_in_place();
        }
    }

    fn validate(&self, page: &UiNodeNavigationPage) -> Result<(), String> {
        if self.id.is_empty() {
            return Err(format!(
                "ui navigation page '{}' contains item with empty id",
                page.id
            ));
        }
        if self.label.is_empty() {
            return Err(format!("ui navigation item '{}' has empty label", self.id));
        }
        Ok(())
    }
}

#[inline]
fn default_version() -> u32 {
    1
}

#[inline]
fn default_root_page() -> String {
    "root".to_owned()
}
