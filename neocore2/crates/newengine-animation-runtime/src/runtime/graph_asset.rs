pub const ANIMATION_GRAPH_ASSET_SCHEMA_V1: &str = "northstar.animation_graph.v1";
pub const MAX_ANIMATION_GRAPH_ASSET_BYTES: usize = 4 * 1024 * 1024;

/// Versioned authored Animation Graph payload.
///
/// The schema wrapper is deliberately separate from `CompiledAnimationGraph`: authoring data stays
/// serializable/toolable while compilation resolves clip ownership, skeleton bindings, parameter
/// addresses, state addresses and masks into an immutable runtime plan.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationGraphAssetV1 {
    #[serde(rename = "$schema")]
    pub schema: String,
    #[serde(flatten)]
    pub definition: AnimationGraphDefinition,
}

impl AnimationGraphAssetV1 {
    #[inline]
    pub fn new(definition: AnimationGraphDefinition) -> Self {
        Self {
            schema: ANIMATION_GRAPH_ASSET_SCHEMA_V1.to_owned(),
            definition,
        }
    }
}

/// Decodes a canonical authored graph payload. Structural/runtime validation is intentionally
/// performed by `CompiledAnimationGraph::compile`, keeping one authoritative validation path.
pub fn decode_animation_graph_asset_v1(bytes: &[u8]) -> Result<AnimationGraphDefinition, String> {
    if bytes.is_empty() {
        return Err("animation graph asset is empty".to_owned());
    }
    if bytes.len() > MAX_ANIMATION_GRAPH_ASSET_BYTES {
        return Err(format!(
            "animation graph asset exceeds size limit bytes={} limit={MAX_ANIMATION_GRAPH_ASSET_BYTES}",
            bytes.len()
        ));
    }
    let asset: AnimationGraphAssetV1 = serde_json::from_slice(bytes)
        .map_err(|error| format!("animation graph asset JSON decode failed: {error}"))?;
    if asset.schema != ANIMATION_GRAPH_ASSET_SCHEMA_V1 {
        return Err(format!(
            "animation graph asset schema mismatch expected='{ANIMATION_GRAPH_ASSET_SCHEMA_V1}' actual='{}'",
            asset.schema
        ));
    }
    Ok(asset.definition)
}

/// Canonical pretty JSON encoder used by editor/importer tooling and tests.
pub fn encode_animation_graph_asset_v1(
    definition: &AnimationGraphDefinition,
) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(&AnimationGraphAssetV1::new(definition.clone()))
        .map_err(|error| format!("animation graph asset JSON encode failed: {error}"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationGraphAssetReference {
    pub logical_path: String,
    pub canonical_path_key: String,
}

impl AnimationGraphAssetReference {
    pub fn parse(reference: &str) -> Result<Self, String> {
        let logical_path = reference.trim().replace('\\', "/");
        if logical_path.is_empty() {
            return Err("animation graph asset reference is empty".to_owned());
        }
        if logical_path.contains('@') {
            return Err(format!(
                "animation graph asset reference cannot contain a clip selector ref='{reference}'"
            ));
        }
        let logical_path = logical_path.trim_start_matches('/').to_owned();
        if logical_path.is_empty() {
            return Err("animation graph asset reference has no logical path".to_owned());
        }
        Ok(Self {
            canonical_path_key: logical_path.to_ascii_lowercase(),
            logical_path,
        })
    }
}
