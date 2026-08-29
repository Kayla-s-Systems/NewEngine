use super::*;

pub(crate) fn parse_navigation_document(
    xml: &str,
) -> Result<Option<UiNodeNavigationDocument>, String> {
    let Some(navigation) = first_element(xml, "UiNodeNavigationDocument") else {
        return Ok(None);
    };
    let mut doc = UiNodeNavigationDocument {
        id: attr_value(&navigation.open, "id").unwrap_or_else(|| "engine.ui.primary".to_owned()),
        version: attr_value(&navigation.open, "version")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        surface_id: attr_value(&navigation.open, "surface_id")
            .or_else(|| attr_value(&navigation.open, "surface"))
            .unwrap_or_else(|| "engine.ui.primary".to_owned()),
        root_page: attr_value(&navigation.open, "root_page").unwrap_or_else(|| "root".to_owned()),
        title: attr_value(&navigation.open, "title").unwrap_or_default(),
        subtitle: attr_value(&navigation.open, "subtitle").unwrap_or_default(),
        footer_lines: Vec::new(),
        pages: Vec::new(),
    };

    if let Some(footer) = first_element(&navigation.inner, "Footer") {
        for line in elements(&footer.inner, "Line") {
            if let Some(value) = attr_value(&line.open, "value") {
                if !value.trim().is_empty() {
                    doc.footer_lines.push(value);
                }
            }
        }
    }

    for page_element in elements(&navigation.inner, "Page") {
        let mut page = UiNodeNavigationPage {
            id: attr_value(&page_element.open, "id").unwrap_or_default(),
            title: attr_value(&page_element.open, "title").unwrap_or_default(),
            subtitle: attr_value(&page_element.open, "subtitle").unwrap_or_default(),
            parent_page: attr_value(&page_element.open, "parent_page"),
            footer_lines: Vec::new(),
            items: Vec::new(),
            back_route: first_route_element(&page_element.inner, "Back"),
        };
        if let Some(footer) = first_element(&page_element.inner, "Footer") {
            for line in elements(&footer.inner, "Line") {
                if let Some(value) = attr_value(&line.open, "value") {
                    page.footer_lines.push(value);
                }
            }
        }
        for item_element in elements(&page_element.inner, "Item") {
            let item = UiNodeNavigationItem {
                id: attr_value(&item_element.open, "id").unwrap_or_default(),
                label: attr_value(&item_element.open, "label").unwrap_or_default(),
                value: attr_value(&item_element.open, "value"),
                detail: attr_value(&item_element.open, "detail"),
                emphasized: bool_attr(&item_element.open, "emphasized"),
                tone: tone_from_attr(attr_value(&item_element.open, "tone").as_deref()),
                dynamic_value: attr_value(&item_element.open, "dynamic_value"),
                action: first_route_element(&item_element.inner, "Action"),
                nav_left: first_route_element(&item_element.inner, "NavLeft"),
                nav_right: first_route_element(&item_element.inner, "NavRight"),
            };
            page.items.push(item);
        }
        doc.pages.push(page);
    }
    doc = doc.canonicalized();
    doc.validate()?;
    Ok(Some(doc))
}
