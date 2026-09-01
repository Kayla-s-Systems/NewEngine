use super::*;

/// Compile a decoded NEUI XMLcentral string directly into a provider-neutral UI tree.
///
/// This helper is intentionally DTO-only: callers that already own bytes/asset
/// lifetime can avoid reimplementing XML parsing while still keeping engine.ui
/// presentation authored by `.neui` instead of Rust code.
pub fn compile_xmlcentral_surface_root(
    xml: &str,

    document_ref: &str,

    style_ref: Option<&str>,
) -> Result<UiNodeRequest, String> {
    compile_request::validate_requested_entry(xml, "surface")?;

    let surface = parse_surface(xml)
        .ok_or_else(|| format!("{document_ref}: .neui document has no <Surface> entry"))?;

    compile_surface_root(
        xml,
        &surface,
        document_ref,
        style_ref.or(surface.theme.as_deref()),
        &NeUiDialect::builtin(),
    )
}

/// Decode packed `.neui` NEF8 bytes and compile its `@surface` entry into a provider-neutral UI tree.
pub fn compile_neui_bytes_surface_root(
    bytes: &[u8],

    document_ref: &str,

    style_ref: Option<&str>,
) -> Result<UiNodeRequest, String> {
    let (logical_path, _) = compile_request::split_ref(document_ref);

    let xml = compile_request::decode_neui_xmlcentral(
        if logical_path.trim().is_empty() {
            document_ref
        } else {
            &logical_path
        },
        bytes,
        LIST_FILE_CONTENT_KIND_NEUI,
    )?;

    compile_xmlcentral_surface_root(&xml, document_ref, style_ref)
}
