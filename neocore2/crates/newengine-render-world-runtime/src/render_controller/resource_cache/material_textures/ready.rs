impl RuntimeRenderController {
    pub(in crate::render_controller) fn material_texture_ready_state(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        path: &str,
        status_owner: &'static str,
    ) -> MaterialTextureReadyState {
        let frame_index = self.frame.frame_index;
        let texture = match self.gpu.material.textures.get_mut(path) {
            Some(MaterialTextureGpuResidency::Ready { texture }) => {
                return MaterialTextureReadyState::Ready(*texture)
            }
            Some(MaterialTextureGpuResidency::Failed { message }) => {
                let _ = message;
                return MaterialTextureReadyState::Failed;
            }
            Some(
                MaterialTextureGpuResidency::Requested
                | MaterialTextureGpuResidency::AssetLoading { .. }
                | MaterialTextureGpuResidency::CpuDecoding { .. },
            )
            | None => return MaterialTextureReadyState::Waiting,
            Some(MaterialTextureGpuResidency::GpuQueued {
                payload_bytes,
                requested_frame,
            }) => {
                let waited = frame_index.saturating_sub(*requested_frame);
                if waited > 120 && waited % 120 == 0 {
                    newengine_ulog_api::ulog::debug!(
                        "render controller: decoded texture waiting for GPU upload path='{}' bytes={} waited_frames={} queued_packets={} policy='priority + bytes/frame bounded upload queue'",
                        path,
                        payload_bytes,
                        waited,
                        self.gpu.material.texture_upload_queue.len(),
                    );
                }
                return MaterialTextureReadyState::Waiting;
            }
            Some(MaterialTextureGpuResidency::GpuLoading {
                texture,
                requested_frame,
                last_residency_poll_frame,
            }) => {
                let waited = frame_index.saturating_sub(*requested_frame);
                if waited > 180 && waited % 120 == 0 {
                    newengine_ulog_api::ulog::debug!(
                        "render controller: material texture still gpu-loading path='{}' waited_frames={}",
                        path,
                        waited,
                    );
                }
                if *last_residency_poll_frame == Some(frame_index) {
                    return MaterialTextureReadyState::Waiting;
                }
                *last_residency_poll_frame = Some(frame_index);
                *texture
            }
        };

        match r.texture_residency(texture) {
            Ok(snapshot) if snapshot.state == GpuResourceResidencyState::Ready => {
                self.gpu.material.textures.insert(
                    path.to_owned(),
                    MaterialTextureGpuResidency::Ready { texture },
                );
                let assets = AssetServiceClient::new(default_host_api());
                let _ = assets.project_status_json_v1(serde_json::json!({
                    "owner": status_owner,
                    "domain": "gpu",
                    "logical_path": path,
                    "stage": "resident",
                    "state": "ready",
                    "resource_id": format!("{:?}", texture),
                    "proof": {
                        "texture": format!("{:?}", texture),
                        "frame": self.frame.frame_index,
                        "residency": "ready"
                    },
                    "detail": "GPU texture residency confirmed by render controller"
                }));
                MaterialTextureReadyState::Ready(texture)
            }
            Ok(snapshot) if snapshot.state == GpuResourceResidencyState::Failed => {
                let message = snapshot
                    .message
                    .unwrap_or_else(|| "gpu upload failed".to_owned());
                newengine_ulog_api::ulog::warn!(
                    "render controller: material texture upload failed path='{}' err='{}'",
                    path,
                    message
                );
                self.gpu.material.textures.insert(
                    path.to_owned(),
                    MaterialTextureGpuResidency::Failed { message },
                );
                MaterialTextureReadyState::Failed
            }
            Err(err) => {
                let message = err.to_string();
                newengine_ulog_api::ulog::warn!(
                    "render controller: material texture residency query failed path='{}' err='{}'",
                    path,
                    message
                );
                self.gpu.material.textures.insert(
                    path.to_owned(),
                    MaterialTextureGpuResidency::Failed { message },
                );
                MaterialTextureReadyState::Failed
            }
            _ => MaterialTextureReadyState::Waiting,
        }
    }

    /// Resolve an authored material texture only when the real GPU resource is resident.
    ///
    /// World alpha cards must never be drawn against the generic white fallback: a
    /// pending leaf/grass atlas would turn every transparent texel into an opaque
    /// white quad. Opaque textured world meshes also use this path when callers
    /// prefer a one-frame omission over a camera-dependent white flash.
    pub(in crate::render_controller) fn material_texture_if_ready_with_priority(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        path: &str,
        status_owner: &'static str,
        priority: MaterialTexturePriority,
    ) -> Option<TextureId> {
        self.request_material_texture_with_priority(path, priority);
        match self.material_texture_ready_state(r, path, status_owner) {
            MaterialTextureReadyState::Ready(texture) => Some(texture),
            MaterialTextureReadyState::Waiting | MaterialTextureReadyState::Failed => None,
        }
    }

    pub(in crate::render_controller) fn material_texture_if_ready(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        path: &str,
        status_owner: &'static str,
    ) -> Option<TextureId> {
        self.material_texture_if_ready_with_priority(
            r,
            path,
            status_owner,
            MaterialTexturePriority::streaming_visible(),
        )
    }

    #[inline]
    pub(in crate::render_controller) fn material_texture_or_default_with_priority(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        path: Option<&str>,
        fallback: TextureId,
        priority: MaterialTexturePriority,
    ) -> TextureId {
        let Some(path) = path else {
            return fallback;
        };

        self.request_material_texture_with_priority(path, priority);
        match self.material_texture_ready_state(r, path, "render.controller") {
            MaterialTextureReadyState::Ready(texture) => texture,
            MaterialTextureReadyState::Waiting => {
                if let Some(MaterialTextureGpuResidency::CpuDecoding { requested_frame }) =
                    self.gpu.material.textures.get(path)
                {
                    let waited = self.frame.frame_index.saturating_sub(*requested_frame);
                    if waited > 180 && waited % 120 == 0 {
                        newengine_ulog_api::ulog::debug!(
                            "render controller: material texture still cpu-decoding path='{}' waited_frames={}",
                            path,
                            waited,
                        );
                    }
                } else if let Some(MaterialTextureGpuResidency::AssetLoading {
                    id_hex32,
                    requested_frame,
                }) = self.gpu.material.textures.get(path)
                {
                    let waited = self.frame.frame_index.saturating_sub(*requested_frame);
                    if waited > 180 && waited % 120 == 0 {
                        newengine_ulog_api::ulog::debug!(
                            "render controller: material texture still asset-loading path='{}' id='{}' waited_frames={}",
                            path,
                            id_hex32,
                            waited,
                        );
                    }
                }
                fallback
            }
            MaterialTextureReadyState::Failed => fallback,
        }
    }

    #[inline]
    pub(in crate::render_controller) fn material_texture_or_default(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        path: Option<&str>,
        fallback: TextureId,
    ) -> TextureId {
        self.material_texture_or_default_with_priority(
            r,
            path,
            fallback,
            MaterialTexturePriority::streaming_visible(),
        )
    }
}

