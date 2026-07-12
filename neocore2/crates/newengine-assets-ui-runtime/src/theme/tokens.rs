use super::*;

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
    if (name == "density" || name == "density.mode") && !value.is_empty() {
        bundle.density = value.clone();
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
