#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

/// Resolve the canonical startup config for the PreStart editor.
///
/// IDE launches commonly start from repository root, `target/debug`, or an app
/// subdirectory. The startup window must still find the same `config.json` that
/// the engine will load; otherwise the PreStart gate silently edits a sibling
/// file or fails before the user can verify launch configuration.
pub(crate) fn resolve_for_edit(raw_path: &str) -> Result<PathBuf, String> {
    let raw = Path::new(raw_path);
    if raw.is_absolute() {
        return Ok(raw.to_path_buf());
    }

    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        push_root_with_ancestors(&mut roots, cwd, 8);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            push_root_with_ancestors(&mut roots, parent.to_path_buf(), 10);
        }
    }

    for root in &roots {
        let candidate = root.join(raw);
        if candidate.exists() {
            return Ok(candidate);
        }

        // Launch from the outer source root is common in IDE run configs.
        let nested_neocore = root.join("NewEngine").join("neocore2").join(raw);
        if nested_neocore.exists() {
            return Ok(nested_neocore);
        }
    }

    // If no file exists yet, create/edit the most stable candidate available.
    if let Some(root) = roots.iter().find(|root| root.join("NewEngine").join("neocore2").exists()) {
        return Ok(root.join("NewEngine").join("neocore2").join(raw));
    }

    if let Some(root) = roots.first() {
        return Ok(root.join(raw));
    }

    Err("unable to resolve startup config path: current_dir/current_exe unavailable".to_owned())
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
