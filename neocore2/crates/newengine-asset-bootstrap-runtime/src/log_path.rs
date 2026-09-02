use std::path::Path;

#[inline]
pub fn shard_log_path_by_run_id(original: &str, run_id: &str) -> Option<String> {
    let source = original.trim();
    if source.is_empty() {
        return None;
    }

    let path = Path::new(source);
    let parent = path.parent();
    let file_name = path.file_name()?.to_string_lossy();
    let (stem, ext) = match (path.file_stem(), path.extension()) {
        (Some(stem), Some(ext)) => (stem.to_string_lossy(), Some(ext.to_string_lossy())),
        (Some(stem), None) => (stem.to_string_lossy(), None),
        _ => return None,
    };

    let new_file = match ext.as_deref() {
        Some("log") => format!("{stem}.{run_id}.log"),
        Some(ext) if !ext.is_empty() => format!("{stem}.{run_id}.{ext}"),
        _ => format!("{file_name}.{run_id}.log"),
    };

    Some(
        parent
            .map(|dir| dir.join(&new_file).to_string_lossy().to_string())
            .unwrap_or(new_file),
    )
}
