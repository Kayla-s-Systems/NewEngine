use super::*;

pub(crate) struct SurfaceInfo {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) root: String,
    pub(crate) theme: Option<String>,
    pub(crate) modal: bool,
    pub(crate) z_order: i32,
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
