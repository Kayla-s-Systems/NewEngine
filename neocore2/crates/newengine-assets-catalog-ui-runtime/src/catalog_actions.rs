use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CatalogToolbarAction {
    Refresh,
    Tree,
    List,
    Grid,
}

impl CatalogToolbarAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Refresh => "Refresh",
            Self::Tree => "Tree",
            Self::List => "List",
            Self::Grid => "Grid",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum CatalogToolbarItem {
    DocumentAction { label: String, enabled: bool },
    ViewAction { label: &'static str },
}

pub(crate) fn catalog_toolbar_items(
    document_actions: &[AssetDocumentAction],
) -> Vec<CatalogToolbarItem> {
    let mut items = document_actions
        .iter()
        .map(|action| CatalogToolbarItem::DocumentAction {
            label: action.label.clone(),
            enabled: action.enabled,
        })
        .collect::<Vec<_>>();
    items.extend([
        CatalogToolbarItem::ViewAction {
            label: CatalogToolbarAction::Tree.label(),
        },
        CatalogToolbarItem::ViewAction {
            label: CatalogToolbarAction::List.label(),
        },
        CatalogToolbarItem::ViewAction {
            label: CatalogToolbarAction::Grid.label(),
        },
        CatalogToolbarItem::ViewAction {
            label: CatalogToolbarAction::Refresh.label(),
        },
    ]);
    items
}
