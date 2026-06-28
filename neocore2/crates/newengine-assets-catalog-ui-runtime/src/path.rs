//! Asset Browser path presentation helpers.
//!
//! This is UI composition state only. It does not resolve bytes and it does not
//! create a backend domain; `engine.assets` remains the authoritative VFS owner.

pub(crate) fn parent_path(path: &str) -> String {
    let normalized = normalize_catalog_path(path);
    if normalized.is_empty() {
        return String::new();
    }
    normalized
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_owned())
        .unwrap_or_default()
}

pub(crate) fn normalize_catalog_path(path: &str) -> String {
    let mut value = path.trim().replace('\\', "/");
    while let Some(rest) = value.strip_prefix('/') {
        value = rest.to_owned();
    }
    while let Some(rest) = value.strip_prefix("./") {
        value = rest.to_owned();
    }
    while value.contains("//") {
        value = value.replace("//", "/");
    }
    value.trim_end_matches('/').to_owned()
}

pub(crate) fn display_path(path: &str) -> String {
    let path = normalize_catalog_path(path);
    if path.is_empty() {
        "/".to_owned()
    } else {
        format!("/{path}")
    }
}

pub(crate) fn browser_folder_label(path: &str) -> String {
    let path = normalize_catalog_path(path);
    path.rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("Content")
        .to_owned()
}
