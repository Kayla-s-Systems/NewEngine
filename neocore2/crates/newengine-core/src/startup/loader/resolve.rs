fn resolve_startup_file_optional(
    _paths: &ConfigPaths,
    raw: &str,
) -> EngineResult<Option<(PathBuf, StartupResolvedFrom)>> {
    let p = Path::new(raw);

    if p.is_absolute() {
        if p.exists() {
            return Ok(Some((p.to_path_buf(), StartupResolvedFrom::Absolute)));
        }
        return Ok(None);
    }

    let roots = startup_search_roots()?;
    for root in &roots {
        let in_root = root.join(p);
        if in_root.exists() {
            return Ok(Some((in_root, StartupResolvedFrom::Cwd)));
        }

        // IDE launches from the outer repository root should still find
        // NewEngine/neocore2/config.json when the app spec says "config.json".
        let in_nested_neocore = root.join("NewEngine").join("neocore2").join(p);
        if in_nested_neocore.exists() {
            return Ok(Some((in_nested_neocore, StartupResolvedFrom::Cwd)));
        }
    }

    Ok(None)
}

fn startup_search_roots() -> EngineResult<Vec<PathBuf>> {
    let mut roots = Vec::new();
    let cwd = std::env::current_dir()
        .map_err(|e| EngineError::Other(format!("startup: current_dir failed err={}", e)))?;
    push_root_with_ancestors(&mut roots, cwd, 8);

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            push_root_with_ancestors(&mut roots, parent.to_path_buf(), 10);
        }
    }

    Ok(roots)
}

fn push_root_with_ancestors(out: &mut Vec<PathBuf>, mut root: PathBuf, max_up: usize) {
    for _ in 0..=max_up {
        if !out.iter().any(|existing| existing == &root) {
            out.push(root.clone());
        }
        if !root.pop() {
            break;
        }
    }
}

fn summarize_json(v: &serde_json::Value) -> String {
    // Compact representation with a hard cap to avoid log spam.
    // (Plugins should keep their base config inside the DLL; config.json carries overrides only.)
    const MAX: usize = 512;
    match serde_json::to_string(v) {
        Ok(s) if s.len() <= MAX => s,
        Ok(s) => {
            let mut out = s;
            out.truncate(MAX);
            out.push_str("...");
            out
        }
        Err(_) => "<invalid json>".to_owned(),
    }
}
