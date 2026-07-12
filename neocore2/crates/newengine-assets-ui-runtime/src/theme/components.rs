use super::*;

pub(crate) fn parse_component_libraries(xml: &str) -> Vec<UiComponentLibraryRef> {
    let mut libraries = BTreeMap::<String, Vec<String>>::new();
    for element in elements(xml, "ComponentRef") {
        let Some(reference) = attr_value(&element.open, "ref").filter(|it| !it.trim().is_empty())
        else {
            continue;
        };
        let (library_ref, entry) = split_ref(&reference);
        if entry.trim().is_empty() {
            libraries.entry(library_ref).or_default();
        } else {
            libraries.entry(library_ref).or_default().push(entry);
        }
    }
    libraries
        .into_iter()
        .map(|(library_ref, mut entries)| {
            entries.sort();
            entries.dedup();
            UiComponentLibraryRef {
                library_ref,
                entries,
            }
        })
        .collect()
}

pub(crate) fn parse_theme_libraries(
    xml: &str,
    surface_theme: Option<&str>,
) -> Vec<UiThemeLibraryRef> {
    let mut themes = BTreeMap::<String, Vec<String>>::new();
    if let Some(theme) = surface_theme.filter(|it| !it.trim().is_empty()) {
        let (theme_ref, entry) = split_ref(theme);
        if entry.trim().is_empty() {
            themes.entry(theme_ref).or_default();
        } else {
            themes.entry(theme_ref).or_default().push(entry);
        }
    }
    for element in elements(xml, "ThemeRef") {
        let Some(reference) = attr_value(&element.open, "ref").filter(|it| !it.trim().is_empty())
        else {
            continue;
        };
        let (theme_ref, entry) = split_ref(&reference);
        if entry.trim().is_empty() {
            themes.entry(theme_ref).or_default();
        } else {
            themes.entry(theme_ref).or_default().push(entry);
        }
    }
    themes
        .into_iter()
        .map(|(theme_ref, mut entries)| {
            entries.sort();
            entries.dedup();
            UiThemeLibraryRef { theme_ref, entries }
        })
        .collect()
}

pub(crate) fn parse_component_templates(xml: &str, source_ref: &str) -> Vec<UiComponentTemplate> {
    let mut templates = Vec::new();
    for component in elements(xml, "Component") {
        let id = attr_value(&component.open, "id")
            .or_else(|| attr_value(&component.open, "name"))
            .unwrap_or_default();
        if id.trim().is_empty() {
            continue;
        }
        let Some(root_element) = direct_child_elements(&component.inner)
            .into_iter()
            .find(|child| !is_metadata_element(&child.name))
        else {
            continue;
        };
        let root = parse_ui_node_element(
            xml,
            &root_element,
            source_ref,
            0,
            0,
            &NeUiDialect::builtin(),
        )
        .unwrap_or_else(|| UiNodeRequest::new(format!("{id}.root"), UiRuntimeNodeKind::Panel));
        let required_props = attr_value(&component.open, "required_props")
            .unwrap_or_default()
            .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
            .filter(|it| !it.trim().is_empty())
            .map(str::to_owned)
            .collect();
        templates.push(UiComponentTemplate {
            id,
            source_ref: source_ref.to_owned(),
            required_props,
            root,
        });
    }
    templates
}

pub(crate) fn resolve_imported_component_templates(
    state: &mut AssetsUiRuntimeState,
    libraries: &[UiComponentLibraryRef],
    warnings: &mut Vec<String>,
) -> Vec<UiComponentTemplate> {
    let mut out = Vec::new();
    for library in libraries {
        if library.library_ref.trim().is_empty() {
            continue;
        }
        let request = AssetsUiRefRequest {
            document_ref: library.library_ref.clone(),
            ..Default::default()
        };
        match load_xmlcentral(state, request) {
            Ok((xml, _, resolved)) => {
                let mut templates = parse_component_templates(&xml, &resolved.document_ref);
                if !library.entries.is_empty() {
                    templates.retain(|template| {
                        library.entries.iter().any(|entry| entry == &template.id)
                    });
                }
                warnings.push(format!(
                    ".neui component library resolved ref='{}' templates={}",
                    resolved.document_ref,
                    templates.len()
                ));
                out.extend(templates);
            }
            Err(err) => warnings.push(format!(
                ".neui component library unresolved ref='{}' err='{}'",
                library.library_ref, err
            )),
        }
    }
    out
}

pub(crate) fn merge_component_templates(
    imported: Vec<UiComponentTemplate>,
    local: Vec<UiComponentTemplate>,
) -> Vec<UiComponentTemplate> {
    let mut by_id = BTreeMap::<String, UiComponentTemplate>::new();
    for template in imported.into_iter().chain(local.into_iter()) {
        if template.id.trim().is_empty() {
            continue;
        }
        by_id.insert(template.id.clone(), template);
    }
    by_id.into_values().collect()
}
