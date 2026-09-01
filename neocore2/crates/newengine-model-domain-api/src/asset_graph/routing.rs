pub(super) fn classify_ref(
    _reference: &str,
) -> (&'static str, &'static str, &'static str, &'static str) {
    // The API layer cannot classify a file format by extension. Runtime hydration
    // resolves the authoritative AssetFileTypeDescriptor through engine.assets.types
    // and replaces these neutral placeholders with descriptor-owned semantics.
    ("asset", "unknown", "engine.assets", "asset.decode_v1")
}
