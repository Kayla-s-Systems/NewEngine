use super::geometry::{
    asset_extension, material_texture_refs, require_complete_graph, scene_preview_snapshot,
    texture_dimensions,
};
use super::*;

impl AssetPreviewApi {
    pub fn request(
        &self,
        document: &AssetDocument,
        width: u32,
        height: u32,
    ) -> AssetPreviewSnapshot {
        self.last_request_cache_hit.store(false, Ordering::Release);
        let width = width.max(1);
        let height = height.max(1);
        let result = if self.is_texture(document) {
            self.request_texture(document)
        } else if self.is_model(document) {
            self.request_model(document, width, height)
        } else if self.is_material(document) {
            self.request_material(document, width, height)
        } else {
            self.clear_render_bundle();
            AssetPreviewSnapshot::unavailable(
                document.asset_ref.clone(),
                format!(
                    "asset type '{}' has no visual runtime representation",
                    document.asset_kind
                ),
            )
        };
        *self.current.lock() = result.clone();
        result
    }

    fn request_texture(&self, document: &AssetDocument) -> AssetPreviewSnapshot {
        self.clear_render_bundle();
        if let Some(snapshot) = self.cached_texture(&document.asset_ref) {
            return snapshot;
        }
        let (texture_ref, entry_metadata) = if document.asset_ref.contains('@') {
            (document.asset_ref.clone(), None)
        } else {
            match self.first_container_entry(&document.asset_ref) {
                Ok(Some(entry)) => (entry.entry_ref, Some(entry.metadata)),
                Ok(None) => {
                    return AssetPreviewSnapshot::unavailable(
                        document.asset_ref.clone(),
                        "texture dictionary contains no entries",
                    )
                }
                Err(error) => {
                    return AssetPreviewSnapshot::unavailable(document.asset_ref.clone(), error)
                }
            }
        };
        let (width, height) = texture_dimensions(document, entry_metadata.as_ref());
        let snapshot = AssetPreviewSnapshot {
            asset_ref: document.asset_ref.clone(),
            kind: AssetPreviewKind::Texture2d,
            ready: true,
            texture_ref: Some(texture_ref.clone()),
            ui_texture_id: None,
            width,
            height,
            diagnostic: None,
        };
        self.cache_texture(&snapshot);
        newengine_ulog_api::ulog::info!(
            "asset preview: 2D texture reference resolved asset_ref='{}' texture_ref='{}' extent={}x{} decode_owner='engine.ui.aurelia' duplicate_decode_avoided=true",
            document.asset_ref,
            texture_ref,
            width,
            height
        );
        snapshot
    }

    fn request_model(
        &self,
        document: &AssetDocument,
        width: u32,
        height: u32,
    ) -> AssetPreviewSnapshot {
        if self.activate_cached_bundle(&document.asset_ref) {
            self.viewport.publish_external_extent(width, height);
            return scene_preview_snapshot(&document.asset_ref, width, height);
        }
        let dependency_graph = match self.resolve_dependency_graph(&document.asset_ref) {
            Ok(graph) => graph,
            Err(error) => {
                self.clear_render_bundle();
                return AssetPreviewSnapshot::unavailable(
                    document.asset_ref.clone(),
                    format!("engine.assets.graph preview failed: {error}"),
                );
            }
        };
        if let Err(error) = require_complete_graph(&dependency_graph) {
            self.clear_render_bundle();
            return AssetPreviewSnapshot::unavailable(document.asset_ref.clone(), error);
        }
        let mut request = if document.semantic_gateway == "engine.assets.definitions"
            || document.asset_kind.contains("archetype")
        {
            ModelAssetRequest {
                properties_ref: Some(document.asset_ref.clone()),
                ..ModelAssetRequest::default()
            }
        } else {
            ModelAssetRequest::new(document.asset_ref.clone())
        };
        request.dependency_graph = Some(dependency_graph);
        match self.models.assemble_bundle(&request) {
            Ok(bundle) => {
                newengine_ulog_api::ulog::info!(
                    "asset preview: 3D bundle resolved asset_ref='{}' parts={} graph_nodes={} graph_edges={} materials={} textures={} uv_refs={} physics_refs={} streaming_refs={} lod_policy='{}' render_role={:?}",
                    document.asset_ref,
                    bundle.parts.len(),
                    bundle.dependency_graph.nodes.len(),
                    bundle.dependency_graph.edges.len(),
                    bundle.configuration.material_refs.len(),
                    bundle.configuration.texture_refs.len(),
                    bundle.configuration.uv_layout_refs.len(),
                    bundle.configuration.physics_refs.len(),
                    bundle.configuration.streaming_refs.len(),
                    bundle.configuration.lod_policy,
                    bundle.configuration.render_options.role
                );
                self.set_render_bundle(&document.asset_ref, bundle);
                self.viewport.publish_external_extent(width, height);
                scene_preview_snapshot(&document.asset_ref, width, height)
            }
            Err(error) => {
                self.clear_render_bundle();
                AssetPreviewSnapshot::unavailable(
                    document.asset_ref.clone(),
                    format!("engine.assets.models preview failed: {error}"),
                )
            }
        }
    }

    fn request_material(
        &self,
        document: &AssetDocument,
        width: u32,
        height: u32,
    ) -> AssetPreviewSnapshot {
        if self.activate_cached_bundle(&document.asset_ref) {
            self.viewport.publish_external_extent(width, height);
            return scene_preview_snapshot(&document.asset_ref, width, height);
        }
        let material_ref = if document.asset_ref.contains('@') {
            document.asset_ref.clone()
        } else {
            match self.materials.preview_material_ref(&document.asset_ref) {
                Ok(entry_ref) => entry_ref,
                Err(error) => {
                    self.clear_render_bundle();
                    newengine_ulog_api::ulog::warn!(
                        "asset preview: material selector resolve failed asset_ref='{}' err='{}'",
                        document.asset_ref,
                        error
                    );
                    return AssetPreviewSnapshot::unavailable(document.asset_ref.clone(), error);
                }
            }
        };
        let request = MaterialLoadRequest {
            logical_path: material_ref.clone(),
            selector: None,
        };
        match self
            .materials
            .load_descriptor(&request)
            .and_then(|response| {
                let binding = ModelMaterialBinding {
                    slot: response.name.clone(),
                    material_ref: Some(document.asset_ref.clone()),
                    descriptor: response.descriptor,
                    textures: response.textures,
                    fallback_color: response.descriptor.base_color,
                    resolution_policy: "engine.assets.preview".to_owned(),
                };
                let mesh = PrimitiveRegistry::with_builtins()
                    .build_mesh(builtins::ID_SPHERE_UV)
                    .map_err(|error| error.to_string())?;
                let dependency_graph = self.resolve_dependency_graph(&material_ref)?;
                require_complete_graph(&dependency_graph)?;
                let texture_refs = material_texture_refs(&binding);
                Ok(ModelAssetBundle {
                    source: document.asset_ref.clone(),
                    properties_ref: None,
                    parts: vec![ModelMeshPart {
                        // Synthetic material-preview sphere has no source container mesh identity.
                        source_mesh_name: String::new(),
                        material_slot: binding.slot.clone(),
                        mesh,
                        skin: None,
                        material: binding,
                    }],
                    skeleton: None,
                    texture_dictionary: texture_refs.first().map(|reference| {
                        reference.split('@').next().unwrap_or(reference).to_owned()
                    }),
                    collisions: Vec::new(),
                    configuration: ModelRuntimeConfiguration {
                        material_refs: vec![document.asset_ref.clone()],
                        texture_refs,
                        ..ModelRuntimeConfiguration::default()
                    },
                    dependency_graph,
                })
            }) {
            Ok(bundle) => {
                newengine_ulog_api::ulog::info!(
                    "asset preview: material sphere bundle resolved asset_ref='{}' material_ref='{}' graph_nodes={} graph_edges={} texture_refs={}",
                    document.asset_ref,
                    material_ref,
                    bundle.dependency_graph.nodes.len(),
                    bundle.dependency_graph.edges.len(),
                    bundle.configuration.texture_refs.len()
                );
                self.set_render_bundle(&document.asset_ref, bundle);
                self.viewport.publish_external_extent(width, height);
                scene_preview_snapshot(&document.asset_ref, width, height)
            }
            Err(error) => {
                self.clear_render_bundle();
                newengine_ulog_api::ulog::warn!(
                    "asset preview: material preview failed asset_ref='{}' material_ref='{}' err='{}'",
                    document.asset_ref,
                    material_ref,
                    error
                );
                AssetPreviewSnapshot::unavailable(
                    document.asset_ref.clone(),
                    format!("engine.assets.materials preview failed: {error}"),
                )
            }
        }
    }

    fn resolve_dependency_graph(&self, root_ref: &str) -> Result<ResolvedAssetGraphV2, String> {
        let payload = serde_json::to_vec(&AssetGraphResolveRequest {
            root_ref: root_ref.to_owned(),
        })
        .map_err(|error| error.to_string())?;
        let bytes = (self.host.call_service_v1)(
            RString::from(ENGINE_ASSETS_GRAPH_SERVICE_ID),
            MethodName::from(ASSET_GRAPH_METHOD_RESOLVE_V1),
            Blob::from(payload),
        )
        .into_result()
        .map(|value| value.into_vec())
        .map_err(|error| error.to_string())?;
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("engine.assets.graph returned invalid preview graph: {error}"))
    }

    fn first_container_entry(
        &self,
        logical_path: &str,
    ) -> Result<Option<AssetEntryManifest>, String> {
        let bytes = self.assets.decode_v1(&AssetDecodeRequest {
            logical_path: logical_path.to_owned(),
            output_kind: ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
            selector: serde_json::Value::Null,
            format_descriptor: None,
        })?;
        let manifest = serde_json::from_slice::<AssetFileManifest>(&bytes)
            .map_err(|error| format!("invalid asset manifest: {error}"))?;
        Ok(manifest.entries.into_iter().next())
    }

    pub(super) fn is_texture(&self, document: &AssetDocument) -> bool {
        document
            .semantic_gateway
            .eq_ignore_ascii_case("engine.assets.textures")
            || document.asset_kind.contains("texture")
            || asset_extension(&document.asset_ref).eq_ignore_ascii_case("ytd")
    }

    pub(super) fn is_model(&self, document: &AssetDocument) -> bool {
        document
            .semantic_gateway
            .eq_ignore_ascii_case("engine.model")
            || document
                .semantic_gateway
                .eq_ignore_ascii_case("engine.assets.models")
            || document
                .semantic_gateway
                .eq_ignore_ascii_case("engine.assets.definitions")
            || document.asset_kind.contains("drawable")
            || document.asset_kind.contains("model")
            || document.asset_kind.contains("archetype")
            || matches!(
                asset_extension(&document.asset_ref)
                    .to_ascii_lowercase()
                    .as_str(),
                "ydd" | "ydr"
            )
    }

    pub(super) fn is_material(&self, document: &AssetDocument) -> bool {
        document
            .semantic_gateway
            .eq_ignore_ascii_case("engine.materials")
            || document
                .semantic_gateway
                .eq_ignore_ascii_case("engine.assets.materials")
            || document.asset_kind.contains("material")
            || document.content_kind == Some(newengine_assets_api::LIST_FILE_CONTENT_KIND_NEMAT)
            || asset_extension(&document.asset_ref).eq_ignore_ascii_case("nemat")
    }
}
