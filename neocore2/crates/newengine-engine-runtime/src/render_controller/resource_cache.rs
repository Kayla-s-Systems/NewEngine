#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_assets::{AssetAccess, AssetErrorKind, AssetServiceClient, RuntimeTextureFormat};
use newengine_core::render::{
    Extent2D, GpuResourceResidencyState, RenderTargetId, SamplerId, TextureDesc, TextureFormat,
    TextureId, TextureMipDataDesc, TextureUsage,
};
use newengine_plugin_host::default_host_api;
use std::num::NonZeroU32;

use super::controller::RuntimeRenderController;
pub use super::state::PerDrawUbo;
use super::gpu::{LitPipeline, LIT_UBO_SIZE};
use super::material_bindings::MaterialTextureGpuResidency;

fn render_texture_format_from_runtime(format: RuntimeTextureFormat) -> TextureFormat {
    match format {
        RuntimeTextureFormat::Rgba8Unorm => TextureFormat::Rgba8Unorm,
        RuntimeTextureFormat::Rgba8Srgb => TextureFormat::Rgba8Srgb,
        RuntimeTextureFormat::Bc1RgbaUnorm => TextureFormat::Bc1RgbaUnorm,
        RuntimeTextureFormat::Bc1RgbaSrgb => TextureFormat::Bc1RgbaSrgb,
        RuntimeTextureFormat::Bc3RgbaUnorm => TextureFormat::Bc3RgbaUnorm,
        RuntimeTextureFormat::Bc3RgbaSrgb => TextureFormat::Bc3RgbaSrgb,
        RuntimeTextureFormat::Bc5RgUnorm => TextureFormat::Bc5RgUnorm,
        RuntimeTextureFormat::Bc7RgbaUnorm => TextureFormat::Bc7RgbaUnorm,
        RuntimeTextureFormat::Bc7RgbaSrgb => TextureFormat::Bc7RgbaSrgb,
    }
}


impl RuntimeRenderController {

    fn request_dictionary_material_texture(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        assets: &AssetServiceClient,
        path: String,
    ) {
        let texture_asset = match assets.textures_entry_runtime_ref_v1_typed(&path) {
            Ok(texture_asset) => texture_asset,
            Err(e) if e.kind == AssetErrorKind::NotReady => {
                log::debug!(
                    "render controller: material texture packet pending path='{}' method='assets.textures.entry_runtime_v1' err='{}'",
                    path,
                    e
                );
                self.gpu.material.textures.insert(
                    path,
                    MaterialTextureGpuResidency::AssetLoading {
                        id_hex32: e.id_hex32.unwrap_or_default(),
                        requested_frame: self.frame.frame_index,
                    },
                );
                return;
            }
            Err(e) => {
                let message = format!("assets.textures.entry_runtime_v1 failed err='{e}'");
                let line = format!(
                    "render controller: material texture packet lookup failed path='{}' method='assets.textures.entry_runtime_v1' kind='{}' err='{}'",
                    path,
                    e.kind,
                    e,
                );
                match e.kind {
                    AssetErrorKind::DecodeFailed | AssetErrorKind::UnsupportedFormat => log::debug!("{}", line),
                    _ => log::warn!("{}", line),
                }
                self.gpu.material.textures.insert(path, MaterialTextureGpuResidency::Failed { message });
                return;
            }
        };

        let extent = Extent2D::new(texture_asset.width, texture_asset.height);
        let mip_levels = NonZeroU32::new(texture_asset.mips.len().max(1) as u32)
            .expect("runtime texture mip count is non-zero");
        let (payload, layout) = texture_asset.concatenated_payload_and_layout();
        let mip_data: Vec<TextureMipDataDesc> = layout
            .into_iter()
            .map(|mip| TextureMipDataDesc::new(mip.level, mip.width, mip.height, mip.offset, mip.byte_len))
            .collect();

        match r.create_texture(
            TextureDesc::new(
                extent,
                render_texture_format_from_runtime(texture_asset.format),
                TextureUsage::Sampled,
            )
            .with_label(format!("material_tex:{path}"))
            .with_mips(mip_levels)
            .with_deferred_mip_data(mip_data, payload),
        ) {
            Ok(texture) => {
                log::debug!(
                    "render controller: material texture packet upload queued path='{}' method='assets.textures.entry_runtime_v1' texture={:?} frame={}",
                    path,
                    texture,
                    self.frame.frame_index
                );
                self.gpu.material.textures.insert(
                    path,
                    MaterialTextureGpuResidency::GpuLoading {
                        texture,
                        requested_frame: self.frame.frame_index,
                    },
                );
            }
            Err(e) => {
                let message = e.to_string();
                log::warn!(
                    "render controller: material texture create failed path='{}' err='{}'",
                    path,
                    message
                );
                self.gpu.material.textures.insert(path, MaterialTextureGpuResidency::Failed { message });
            }
        }
    }

    pub(super) fn request_material_texture(&mut self, path: &str) {
        if self.gpu.material.textures.contains_key(path) {
            return;
        }
        self.gpu.material.textures
            .insert(path.to_string(), MaterialTextureGpuResidency::Requested);
        self.gpu.material.texture_queue.push_back(path.to_string());
    }

    pub(super) fn pump_material_texture_requests(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        max_start_jobs: u32,
        max_decode_jobs: u32,
    ) {
        let max_start_jobs = max_start_jobs.max(1);
        let max_decode_jobs = max_decode_jobs.max(1);
        let assets = AssetServiceClient::new(default_host_api());
        assets.pump();

        let loading_retry_paths = self
            .gpu
            .material
            .textures
            .iter()
            .filter_map(|(path, state)| match state {
                MaterialTextureGpuResidency::AssetLoading { requested_frame, .. }
                    if self.frame.frame_index > *requested_frame => Some(path.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();

        for path in loading_retry_paths {
            if !self.gpu.material.texture_queue.contains(&path) {
                self.gpu.material.textures
                    .insert(path.clone(), MaterialTextureGpuResidency::Requested);
                self.gpu.material.texture_queue.push_back(path);
            }
        }

        let mut started_jobs = 0_u32;
        while started_jobs < max_start_jobs {
            let Some(path) = self.gpu.material.texture_queue.pop_front() else {
                break;
            };

            if !matches!(
                self.gpu.material.textures.get(&path),
                Some(MaterialTextureGpuResidency::Requested)
            ) {
                continue;
            }

            self.request_dictionary_material_texture(r, &assets, path);
            started_jobs = started_jobs.saturating_add(1);
        }

        let _ = max_decode_jobs;
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

        let Some(entry) = self.gpu.material.textures.get(path).cloned() else {
            return fallback;
        };

        match entry {
            MaterialTextureGpuResidency::Ready { texture } => texture,
            MaterialTextureGpuResidency::GpuLoading {
                texture,
                requested_frame,
            } => match r.texture_residency(texture) {
                Ok(snapshot) if snapshot.state == GpuResourceResidencyState::Ready => {
                    self.gpu.material.textures.insert(
                        path.to_string(),
                        MaterialTextureGpuResidency::Ready { texture },
                    );
                    let assets = AssetServiceClient::new(default_host_api());
                    let _ = assets.project_status_json_v1(serde_json::json!({
                        "owner": "render.controller",
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
                    log::debug!(
                        "render controller: asset status gpu resident path='{}' texture={:?} frame={}",
                        path,
                        texture,
                        self.frame.frame_index
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
                    self.gpu.material.textures.insert(
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
            MaterialTextureGpuResidency::AssetLoading { id_hex32, requested_frame } => {
                let waited = self.frame.frame_index.saturating_sub(requested_frame);
                if waited > 180 && waited % 120 == 0 {
                    log::debug!(
                        "render controller: material texture still asset-loading path='{}' id='{}' waited_frames={}",
                        path,
                        id_hex32,
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
        if let Some(mut e) = self.gpu.material.per_draw_ubo.get(&key).copied() {
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
                    .with_label("material_lit_entity_bg")
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
            self.gpu.material.per_draw_ubo.insert(key, e);
            return Ok(e);
        }

        let ubo = r.create_buffer(
            newengine_core::render::BufferDesc::new(
                LIT_UBO_SIZE,
                newengine_core::render::BufferUsage::Uniform,
                newengine_core::render::MemoryHint::CpuToGpu,
            )
            .with_label("material_lit_entity_ubo"),
        )?;

        let bg = r.create_bind_group(
            newengine_core::render::BindGroupDesc::new(lit.bgl)
                .with_label("material_lit_entity_bg")
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
            last_seen_frame: self.frame.frame_index,
        };
        self.gpu.material.per_draw_ubo.insert(key, entry);
        Ok(entry)
    }

    pub(super) fn gc_per_draw_ubos(&mut self, r: &mut dyn newengine_core::render::RenderApi) {
        let now = self.frame.frame_index;
        let grace = 8_u64;

        let mut dead: Vec<u64> = Vec::new();
        for (k, v) in &self.gpu.material.per_draw_ubo {
            if now.saturating_sub(v.last_seen_frame) > grace {
                dead.push(*k);
            }
        }
        for k in dead {
            if let Some(v) = self.gpu.material.per_draw_ubo.remove(&k) {
                r.destroy_bind_group(v.bg);
                r.destroy_buffer(v.ubo);
            }
        }
    }

    pub(super) fn retire_render_target(&mut self, rt: RenderTargetId) {
        self.gpu.lifetimes.render_target_lifetimes
            .retire_after_frames(rt, self.frame.frame_index, 8);
    }

    pub(super) fn gc_deferred_rts(&mut self, r: &mut dyn newengine_core::render::RenderApi) {
        self.gpu.lifetimes.render_target_lifetimes.collect(r, self.frame.frame_index);
    }
}
