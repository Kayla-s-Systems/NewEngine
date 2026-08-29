use super::*;

pub(super) fn normalize_logical_path(path: &str) -> String {
    let mut s = path.trim().replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_owned();
    }
    s.trim_start_matches('/').to_ascii_lowercase()
}

pub(super) fn path_extension(path: &str) -> String {
    let path = path.split('@').next().unwrap_or(path);
    path.rsplit_once('.')
        .map(|(_, ext)| AssetFileTypeDescriptor::extension_key(ext))
        .unwrap_or_default()
}
