fn definition_render_options(
    entry: &serde_json::Value,
) -> Option<newengine_model_domain_api::MeshRenderOptions> {
    entry
        .get("model_explanation")
        .and_then(|value| value.get("render_options"))
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

fn apply_sky_drawable_from_ytyp(
    profile: &mut AuthoredWorldProfile,
    entry: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let Some(options) = definition_render_options(entry) else {
        return 0;
    };
    if !matches!(
        options.role,
        newengine_model_domain_api::MeshRenderRole::SkyBackground
            | newengine_model_domain_api::MeshRenderRole::CelestialBillboard
    ) {
        return 0;
    }
    let drawable = entry
        .get("model_explanation")
        .and_then(|value| value.get("drawable_ref"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(drawable) = drawable else {
        return 0;
    };
    profile.sky.mesh = drawable.replace('\\', "/");
    newengine_ulog_api::ulog::info!(
        "game-ready ytyp sky drawable: definition_ref='{}' mesh='{}' source='model_explanation.drawable_ref' policy='YTYP dependency graph owns skydome asset identity'",
        definition_ref,
        profile.sky.mesh
    );
    1
}

fn apply_render_options_from_ytyp(
    profile: &mut AuthoredWorldProfile,
    entry: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let Some(options) = definition_render_options(entry) else {
        return 0;
    };
    let target = match options.role {
        newengine_model_domain_api::MeshRenderRole::TerrainPatch => {
            profile.terrain.render_options = options.clone();
            "terrain"
        }
        newengine_model_domain_api::MeshRenderRole::FoliageInstanced => {
            profile.foliage.render_options = options.clone();
            "foliage"
        }
        newengine_model_domain_api::MeshRenderRole::SkyBackground
        | newengine_model_domain_api::MeshRenderRole::CelestialBillboard => {
            profile.sky.render_options = options.clone();
            "sky"
        }
        newengine_model_domain_api::MeshRenderRole::CharacterBody => {
            profile.player.model.render_options = options.clone();
            "player"
        }
        _ => {
            return 0;
        }
    };
    newengine_ulog_api::ulog::info!("game-ready ytyp render policy: target='{}' definition_ref='{}' role={:?} shadow_policy={:?} source='engine.assets.definitions/.ytyp'", target, definition_ref, options.role, options.shadow_policy);
    1
}
