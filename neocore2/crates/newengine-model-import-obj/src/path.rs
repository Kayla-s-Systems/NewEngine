pub fn normalize_logical_path(raw: &str, allow_selector: bool) -> Result<String, String> {
    let trimmed = raw.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Err("empty asset path".to_owned());
    }
    if !allow_selector && trimmed.contains('@') {
        return Err(format!(
            "texture selector is not allowed for asset path '{raw}'"
        ));
    }

    let (path, selector) = if allow_selector {
        match trimmed.split_once('@') {
            Some((path, selector)) => (path, Some(selector)),
            None => (trimmed.as_str(), None),
        }
    } else {
        (trimmed.as_str(), None)
    };

    if path.is_empty() || path.starts_with('/') || path.contains(':') {
        return Err(format!("invalid logical asset path '{raw}'"));
    }

    let mut parts = Vec::new();
    for part in path.split('/') {
        let part = part.trim();
        if part.is_empty() || matches!(part, "." | "..") {
            return Err(format!("invalid logical asset path '{raw}'"));
        }
        parts.push(part);
    }

    let normalized = parts.join("/");
    match selector {
        Some(selector) => normalize_selector(&normalized, selector, raw),
        None => Ok(normalized),
    }
}

fn normalize_selector(path: &str, selector: &str, raw: &str) -> Result<String, String> {
    let selector = selector.trim();
    if selector.is_empty()
        || selector.contains('@')
        || selector.contains('/')
        || selector.contains('\\')
        || selector.contains(':')
    {
        return Err(format!("invalid logical texture selector '{raw}'"));
    }
    Ok(format!("{path}@{selector}"))
}

#[inline]
pub fn logical_dir(logical_path: &str) -> &str {
    logical_path
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("")
}

pub fn join_logical_path(base_dir: &str, relative: &str) -> Result<String, String> {
    let relative = normalize_logical_path(relative, true)?;
    if base_dir.trim().is_empty() {
        Ok(relative)
    } else {
        Ok(format!("{}/{}", base_dir.trim_end_matches('/'), relative))
    }
}

pub(crate) fn mtl_texture_path(base_dir: &str, tokens: &[&str]) -> Option<String> {
    let raw = tokens
        .iter()
        .rev()
        .find(|token| !token.trim().is_empty() && !token.starts_with('-'))?;
    join_logical_path(base_dir, raw).ok()
}
