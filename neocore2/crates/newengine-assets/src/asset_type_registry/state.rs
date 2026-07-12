use super::*;

impl AssetTypeRegistryState {
    pub(super) fn service_info(&self) -> AssetTypesServiceInfo {
        AssetTypesServiceInfo {
            id: ASSET_TYPES_SERVICE_ID,
            gateway: ENGINE_ASSET_TYPES_SERVICE_ID,
            methods: ASSET_TYPES_SERVICE_METHODS,
            backend: "engine.assets.starvault.file-type-registry",
            registered_extensions: self.registry.keys().cloned().collect(),
        }
    }

    pub(super) fn manifest(&self) -> AssetFileTypeManifest {
        AssetFileTypeManifest {
            formats: self.registry.values().cloned().collect(),
            ..AssetFileTypeManifest::default()
        }
    }

    pub(super) fn register(
        &mut self,
        request: AssetFileTypeRegisterRequest,
    ) -> AssetFileTypeDescriptor {
        let mut desc = request.descriptor;
        desc.normalize_layer_contract();
        if let Err(e) = desc.validate_generic_rules() {
            let mut rejected = desc.clone();
            rejected.notes = format!("descriptor rejected by generic codec rules: {e}");
            return rejected;
        }
        warn_if_semantic_gateway_unresolved(&desc);
        let key = desc.extension.clone();
        let is_new_extension = !self.registry.contains_key(&key);
        let replace = self
            .registry
            .get(&key)
            .map(|prev| {
                desc.priority > prev.priority
                    || (desc.priority == prev.priority
                        && desc.handler_service < prev.handler_service)
            })
            .unwrap_or(true);
        if replace {
            self.registry.insert(key.clone(), desc.clone());
            if is_new_extension {
                self.extension_suffixes.push(key);
                self.extension_suffixes.sort_by(|left, right| {
                    right.len().cmp(&left.len()).then_with(|| left.cmp(right))
                });
            }
        }
        self.registry.get(&desc.extension).cloned().unwrap_or(desc)
    }

    pub(super) fn probe(&self, request: AssetFileTypeProbeRequest) -> AssetFileTypeProbeResult {
        let logical_path = normalize_logical_path(&request.logical_path);
        let extension = self
            .best_extension_match(&logical_path)
            .map(str::to_owned)
            .unwrap_or_else(|| path_extension(&logical_path));
        let descriptor = self.registry.get(&extension).cloned();
        AssetFileTypeProbeResult {
            logical_path,
            extension,
            known: descriptor.is_some(),
            descriptor,
        }
    }

    fn best_extension_match<'a>(&'a self, logical_path: &str) -> Option<&'a str> {
        let path = logical_path.split('@').next().unwrap_or(logical_path);
        self.extension_suffixes
            .iter()
            .map(String::as_str)
            .find(|extension| {
                path.strip_suffix(*extension)
                    .is_some_and(|prefix| prefix.ends_with('.'))
            })
    }

    pub(super) fn invoke_json(&mut self, payload: Blob) -> RResult<Blob, RString> {
        #[derive(Deserialize)]
        struct InvokeEnvelope {
            method: String,
            #[serde(default)]
            request: serde_json::Value,
        }

        let envelope = match serde_json::from_slice::<InvokeEnvelope>(payload.as_slice()) {
            Ok(envelope) => envelope,
            Err(e) => {
                return RResult::RErr(RString::from(format!(
                    "asset.file_types: invalid invoke_json payload: {e}"
                )))
            }
        };

        match envelope.method.as_str() {
            file_type_method::MANIFEST_JSON_V1 => ok_json(self.manifest()),
            file_type_method::REGISTER_JSON_V1 => {
                let request = match serde_json::from_value::<AssetFileTypeRegisterRequest>(
                    envelope.request,
                ) {
                    Ok(request) => request,
                    Err(e) => {
                        return RResult::RErr(RString::from(format!(
                            "asset.file_types: invalid register request: {e}"
                        )))
                    }
                };
                ok_json(self.register(request))
            }
            file_type_method::PROBE_JSON_V1 | file_type_method::RESOLVE_JSON_V1 => {
                let request =
                    match serde_json::from_value::<AssetFileTypeProbeRequest>(envelope.request) {
                        Ok(request) => request,
                        Err(e) => {
                            return RResult::RErr(RString::from(format!(
                                "asset.file_types: invalid probe request: {e}"
                            )))
                        }
                    };
                ok_json(self.probe(request))
            }
            other => RResult::RErr(RString::from(format!(
                "asset.file_types: unknown invoke method '{other}'"
            ))),
        }
    }
}

fn warn_if_semantic_gateway_unresolved(desc: &AssetFileTypeDescriptor) {
    if !newengine_service_api::is_engine_service_gateway_id(&desc.semantic_gateway) {
        newengine_ulog_api::ulog::warn!(
            "asset type registry: invalid semantic_gateway='{}' extension='.{}' asset_kind='{}'; descriptors must target an engine.* gateway",
            desc.semantic_gateway,
            desc.extension,
            desc.asset_kind
        );
        return;
    }

    let is_byte_bucket_only =
        desc.semantic_gateway == newengine_assets_api::ENGINE_ASSET_SERVICE_ID;
    if is_byte_bucket_only && !desc.is_container_codec() {
        newengine_ulog_api::ulog::warn!(
            "asset type registry: semantic_gateway fell back to engine.assets for non-container extension='.{}' asset_kind='{}'; this is a byte-owner only fallback and must be replaced by a real domain gateway",
            desc.extension,
            desc.asset_kind
        );
    }
}
