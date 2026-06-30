#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_assets::{
    AssetErrorKind, AssetServiceClient, RuntimeTextureAsset, RuntimeTextureFormat,
};
use newengine_core::render::{
    Extent2D, GpuResourceResidencyState, RenderTargetId, SamplerId, TextureDesc, TextureFormat,
    TextureId, TextureMipDataDesc, TextureUsage,
};
use newengine_core::{TaskLane, TaskPriority, TaskRequest, ThreadPoolHandle};
use newengine_plugin_host::default_host_api;
use newengine_task_api::{task_domain, task_pass};
use parking_lot::Mutex;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;

use super::controller::RuntimeRenderController;
use super::gpu::{LitPipeline, LIT_UBO_SIZE};
use super::material_bindings::MaterialTextureGpuResidency;
use super::state::MaterialTextureDecodeJob;
pub use super::state::PerDrawUbo;

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

fn sanitize_material_texture_task_id(path: &str) -> String {
    let mut out = String::with_capacity(path.len().min(96));
    for ch in path.chars().take(96) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("unknown");
    }
    out
}

impl RuntimeRenderController {
    fn queue_dictionary_material_texture_decode(
        &mut self,
        path: String,
        thread_pool: Option<&ThreadPoolHandle>,
    ) {
        if self.gpu.material.texture_decode_jobs.contains_key(&path) {
            self.gpu.material.textures.insert(
                path,
                MaterialTextureGpuResidency::CpuDecoding {
                    requested_frame: self.frame.frame_index,
                },
            );
            return;
        }

        let Some(thread_pool) = thread_pool else {
            let message = "engine.threading unavailable for material texture decode".to_owned();
            newengine_ulog_api::ulog::warn!(
                "render controller: material texture decode skipped path='{}' err='{}'",
                path,
                message
            );
            self.gpu
                .material
                .textures
                .insert(path, MaterialTextureGpuResidency::Failed { message });
            return;
        };

        let worker_path = path.clone();
        let result = Arc::new(Mutex::new(None));
        let result_out = Arc::clone(&result);
        let task_path = sanitize_material_texture_task_id(&path);
        let request = TaskRequest::new("material.texture.decode")
            .with_source("render.controller")
            .with_owner("engine.render")
            .with_category("asset-decode")
            .with_lane(TaskLane::AssetIo)
            .with_priority(TaskPriority::Interactive)
            .with_frame_id(self.frame.frame_index)
            .with_dependency_group(format!(
                "frame.{}.asset-io.texture-decode",
                self.frame.frame_index
            ))
            .with_task_domain(task_domain::ENGINE_ASSETS)
            .with_task_pass(task_pass::TEXTURE_DECODE)
            .with_task_id(format!("render.material.texture.decode.{task_path}"));
        let ticket = thread_pool.submit_request(request, move || {
            let assets = AssetServiceClient::new(default_host_api());
            let decoded = assets.textures_entry_runtime_ref_v1_typed(&worker_path);
            *result_out.lock() = Some(decoded);
        });

        self.gpu
            .material
            .texture_decode_jobs
            .insert(path.clone(), MaterialTextureDecodeJob { ticket, result });
        self.gpu.material.textures.insert(
            path,
            MaterialTextureGpuResidency::CpuDecoding {
                requested_frame: self.frame.frame_index,
            },
        );
    }

    fn upload_decoded_material_texture(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        path: String,
        texture_asset: RuntimeTextureAsset,
    ) {
        let extent = Extent2D::new(texture_asset.width, texture_asset.height);
        let mip_levels = NonZeroU32::new(texture_asset.mips.len().max(1) as u32)
            .expect("runtime texture mip count is non-zero");
        let (payload, layout) = texture_asset.concatenated_payload_and_layout();
        let mip_data: Vec<TextureMipDataDesc> = layout
            .into_iter()
            .map(|mip| {
                TextureMipDataDesc::new(mip.level, mip.width, mip.height, mip.offset, mip.byte_len)
            })
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
                newengine_ulog_api::ulog::debug!(
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
                newengine_ulog_api::ulog::warn!(
                    "render controller: material texture create failed path='{}' err='{}'",
                    path,
                    message
                );
                self.gpu
                    .material
                    .textures
                    .insert(path, MaterialTextureGpuResidency::Failed { message });
            }
        }
    }

    fn poll_material_texture_decode_jobs(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        max_completions: u32,
    ) {
        let max_completions = max_completions.max(1) as usize;
        let ready_paths = self
            .gpu
            .material
            .texture_decode_jobs
            .iter()
            .filter_map(|(path, job)| {
                if job.is_complete() {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .take(max_completions)
            .collect::<Vec<_>>();

        for path in ready_paths {
            let Some(job) = self.gpu.material.texture_decode_jobs.remove(&path) else {
                continue;
            };

            let Some(result) = job.take_result() else {
                let message = "material texture decode job completed without result".to_owned();
                newengine_ulog_api::ulog::warn!(
                    "render controller: material texture decode job completed without result path='{}'",
                    path
                );
                self.gpu
                    .material
                    .textures
                    .insert(path, MaterialTextureGpuResidency::Failed { message });
                continue;
            };

            match result {
                Ok(texture_asset) => {
                    self.upload_decoded_material_texture(r, path, texture_asset);
                }
                Err(e) if e.kind == AssetErrorKind::NotReady => {
                    if let Some(id_hex32) = e.id_hex32.clone() {
                        self.gpu.material.textures.insert(
                            path.clone(),
                            MaterialTextureGpuResidency::AssetLoading {
                                id_hex32,
                                requested_frame: self.frame.frame_index,
                            },
                        );
                    } else {
                        self.gpu
                            .material
                            .textures
                            .insert(path.clone(), MaterialTextureGpuResidency::Requested);
                    }
                    if !self.gpu.material.texture_queue.contains(&path) {
                        self.gpu.material.texture_queue.push_back(path);
                    }
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
                        AssetErrorKind::DecodeFailed | AssetErrorKind::UnsupportedFormat => {
                            newengine_ulog_api::ulog::debug!("{}", line)
                        }
                        _ => newengine_ulog_api::ulog::warn!("{}", line),
                    }
                    self.gpu
                        .material
                        .textures
                        .insert(path, MaterialTextureGpuResidency::Failed { message });
                }
            }
        }
    }

    pub(super) fn request_material_texture(&mut self, path: &str) {
        if self.gpu.material.textures.contains_key(path) {
            return;
        }
        self.gpu
            .material
            .textures
            .insert(path.to_string(), MaterialTextureGpuResidency::Requested);
        self.gpu.material.texture_queue.push_back(path.to_string());
    }

    pub(super) fn pump_material_texture_requests(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        thread_pool: Option<&ThreadPoolHandle>,
        max_start_jobs: u32,
        max_decode_jobs: u32,
    ) {
        let max_start_jobs = max_start_jobs.max(1);
        let max_decode_jobs = max_decode_jobs
            .max(1)
            .min(super::render_quality::MATERIAL_TEXTURE_MAX_ASYNC_DECODE_JOBS as u32);
        let max_jobs_this_pump = max_start_jobs.min(max_decode_jobs).max(1);
        let pump_started = Instant::now();
        let decode_budget_ms =
            super::render_quality::MATERIAL_TEXTURE_DECODE_PUMP_BUDGET_MS.max(0.25);
        // Progress AssetManager's own background queue, then harvest any render-owned
        // CPU decode jobs that completed on worker threads. This function must stay
        // bounded: it may start/poll work, but must never synchronously decode a .ytd.
        self.poll_material_texture_decode_jobs(r, max_decode_jobs);

        let loading_retry_paths = self
            .gpu
            .material
            .textures
            .iter()
            .filter_map(|(path, state)| match state {
                MaterialTextureGpuResidency::AssetLoading {
                    requested_frame, ..
                } if self.frame.frame_index > *requested_frame => Some(path.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();

        for path in loading_retry_paths {
            if !self.gpu.material.texture_queue.contains(&path) {
                self.gpu
                    .material
                    .textures
                    .insert(path.clone(), MaterialTextureGpuResidency::Requested);
                self.gpu.material.texture_queue.push_back(path);
            }
        }

        let mut started_jobs = 0_u32;
        while started_jobs < max_jobs_this_pump {
            let active_jobs = self.gpu.material.texture_decode_jobs.len() as u32;
            if active_jobs >= max_decode_jobs {
                break;
            }

            let Some(path) = self.gpu.material.texture_queue.pop_front() else {
                break;
            };

            if !matches!(
                self.gpu.material.textures.get(&path),
                Some(MaterialTextureGpuResidency::Requested)
            ) {
                continue;
            }

            self.queue_dictionary_material_texture_decode(path, thread_pool);
            started_jobs = started_jobs.saturating_add(1);

            let elapsed_ms = pump_started.elapsed().as_secs_f32() * 1000.0;
            if elapsed_ms >= decode_budget_ms {
                newengine_ulog_api::ulog::debug!(
                    "render controller: material texture pump yielded started={} active_jobs={} max_jobs={} elapsed_ms={:.2} budget_ms={:.2} remaining={}",
                    started_jobs,
                    self.gpu.material.texture_decode_jobs.len(),
                    max_jobs_this_pump,
                    elapsed_ms,
                    decode_budget_ms,
                    self.gpu.material.texture_queue.len(),
                );
                break;
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
                    newengine_ulog_api::ulog::debug!(
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
                    newengine_ulog_api::ulog::warn!(
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
            MaterialTextureGpuResidency::CpuDecoding { requested_frame } => {
                let waited = self.frame.frame_index.saturating_sub(requested_frame);
                if waited > 180 && waited % 120 == 0 {
                    newengine_ulog_api::ulog::debug!(
                        "render controller: material texture still cpu-decoding path='{}' waited_frames={}",
                        path,
                        waited,
                    );
                }
                fallback
            }
            MaterialTextureGpuResidency::AssetLoading {
                id_hex32,
                requested_frame,
            } => {
                let waited = self.frame.frame_index.saturating_sub(requested_frame);
                if waited > 180 && waited % 120 == 0 {
                    newengine_ulog_api::ulog::debug!(
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
            e.last_seen_frame = self.frame.frame_index;
            if e.base_texture == base_texture
                && e.normal_texture == normal_texture
                && e.roughness_texture == roughness_texture
                && e.shadow_texture == shadow_texture
                && e.sampler == sampler
            {
                self.gpu.material.per_draw_ubo.insert(key, e);
                return Ok(e);
            }
            // Do not destroy the previous bind group immediately. The last
            // submitted frame may still reference it through an in-flight command
            // buffer. Queue it against the current engine frame and let renderer
            // fence-completion events retire it. No guessed frame grace window.
            let old_bg = e.bg;
            let bg = r.create_bind_group(
                newengine_core::render::BindGroupDesc::new(lit.bgl)
                    .with_label("material_lit_entity_bg")
                    .with_uniform0(newengine_core::render::BufferBinding::new(
                        e.ubo,
                        0,
                        LIT_UBO_SIZE,
                    ))
                    .with_texture0(base_texture)
                    .with_texture1(normal_texture)
                    .with_texture2(roughness_texture)
                    .with_texture3(shadow_texture)
                    .with_sampler0(sampler),
            )?;
            self.gpu
                .lifetimes
                .resources
                .retire_bind_group_after_frame(old_bg, self.frame.frame_index);
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
                .with_uniform0(newengine_core::render::BufferBinding::new(
                    ubo,
                    0,
                    LIT_UBO_SIZE,
                ))
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

    pub(super) fn collect_render_lifetime_events(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
    ) {
        self.gpu.lifetimes.resources.collect(r);
    }

    pub(super) fn retire_render_target(&mut self, rt: RenderTargetId) {
        self.gpu
            .lifetimes
            .resources
            .retire_render_target_after_frame(rt, self.frame.frame_index);
    }

    pub(super) fn gc_per_draw_ubos(&mut self, r: &mut dyn newengine_core::render::RenderApi) {
        self.collect_render_lifetime_events(r);
    }

    pub(super) fn gc_deferred_rts(&mut self, r: &mut dyn newengine_core::render::RenderApi) {
        self.collect_render_lifetime_events(r);
    }
    pub(super) fn bridge_render_backend_events<E: Send + 'static>(
        &mut self,
        ctx: &mut newengine_core::ModuleCtx<'_, E>,
        r: &mut dyn newengine_core::render::RenderApi,
    ) {
        self.gpu.lifetimes.resources.subscribe(ctx.events());
        match r.drain_backend_events() {
            Ok(events) => {
                for event in events {
                    self.gpu.material.registry.observe_backend_event(&event);
                    let _ = ctx.events().publish(event);
                }
            }
            Err(err) => {
                newengine_ulog_api::ulog::warn!(
                    "render controller: failed to drain renderer backend events err='{}'",
                    err
                );
            }
        }
        self.collect_render_lifetime_events(r);
    }
}
