use super::*;

#[derive(Clone, Debug, Serialize)]
pub struct DefinitionsServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub provider: &'static str,
    pub contract: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub methods: &'static [&'static str],
    pub ownership_policy: &'static str,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionRefRequest {
    pub definition_ref: String,
    pub source: String,
    pub entry: Option<String>,
}

impl Default for DefinitionRefRequest {
    #[inline]
    fn default() -> Self {
        Self {
            definition_ref: String::new(),
            source: String::new(),
            entry: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionManifestRequest {
    pub source: String,
    pub definition_ref: String,
}

impl Default for DefinitionManifestRequest {
    #[inline]
    fn default() -> Self {
        Self {
            source: String::new(),
            definition_ref: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct RawDefinitionEntryV1 {
    #[serde(alias = "id", alias = "asset_name", alias = "assetName")]
    pub(super) name: String,
    #[serde(alias = "stableHash")]
    pub(super) stable_hash: u64,
    #[serde(alias = "entryKind")]
    pub(super) entry_kind: String,
    pub(super) kind: String,
    pub(super) schema: String,
    pub(super) target: Option<serde_json::Value>,
    pub(super) dependencies: Vec<AssetDependencyRecord>,
    pub(super) namespaces: BTreeMap<String, serde_json::Value>,
    pub(super) metadata: BTreeMap<String, serde_json::Value>,
    #[serde(alias = "materialBindings")]
    pub(super) material_bindings: Vec<MaterialBindingRef>,
    #[serde(alias = "semanticTags")]
    pub(super) semantic_tags: Vec<String>,
    #[serde(alias = "domainTags")]
    pub(super) domain_tags: Vec<String>,
    #[serde(alias = "sideEffects")]
    pub(super) side_effects: Vec<DefinitionSideEffectV1>,
    pub(super) flags: u32,
}

impl Default for RawDefinitionEntryV1 {
    fn default() -> Self {
        Self {
            name: String::new(),
            stable_hash: 0,
            entry_kind: "archetype_definition".to_owned(),
            kind: String::new(),
            schema: String::new(),
            target: None,
            dependencies: Vec::new(),
            namespaces: BTreeMap::new(),
            metadata: BTreeMap::new(),
            material_bindings: Vec::new(),
            semantic_tags: Vec::new(),
            domain_tags: Vec::new(),
            side_effects: Vec::new(),
            flags: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DefinitionIdentityV1 {
    pub name: String,
    pub source: String,
    pub definition_ref: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionRefsV1 {
    pub drawable_refs: Vec<String>,
    pub material_refs: Vec<String>,
    pub texture_refs: Vec<String>,
    pub uv_layout_refs: Vec<String>,
    pub physics_refs: Vec<String>,
    pub collision_refs: Vec<String>,
    pub ai_refs: Vec<String>,
    pub streaming_refs: Vec<String>,
    pub editor_refs: Vec<String>,
    pub other_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DefinitionSideEffectV1 {
    pub domain: String,
    pub effect: String,
    pub target: String,
    pub metadata: BTreeMap<String, serde_json::Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelExplanationV1 {
    pub schema: String,
    pub source: String,
    pub model_ref: Option<String>,
    pub drawable_ref: Option<String>,
    pub material_bindings: Vec<MaterialBindingRef>,
    pub material_refs: Vec<String>,
    pub texture_refs: Vec<String>,
    pub uv_layout_refs: Vec<String>,
    pub physics_refs: Vec<String>,
    pub collision_refs: Vec<String>,
    pub render_options: MeshRenderOptions,
    pub collision_policy: String,
    pub uv_policy: String,
    pub physics_policy: String,
    pub lod_policy: String,
    pub streaming_policy: String,
    pub explanation: String,
}

impl Default for ModelExplanationV1 {
    fn default() -> Self {
        Self {
            schema: "newengine.ytyp.model_explanation.v1".to_owned(),
            source: String::new(),
            model_ref: None,
            drawable_ref: None,
            material_bindings: Vec::new(),
            material_refs: Vec::new(),
            texture_refs: Vec::new(),
            uv_layout_refs: Vec::new(),
            physics_refs: Vec::new(),
            collision_refs: Vec::new(),
            render_options: MeshRenderOptions::world_opaque(),
            collision_policy: "unspecified".to_owned(),
            uv_policy: "authored".to_owned(),
            physics_policy: "unspecified".to_owned(),
            lod_policy: "unspecified".to_owned(),
            streaming_policy: "unspecified".to_owned(),
            explanation: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionEntryV1 {
    pub schema: String,
    pub identity: DefinitionIdentityV1,
    pub kind: String,
    pub stable_hash: u64,
    pub semantic_tags: Vec<String>,
    pub domain_tags: Vec<String>,
    pub refs: DefinitionRefsV1,
    pub model_explanation: ModelExplanationV1,
    pub side_effects: Vec<DefinitionSideEffectV1>,
    pub arbitrary_metadata: BTreeMap<String, serde_json::Value>,
    pub warnings: Vec<String>,
}

impl Default for DefinitionEntryV1 {
    fn default() -> Self {
        Self {
            schema: "newengine.assets.definitions.entry.v1".to_owned(),
            identity: DefinitionIdentityV1::default(),
            kind: "archetype_definition".to_owned(),
            stable_hash: 0,
            semantic_tags: Vec::new(),
            domain_tags: Vec::new(),
            refs: DefinitionRefsV1::default(),
            model_explanation: ModelExplanationV1::default(),
            side_effects: Vec::new(),
            arbitrary_metadata: BTreeMap::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DefinitionManifestEntryV1 {
    pub name: String,
    pub stable_hash: u64,
    pub kind: String,
    pub semantic_tags: Vec<String>,
    pub domain_tags: Vec<String>,
    pub definition_ref: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DefinitionManifestV1 {
    pub schema: &'static str,
    pub gateway: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub source: String,
    pub status: &'static str,
    pub entries: Vec<DefinitionManifestEntryV1>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DefinitionRefResolutionV1 {
    pub ok: bool,
    pub gateway: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub definition_ref: String,
    pub refs: DefinitionRefsV1,
    pub flattened_refs: Vec<String>,
    pub resolver: &'static str,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DefinitionValidationV1 {
    pub ok: bool,
    pub gateway: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub definition_ref: String,
    pub code: &'static str,
    pub message: String,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct StableDiagnostic {
    pub(super) ok: bool,
    pub(super) code: &'static str,
    pub(super) message: String,
    pub(super) gateway: &'static str,
    pub(super) byte_owner: &'static str,
    pub(super) semantic_owner: &'static str,
}
