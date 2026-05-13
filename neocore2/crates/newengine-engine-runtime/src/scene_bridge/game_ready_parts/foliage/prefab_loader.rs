fn prefab_asset_raw_bytes(logical_path: &str) -> Result<Vec<u8>, String> {
    let assets = AssetServiceClient::new(default_host_api());
    assets
        .raw_bytes_v1(logical_path)
        .map_err(|e| format!("AssetManager raw read failed path='{logical_path}' err='{e}'"))
}

#[inline]
fn value_array<'a>(v: &'a serde_json::Value, key: &str) -> Result<&'a Vec<serde_json::Value>, String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .ok_or_else(|| format!("gltf: missing array '{key}'"))
}

#[inline]
fn value_index<'a>(arr: &'a [serde_json::Value], index: usize, what: &str) -> Result<&'a serde_json::Value, String> {
    arr.get(index)
        .ok_or_else(|| format!("gltf: {what} index out of range index={index} len={}", arr.len()))
}

#[inline]
fn u64_field(v: &serde_json::Value, key: &str, default: Option<u64>) -> Result<u64, String> {
    match v.get(key).and_then(|x| x.as_u64()) {
        Some(x) => Ok(x),
        None => default.ok_or_else(|| format!("gltf: missing integer field '{key}'")),
    }
}

#[inline]
fn str_field<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|x| x.as_str()).map(str::trim).filter(|x| !x.is_empty())
}

#[inline]
fn logical_dir(path: &str) -> &str {
    path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("")
}

fn join_logical_path(base_dir: &str, rel: &str) -> Result<String, String> {
    let rel = rel.trim().replace('\\', "/");
    if rel.is_empty() {
        return Err("empty relative asset path".to_owned());
    }
    if rel.starts_with("data:") {
        return Err("embedded data: URIs are not used by this runtime path; use AssetManager VFS files".to_owned());
    }
    if rel.contains("://") || rel.starts_with('/') {
        return Err(format!("external/absolute asset URI is not allowed: '{rel}'"));
    }

    let mut parts = Vec::<&str>::new();
    for part in base_dir.split('/').chain(rel.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                let _ = parts.pop();
            }
            x => parts.push(x),
        }
    }
    Ok(parts.join("/"))
}

fn load_prefab_logical_asset(prefab: &GameReadyPrefabSpec) -> Result<String, String> {
    let bytes = prefab_asset_raw_bytes(&prefab.source)?;
    let doc: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("prefab json parse failed source='{}' err='{e}'", prefab.source))?;
    let logical_asset = str_field(&doc, "logical_asset")
        .ok_or_else(|| format!("prefab source='{}' has no logical_asset", prefab.source))?;

    // Prefer the authored logical asset path exactly as declared. If it is a
    // relative sidecar path such as "scene.gltf", resolve it against the prefab
    // document directory. Both probes go through AssetManager raw VFS access.
    if prefab_asset_raw_bytes(logical_asset).is_ok() {
        Ok(logical_asset.to_owned())
    } else {
        join_logical_path(logical_dir(&prefab.source), logical_asset)
    }
}
