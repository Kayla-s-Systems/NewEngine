use super::*;

pub(crate) fn invalidate_caches(
    state: &mut AssetsUiRuntimeState,
    request: AssetsUiInvalidateRequest,
) -> serde_json::Value {
    let mut xml_n = 0usize;
    let mut compile_n = 0usize;
    let mut dialect_n = 0usize;
    let document_ref = request.document_ref.trim();
    let dialect_ref = request.dialect_ref.trim();
    let mut scope = "selective";

    if request.all || (document_ref.is_empty() && dialect_ref.is_empty()) {
        scope = "all";
        xml_n = state.xml_cache.len();
        compile_n = state.compile_cache.len();
        dialect_n = state.dialect_cache.len();
        state.xml_cache.clear();
        state.compile_cache.clear();
        state.dialect_cache.clear();
    } else {
        if !document_ref.is_empty() {
            let (path, entry) = split_ref(document_ref);
            let entry = if entry.trim().is_empty() {
                "surface".to_owned()
            } else {
                entry
            };
            let canonical = format!("{}@{}", path, entry);
            if state.compile_cache.remove(&canonical).is_some() {
                compile_n += 1;
            }
            for candidate in vfs_candidates(&path) {
                if state.xml_cache.remove(&candidate).is_some() {
                    xml_n += 1;
                }
            }
        }
        if !dialect_ref.is_empty() {
            let canonical = canonical_dialect_ref(dialect_ref);
            if state.dialect_cache.remove(&canonical).is_some() {
                dialect_n += 1;
            }
            compile_n += state.compile_cache.len();
            state.compile_cache.clear();
        }
    }

    let mut out = serde_json::Map::new();
    out.insert("ok".to_owned(), serde_json::Value::Bool(true));
    out.insert(
        "schema".to_owned(),
        serde_json::Value::String("newengine.assets.ui.invalidate.response.v1".to_owned()),
    );
    out.insert(
        "scope".to_owned(),
        serde_json::Value::String(scope.to_owned()),
    );
    out.insert(
        "document_ref".to_owned(),
        serde_json::Value::String(document_ref.to_owned()),
    );
    out.insert(
        "dialect_ref".to_owned(),
        serde_json::Value::String(dialect_ref.to_owned()),
    );
    out.insert("cleared_xml_entries".to_owned(), serde_json::json!(xml_n));
    out.insert(
        "cleared_compile_entries".to_owned(),
        serde_json::json!(compile_n),
    );
    out.insert(
        "cleared_dialect_entries".to_owned(),
        serde_json::json!(dialect_n),
    );
    serde_json::Value::Object(out)
}
