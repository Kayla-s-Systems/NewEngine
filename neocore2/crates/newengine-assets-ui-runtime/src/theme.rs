use super::*;
use crate::compile_request::{load_xmlcentral, split_ref};

pub(crate) struct SurfaceInfo {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) root: String,
    pub(crate) theme: Option<String>,
    pub(crate) modal: bool,
    pub(crate) z_order: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ThemeTokenBundle {
    pub(crate) theme_ref: String,
    pub(crate) theme_id: String,
    pub(crate) density: String,
    tokens: BTreeMap<String, String>,
    colors: BTreeMap<String, [u8; 4]>,
    metrics: BTreeMap<String, f32>,
    pub(crate) font_tokens: BTreeMap<String, String>,
}

pub(crate) fn parse_surface(xml: &str) -> Option<SurfaceInfo> {
    let element = first_element(xml, "Surface")?;
    let name = attr_value(&element.open, "name").unwrap_or_else(|| "engine.unknown".to_owned());
    let kind = attr_value(&element.open, "kind").unwrap_or_else(|| "surface".to_owned());
    let root = attr_value(&element.open, "root").unwrap_or_else(|| "layout.main".to_owned());
    let theme = attr_value(&element.open, "theme").filter(|value| !value.trim().is_empty());
    let modal = bool_attr(&element.open, "modal");
    let z_order = attr_value(&element.open, "z_order")
        .or_else(|| attr_value(&element.open, "z"))
        .and_then(|value| value.parse().ok())
        .unwrap_or(50);
    Some(SurfaceInfo {
        name,
        kind,
        root,
        theme,
        modal,
        z_order,
    })
}

pub(crate) fn extract_dependencies(xml: &str) -> Vec<String> {
    let mut deps = Vec::new();
    for tag in [
        "ThemeRef",
        "ComponentRef",
        "TextureRef",
        "FontRef",
        "SoundRef",
        "BindingGraphRef",
        "DocumentRef",
    ] {
        for element in elements(xml, tag) {
            if let Some(reference) = attr_value(&element.open, "ref") {
                if !reference.trim().is_empty() {
                    deps.push(reference);
                }
            }
        }
    }
    deps.sort();
    deps.dedup();
    deps
}

pub(crate) fn first_dependency_with_suffix(
    dependencies: &[String],
    suffix: &str,
) -> Option<String> {
    dependencies
        .iter()
        .find(|dep| {
            dep.to_ascii_lowercase()
                .contains(&suffix.to_ascii_lowercase())
        })
        .cloned()
}

pub(crate) fn parse_binding_plan(xml: &str, document_ref: &str, surface_id: &str) -> UiBindingPlan {
    let mut plan = UiBindingPlan {
        document_ref: document_ref.to_owned(),
        surface_id: surface_id.to_owned(),
        ..Default::default()
    };
    if let Some(graph) = first_element(xml, "BindingGraph") {
        for source in elements(&graph.inner, "StateSource") {
            plan.state_sources.push(UiStateSource {
                id: attr_value(&source.open, "id").unwrap_or_default(),
                source: attr_value(&source.open, "source").unwrap_or_default(),
                contract: attr_value(&source.open, "contract").unwrap_or_default(),
                update_policy: update_policy_from_attr(
                    attr_value(&source.open, "update").as_deref(),
                ),
            });
        }
        for bind in elements(&graph.inner, "Bind") {
            plan.bindings.push(UiBindingEdge {
                element_id: attr_value(&bind.open, "element").unwrap_or_default(),
                property: attr_value(&bind.open, "property").unwrap_or_default(),
                source_id: attr_value(&bind.open, "source_id").unwrap_or_default(),
                path: attr_value(&bind.open, "source").unwrap_or_default(),
                mode: UiBindingMode::OneWay,
                fallback: attr_value(&bind.open, "fallback"),
                transform: attr_value(&bind.open, "transform"),
            });
        }
    }
    for action in elements(xml, "Action") {
        if let Some(action_id) = attr_value(&action.open, "id") {
            plan.actions.push(UiActionEdge {
                element_id: attr_value(&action.open, "element").unwrap_or_default(),
                trigger: attr_value(&action.open, "trigger").unwrap_or_else(|| "click".to_owned()),
                action_id,
                target_gateway: attr_value(&action.open, "target").unwrap_or_default(),
                command: attr_value(&action.open, "command")
                    .or_else(|| attr_value(&action.open, "event"))
                    .unwrap_or_default(),
                payload_schema: None,
            });
        }
    }
    plan
}

pub(crate) fn update_policy_from_attr(value: Option<&str>) -> UiUpdatePolicy {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "frame" => UiUpdatePolicy::Frame,
        "event" => UiUpdatePolicy::Event,
        "dirty" => UiUpdatePolicy::Dirty,
        "manual" => UiUpdatePolicy::Manual,
        _ => UiUpdatePolicy::OnChange,
    }
}

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
        let root = parse_ui_node_element(xml, &root_element, source_ref, 0, 0)
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

pub(crate) fn resolve_theme_token_bundle(
    state: &mut AssetsUiRuntimeState,
    libraries: &[UiThemeLibraryRef],
    fallback_ref: Option<&str>,
    warnings: &mut Vec<String>,
) -> Option<ThemeTokenBundle> {
    let mut resolved = ThemeTokenBundle::default();
    for library in libraries {
        if library.theme_ref.trim().is_empty() {
            continue;
        }
        let request = AssetsUiRefRequest {
            document_ref: library.theme_ref.clone(),
            ..Default::default()
        };
        match load_xmlcentral(state, request) {
            Ok((xml, _, resolved_ref)) => {
                let bundle = parse_theme_tokens(&xml, &resolved_ref.document_ref, &library.entries);
                if let Some(bundle) = bundle {
                    merge_theme_tokens(&mut resolved, bundle);
                    warnings.push(format!(
                        ".neui theme library resolved ref='{}' density='{}' tokens={}",
                        resolved_ref.document_ref,
                        resolved.density,
                        resolved.tokens.len()
                    ));
                } else {
                    warnings.push(format!(
                        ".neui theme library contains no Theme entry ref='{}'",
                        resolved_ref.document_ref
                    ));
                }
            }
            Err(err) => warnings.push(format!(
                ".neui theme library unresolved ref='{}' err='{}'",
                library.theme_ref, err
            )),
        }
    }
    if resolved.theme_ref.trim().is_empty() {
        if let Some(reference) = fallback_ref.filter(|it| !it.trim().is_empty()) {
            resolved.theme_ref = reference.to_owned();
            resolved.theme_id = reference.to_owned();
            resolved.density = "normal".to_owned();
            return Some(resolved);
        }
        return None;
    }
    Some(resolved)
}

pub(crate) fn parse_theme_tokens(
    xml: &str,
    source_ref: &str,
    entries: &[String],
) -> Option<ThemeTokenBundle> {
    let themes = elements(xml, "Theme");
    let selected = if entries.is_empty() {
        themes.into_iter().next()
    } else {
        themes.into_iter().find(|theme| {
            attr_value(&theme.open, "name")
                .map(|name| entries.iter().any(|entry| entry == &name))
                .unwrap_or(false)
        })
    }?;

    let mut bundle = ThemeTokenBundle {
        theme_ref: source_ref.to_owned(),
        theme_id: attr_value(&selected.open, "id")
            .or_else(|| attr_value(&selected.open, "theme_id"))
            .or_else(|| attr_value(&selected.open, "name"))
            .unwrap_or_else(|| source_ref.to_owned()),
        density: attr_value(&selected.open, "density").unwrap_or_else(|| "normal".to_owned()),
        ..Default::default()
    };

    for token in elements(&selected.inner, "Token") {
        let Some(name) = attr_value(&token.open, "name").filter(|it| !it.trim().is_empty()) else {
            continue;
        };
        let value = attr_value(&token.open, "value")
            .or_else(|| attr_value(&token.open, "ref"))
            .unwrap_or_default();
        insert_theme_token(&mut bundle, &name, &value);
    }
    for color in elements(&selected.inner, "Color") {
        if let (Some(name), Some(value)) = (
            attr_value(&color.open, "name"),
            attr_value(&color.open, "value"),
        ) {
            insert_theme_token(&mut bundle, &format!("color.{name}"), &value);
        }
    }
    for metric in elements(&selected.inner, "Metric") {
        if let (Some(name), Some(value)) = (
            attr_value(&metric.open, "name"),
            attr_value(&metric.open, "value"),
        ) {
            insert_theme_token(&mut bundle, &format!("metric.{name}"), &value);
        }
    }
    for font in elements(&selected.inner, "FontToken") {
        if let (Some(name), Some(value)) = (
            attr_value(&font.open, "name"),
            attr_value(&font.open, "ref").or_else(|| attr_value(&font.open, "value")),
        ) {
            insert_theme_token(&mut bundle, &format!("font.{name}"), &value);
        }
    }
    Some(bundle)
}

pub(crate) fn insert_theme_token(bundle: &mut ThemeTokenBundle, name: &str, value: &str) {
    let name = name.trim().to_owned();
    let value = value.trim().to_owned();
    if name.is_empty() {
        return;
    }
    if name == "density" || name == "density.mode" {
        if !value.is_empty() {
            bundle.density = value.clone();
        }
    }
    if let Some(color) = parse_hex_rgba(&value) {
        if name.starts_with("color.") {
            bundle.colors.insert(name.clone(), color);
        }
    }
    if name.starts_with("metric.") {
        if let Ok(number) = value.parse::<f32>() {
            bundle
                .metrics
                .insert(name.trim_start_matches("metric.").to_owned(), number);
        }
    }
    if name.starts_with("font.") && !value.is_empty() {
        bundle
            .font_tokens
            .insert(name.trim_start_matches("font.").to_owned(), value.clone());
    }
    bundle.tokens.insert(name, value);
}

pub(crate) fn merge_theme_tokens(target: &mut ThemeTokenBundle, source: ThemeTokenBundle) {
    if !source.theme_ref.trim().is_empty() {
        target.theme_ref = source.theme_ref;
    }
    if !source.theme_id.trim().is_empty() {
        target.theme_id = source.theme_id;
    }
    if !source.density.trim().is_empty() {
        target.density = source.density;
    }
    target.tokens.extend(source.tokens);
    target.colors.extend(source.colors);
    target.metrics.extend(source.metrics);
    target.font_tokens.extend(source.font_tokens);
}

pub(crate) fn parse_hex_rgba(value: &str) -> Option<[u8; 4]> {
    let hex = value.trim().trim_start_matches('#');
    let read = |range: std::ops::Range<usize>| u8::from_str_radix(hex.get(range)?, 16).ok();
    match hex.len() {
        6 => Some([read(0..2)?, read(2..4)?, read(4..6)?, 255]),
        8 => Some([read(0..2)?, read(2..4)?, read(4..6)?, read(6..8)?]),
        _ => None,
    }
}
