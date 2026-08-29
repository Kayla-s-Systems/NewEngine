use super::*;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct DefinitionEntryProjection {
    pub(super) refs: DefinitionRefsProjection,
    pub(super) model_explanation: ModelExplanationProjection,
    pub(super) arbitrary_metadata: serde_json::Value,
    pub(super) warnings: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct DefinitionRefsProjection {
    pub(super) drawable_refs: Vec<String>,
    pub(super) material_refs: Vec<String>,
    pub(super) texture_refs: Vec<String>,
    pub(super) uv_layout_refs: Vec<String>,
    pub(super) physics_refs: Vec<String>,
    pub(super) collision_refs: Vec<String>,
    pub(super) ai_refs: Vec<String>,
    pub(super) streaming_refs: Vec<String>,
    pub(super) editor_refs: Vec<String>,
    pub(super) other_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub(super) struct ModelExplanationProjection {
    pub(super) model_ref: Option<String>,
    pub(super) drawable_ref: Option<String>,
    pub(super) material_bindings: Vec<newengine_model_domain_api::MaterialBindingRef>,
    pub(super) material_refs: Vec<String>,
    pub(super) texture_refs: Vec<String>,
    pub(super) uv_layout_refs: Vec<String>,
    pub(super) physics_refs: Vec<String>,
    pub(super) collision_refs: Vec<String>,
    pub(super) render_options: newengine_model_domain_api::MeshRenderOptions,
    pub(super) collision_policy: String,
    pub(super) uv_policy: String,
    pub(super) physics_policy: String,
    pub(super) lod_policy: String,
    pub(super) streaming_policy: String,
}

impl Default for ModelExplanationProjection {
    fn default() -> Self {
        Self {
            model_ref: None,
            drawable_ref: None,
            material_bindings: Vec::new(),
            material_refs: Vec::new(),
            texture_refs: Vec::new(),
            uv_layout_refs: Vec::new(),
            physics_refs: Vec::new(),
            collision_refs: Vec::new(),
            render_options: newengine_model_domain_api::MeshRenderOptions::world_opaque(),
            collision_policy: "unspecified".to_owned(),
            uv_policy: "authored".to_owned(),
            physics_policy: "unspecified".to_owned(),
            lod_policy: "unspecified".to_owned(),
            streaming_policy: "unspecified".to_owned(),
        }
    }
}

pub(super) fn model_configuration_from_projection(
    properties_ref: String,
    projection: DefinitionEntryProjection,
) -> Result<ModelRuntimeConfiguration, String> {
    let explanation = projection.model_explanation;
    let refs = projection.refs;
    Ok(ModelRuntimeConfiguration {
        properties_ref: Some(properties_ref),
        model_ref: explanation.model_ref,
        drawable_ref: explanation.drawable_ref,
        material_bindings: explanation.material_bindings,
        material_refs: merge_refs(explanation.material_refs, refs.material_refs),
        texture_refs: merge_refs(explanation.texture_refs, refs.texture_refs),
        uv_layout_refs: merge_refs(explanation.uv_layout_refs, refs.uv_layout_refs),
        physics_refs: merge_refs(explanation.physics_refs, refs.physics_refs),
        collision_refs: merge_refs(explanation.collision_refs, refs.collision_refs),
        ai_refs: refs.ai_refs,
        streaming_refs: refs.streaming_refs,
        editor_refs: refs.editor_refs,
        other_refs: refs.other_refs,
        render_options: explanation.render_options,
        collision_policy: explanation.collision_policy,
        uv_policy: explanation.uv_policy,
        physics_policy: explanation.physics_policy,
        lod_policy: explanation.lod_policy,
        streaming_policy: explanation.streaming_policy,
        metadata: projection.arbitrary_metadata,
        warnings: projection.warnings,
    })
}

fn merge_refs(mut primary: Vec<String>, secondary: Vec<String>) -> Vec<String> {
    primary.extend(secondary);
    primary.retain(|reference| !reference.trim().is_empty());
    primary.sort();
    primary.dedup();
    primary
}
