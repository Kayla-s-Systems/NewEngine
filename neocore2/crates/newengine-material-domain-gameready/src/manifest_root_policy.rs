use newengine_material_domain_api::{MaterialDomainError, MaterialDomainResult};

pub(crate) const DEFERRED_ROOT_PREFIX: &str = "shaders/deferred/";

pub(crate) const REQUIRED_DEFERRED_ROOT_SHADERS: &[&str] = &[
    "shaders/deferred/gbuffer_lit_shadowed.frag",
    "shaders/deferred/gbuffer_terrain.frag",
];

pub(crate) fn validate_manifest_shader_path(
    manifest_path: &str,
    field: &str,
    logical_path: &str,
) -> MaterialDomainResult<()> {
    if !logical_path.starts_with(DEFERRED_ROOT_PREFIX) {
        return Ok(());
    }

    if REQUIRED_DEFERRED_ROOT_SHADERS.contains(&logical_path) {
        return Ok(());
    }

    Err(MaterialDomainError::other(format!(
        "GameReady shader manifest path='{manifest_path}' field='{field}' references deferred renderer-root shader '{logical_path}', but it is not in REQUIRED_DEFERRED_ROOT_SHADERS; add it to engine-render-vulkan root_internal bundle before using it"
    )))
}

#[cfg(test)]
mod tests {
    use super::{validate_manifest_shader_path, REQUIRED_DEFERRED_ROOT_SHADERS};

    #[test]
    fn known_gameready_deferred_shaders_are_root_allowed() {
        for path in REQUIRED_DEFERRED_ROOT_SHADERS {
            validate_manifest_shader_path("test_manifest", "test_field", path)
                .expect("required deferred shader should be allowed");
        }
    }

    #[test]
    fn unknown_deferred_shader_requires_renderer_root_registration() {
        let err = validate_manifest_shader_path(
            "test_manifest",
            "gbuffer_custom_fs",
            "shaders/deferred/custom.frag",
        )
        .expect_err("unknown deferred shader must fail policy validation");
        assert!(err.to_string().contains("renderer-root"));
    }
}
