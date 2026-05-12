#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_assets::{AssetAccess, AssetServiceClient, AssetState};
use newengine_core::render::{
    Extent2D, GpuResourceResidencyState, RenderTargetId, SamplerId, TextureDesc, TextureFormat,
    TextureId, TextureUsage,
};
use newengine_core::{EngineError, EngineResult};
use newengine_plugin_host::default_host_api;
use std::num::NonZeroU32;

use super::controller::{PerDrawUbo, RuntimeRenderController};
use super::gpu::{LitPipeline, LIT_UBO_SIZE};
use super::material_bindings::MaterialTextureGpuResidency;

#[inline]
fn material_texture_mip_count(extent: Extent2D) -> NonZeroU32 {
    let max_dim = extent.width.max(extent.height).max(1);
    // Full mip chains are the first anti-aliasing primitive for textured terrain.
    // Without them, distant repeated albedo/roughness textures shimmer into the
    // grain pattern visible in runtime captures. Cap the chain to keep memory
    // bounded until the renderer grows streaming texture LOD residency.
    let levels = (32 - max_dim.leading_zeros()).clamp(
        1,
        super::render_quality::MATERIAL_TEXTURE_MAX_MIP_LEVELS,
    );
    NonZeroU32::new(levels).expect("clamped mip level count is non-zero")
}

fn material_texture_format(path: &str) -> TextureFormat {
    let lower = path.to_ascii_lowercase();
    if lower.contains("normal")
        || lower.contains("roughness")
        || lower.contains("metallic")
        || lower.contains("occlusion")
        || lower.contains("_ao")
    {
        TextureFormat::Rgba8Unorm
    } else {
        TextureFormat::Rgba8Srgb
    }
}

#[cfg(feature = "texture-decode")]
fn decode_material_texture_payload(
    path: &str,
    payload: &[u8],
) -> EngineResult<(Extent2D, Vec<u8>)> {
    let dyn_img = image::load_from_memory(payload)
        .map_err(|e| EngineError::other(format!("image decode failed path='{path}' err='{e}'")))?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok((Extent2D::new(w, h), rgba.into_raw()))
}

#[cfg(not(feature = "texture-decode"))]
fn decode_material_texture_payload(
    path: &str,
    _payload: &[u8],
) -> EngineResult<(Extent2D, Vec<u8>)> {
    Err(EngineError::other(format!(
        "texture decode requested in runtime-core for '{path}', but image decoding must go through AssetManager/imageImporter"
    )))
}

impl RuntimeRenderController {
    pub(super) fn request_material_texture(&mut self, path: &str) {
        if self.material_textures.contains_key(path) {
            return;
        }
        self.material_textures
            .insert(path.to_string(), MaterialTextureGpuResidency::Requested);
        self.material_texture_queue.push_back(path.to_string());
    }

    pub(super) fn pump_material_texture_requests(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        max_jobs: u32,
    ) {
        let max_jobs = max_jobs.max(1);
        let assets = AssetServiceClient::new(default_host_api());
        assets.pump();

        let mut started_jobs = 0_u32;
        while started_jobs < max_jobs {
            let Some(path) = self.material_texture_queue.pop_front() else {
                break;
            };

            if !matches!(
                self.material_textures.get(&path),
                Some(MaterialTextureGpuResidency::Requested)
            ) {
                continue;
            }

            match assets.load(&path) {
                Ok(id_hex32) => {
                    self.material_textures.insert(
                        path,
                        MaterialTextureGpuResidency::AssetLoading {
                            id_hex32,
                            requested_frame: self.frame_index,
                        },
                    );
                    started_jobs = started_jobs.saturating_add(1);
                }
                Err(e) => {
                    log::warn!(
                        "render controller: material texture asset request failed path='{}' err='{}'",
                        path,
                        e
                    );
                    self.material_textures.insert(
                        path,
                        MaterialTextureGpuResidency::Failed {
                            message: e.to_string(),
                        },
                    );
                }
            }
        }

        let decode_candidates = self
            .material_textures
            .iter()
            .filter_map(|(path, entry)| match entry {
                MaterialTextureGpuResidency::AssetLoading { id_hex32, .. } => {
                    Some((path.clone(), id_hex32.clone()))
                }
                _ => None,
            })
            .take(max_jobs as usize)
            .collect::<Vec<_>>();

        let mut decoded_jobs = 0_u32;
        for (path, id_hex32) in decode_candidates {
            if decoded_jobs >= max_jobs {
                break;
            }

            match assets.state(&id_hex32) {
                Ok(AssetState::Ready) => {
                    decoded_jobs = decoded_jobs.saturating_add(1);
                    let decoded = assets
                        .blob_wire_v1(&id_hex32)
                        .map_err(|e| EngineError::other(format!(
                            "asset.blob_wire_v1 failed path='{path}' id='{id_hex32}' err='{e}'"
                        )))
                        .and_then(|(_meta, payload)| {
                            decode_material_texture_payload(&path, &payload)
                        });

                    match decoded {
                        Ok((extent, rgba)) => match r.create_texture(
                            TextureDesc::new(
                                extent,
                                material_texture_format(&path),
                                TextureUsage::Sampled,
                            )
                            .with_label(format!("material_tex:{path}"))
                            .with_mips(material_texture_mip_count(extent))
                            .with_deferred_data(rgba),
                        ) {
                            Ok(texture) => {
                                self.material_textures.insert(
                                    path,
                                    MaterialTextureGpuResidency::GpuLoading {
                                        texture,
                                        requested_frame: self.frame_index,
                                    },
                                );
                            }
                            Err(e) => {
                                log::warn!(
                                    "render controller: material texture create failed path='{}' err='{}'",
                                    path,
                                    e
                                );
                                self.material_textures.insert(
                                    path,
                                    MaterialTextureGpuResidency::Failed {
                                        message: e.to_string(),
                                    },
                                );
                            }
                        },
                        Err(e) => {
                            log::warn!(
                                "render controller: material texture decode failed path='{}' err='{}'",
                                path,
                                e
                            );
                            self.material_textures.insert(
                                path,
                                MaterialTextureGpuResidency::Failed {
                                    message: e.to_string(),
                                },
                            );
                        }
                    }
                }
                Ok(AssetState::Failed) => {
                    decoded_jobs = decoded_jobs.saturating_add(1);
                    let message = format!("AssetManager reported failed state for id='{id_hex32}'");
                    log::warn!(
                        "render controller: material texture asset failed path='{}' err='{}'",
                        path,
                        message
                    );
                    self.material_textures.insert(
                        path,
                        MaterialTextureGpuResidency::Failed { message },
                    );
                }
                Ok(AssetState::Loading | AssetState::Unloaded | AssetState::Unknown) => {}
                Err(e) => {
                    let message = format!("asset state query failed id='{id_hex32}' err='{e}'");
                    log::warn!(
                        "render controller: material texture asset state failed path='{}' err='{}'",
                        path,
                        message
                    );
                    self.material_textures.insert(
                        path,
                        MaterialTextureGpuResidency::Failed { message },
                    );
                }
            }
        }
    }

    #[inline]
    pub(super) fn material_texture_or_default(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        path: Option<&str>,
        fallback: TextureId,
    ) -> TextureId {
        let Some(path) = path else {
            return fallback;
        };

        self.request_material_texture(path);

        let Some(entry) = self.material_textures.get(path).cloned() else {
            return fallback;
        };

        match entry {
            MaterialTextureGpuResidency::Ready { texture } => texture,
            MaterialTextureGpuResidency::GpuLoading {
                texture,
                requested_frame,
            } => match r.texture_residency(texture) {
                Ok(snapshot) if snapshot.state == GpuResourceResidencyState::Ready => {
                    self.material_textures.insert(
                        path.to_string(),
                        MaterialTextureGpuResidency::Ready { texture },
                    );
                    texture
                }
                Ok(snapshot) if snapshot.state == GpuResourceResidencyState::Failed => {
                    let message = snapshot
                        .message
                        .unwrap_or_else(|| "gpu upload failed".to_string());
                    log::warn!(
                        "render controller: material texture upload failed path='{}' err='{}'",
                        path,
                        message
                    );
                    self.material_textures.insert(
                        path.to_string(),
                        MaterialTextureGpuResidency::Failed { message },
                    );
                    fallback
                }
                _ => {
                    let _ = requested_frame;
                    fallback
                }
            },
            MaterialTextureGpuResidency::Requested => fallback,
            MaterialTextureGpuResidency::AssetLoading { requested_frame, .. } => {
                let waited = self.frame_index.saturating_sub(requested_frame);
                if waited > 180 && waited % 120 == 0 {
                    log::debug!(
                        "render controller: material texture still asset-loading path='{}' waited_frames={}",
                        path,
                        waited,
                    );
                }
                fallback
            }
            MaterialTextureGpuResidency::Failed { message } => {
                let _ = message;
                fallback
            }
        }
    }


    pub(super) fn ensure_per_draw_ubo(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        lit: LitPipeline,
        key: u64,
    ) -> newengine_core::EngineResult<PerDrawUbo> {
        self.ensure_per_draw_ubo_with_binding(
            r,
            lit,
            key,
            lit.white_texture,
            lit.flat_normal_texture,
            lit.white_texture,
            lit.white_texture,
            lit.clamp_sampler,
        )
    }

    pub(super) fn ensure_per_draw_ubo_with_binding(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        lit: LitPipeline,
        key: u64,
        base_texture: TextureId,
        normal_texture: TextureId,
        roughness_texture: TextureId,
        shadow_texture: TextureId,
        sampler: SamplerId,
    ) -> newengine_core::EngineResult<PerDrawUbo> {
        if let Some(mut e) = self.per_draw_ubo.get(&key).copied() {
            if e.base_texture == base_texture
                && e.normal_texture == normal_texture
                && e.roughness_texture == roughness_texture
                && e.shadow_texture == shadow_texture
                && e.sampler == sampler
            {
                return Ok(e);
            }
            r.destroy_bind_group(e.bg);
            let bg = r.create_bind_group(
                newengine_core::render::BindGroupDesc::new(lit.bgl)
                    .with_label("editor_lit_entity_bg")
                    .with_uniform0(newengine_core::render::BufferBinding::new(e.ubo, 0, LIT_UBO_SIZE))
                    .with_texture0(base_texture)
                    .with_texture1(normal_texture)
                    .with_texture2(roughness_texture)
                    .with_texture3(shadow_texture)
                    .with_sampler0(sampler),
            )?;
            e.bg = bg;
            e.base_texture = base_texture;
            e.normal_texture = normal_texture;
            e.roughness_texture = roughness_texture;
            e.shadow_texture = shadow_texture;
            e.sampler = sampler;
            self.per_draw_ubo.insert(key, e);
            return Ok(e);
        }

        let ubo = r.create_buffer(
            newengine_core::render::BufferDesc::new(
                LIT_UBO_SIZE,
                newengine_core::render::BufferUsage::Uniform,
                newengine_core::render::MemoryHint::CpuToGpu,
            )
            .with_label("editor_lit_entity_ubo"),
        )?;

        let bg = r.create_bind_group(
            newengine_core::render::BindGroupDesc::new(lit.bgl)
                .with_label("editor_lit_entity_bg")
                .with_uniform0(newengine_core::render::BufferBinding::new(ubo, 0, LIT_UBO_SIZE))
                .with_texture0(base_texture)
                .with_texture1(normal_texture)
                .with_texture2(roughness_texture)
                .with_texture3(shadow_texture)
                .with_sampler0(sampler),
        )?;

        let entry = PerDrawUbo {
            ubo,
            bg,
            base_texture,
            normal_texture,
            roughness_texture,
            shadow_texture,
            sampler,
            last_seen_frame: self.frame_index,
        };
        self.per_draw_ubo.insert(key, entry);
        Ok(entry)
    }

    pub(super) fn gc_per_draw_ubos(&mut self, r: &mut dyn newengine_core::render::RenderApi) {
        let now = self.frame_index;
        let grace = 8_u64;

        let mut dead: Vec<u64> = Vec::new();
        for (k, v) in &self.per_draw_ubo {
            if now.saturating_sub(v.last_seen_frame) > grace {
                dead.push(*k);
            }
        }
        for k in dead {
            if let Some(v) = self.per_draw_ubo.remove(&k) {
                r.destroy_bind_group(v.bg);
                r.destroy_buffer(v.ubo);
            }
        }
    }

    pub(super) fn retire_render_target(&mut self, rt: RenderTargetId) {
        self.render_target_lifetimes
            .retire_after_frames(rt, self.frame_index, 8);
    }

    pub(super) fn gc_deferred_rts(&mut self, r: &mut dyn newengine_core::render::RenderApi) {
        self.render_target_lifetimes.collect(r, self.frame_index);
    }
}
