use newengine_assets::{
    AssetErrorKind, AssetServiceClient, RuntimeTextureAsset, RuntimeTextureFormat,
};
use newengine_core::render::{
    Extent2D, GpuResourceResidencyState, TextureDesc, TextureFormat, TextureId, TextureMipDataDesc,
    TextureUsage,
};
use newengine_core::{TaskLane, TaskPriority, TaskRequest, ThreadPoolHandle};
use newengine_plugin_host::default_host_api;
use newengine_task_api::{task_domain, task_pass};
use parking_lot::Mutex;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;

use super::super::controller::RuntimeRenderController;
use super::super::material_bindings::MaterialTextureGpuResidency;
use super::super::state::MaterialTextureDecodeJob;

const MATERIAL_TEXTURE_ASSET_RETRY_FRAMES: u64 = 4;
const MATERIAL_TEXTURE_ALLOCATION_STALL_WARN_MS: f32 = 16.67;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::render_controller) enum MaterialTextureReadyState {
    Ready(TextureId),
    Waiting,
    Failed,
}

#[inline]
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

#[inline]
fn material_texture_decode_request(path: &str, frame_index: u64) -> TaskRequest {
    let task_path = sanitize_material_texture_task_id(path);
    TaskRequest::new("material.texture.decode")
        .with_source("render.controller")
        .with_owner("engine.render")
        .with_category("asset-decode")
        .with_lane(TaskLane::AssetIo)
        // Texture semantic decode is required for residency, but it is not frame-critical CPU
        // work. Simulation/RenderPrep interactive jobs must remain ahead of it in the shared pool.
        .with_priority(TaskPriority::Normal)
        .with_frame_id(frame_index)
        .with_dependency_group(format!("frame.{frame_index}.asset-io.texture-decode"))
        .with_task_domain(task_domain::ENGINE_ASSETS)
        .with_task_pass(task_pass::TEXTURE_DECODE)
        .with_task_id(format!("render.material.texture.decode.{task_path}"))
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
        let request = material_texture_decode_request(&path, self.frame.frame_index);
        let host_context = newengine_plugin_host::current_host_context();
        let ticket = thread_pool.submit_request(request, move || {
            let decoded = newengine_plugin_host::with_host_context(&host_context, || {
                let assets = AssetServiceClient::new(default_host_api());
                assets.textures_entry_runtime_ref_v1_typed(&worker_path)
            });
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
        let texture_width = texture_asset.width;
        let texture_height = texture_asset.height;
        let texture_format = texture_asset.format;
        let extent = Extent2D::new(texture_width, texture_height);
        let mip_levels = NonZeroU32::new(texture_asset.mips.len().max(1) as u32)
            .expect("runtime texture mip count is non-zero");

        // Uncompressed RGBA dictionaries are normalized to a base-level upload and
        // backend-generated mip chain. Sending many small RGBA BufferImageCopy regions
        // through the deferred explicit-mip path has proven driver-fragile and can
        // escalate from a bad submission to VK_ERROR_DEVICE_LOST. BCn payloads cannot
        // be blitted safely, so they keep their authored runtime mip chain.
        let (payload, mip_data, upload_contract) = match texture_format {
            RuntimeTextureFormat::Rgba8Unorm | RuntimeTextureFormat::Rgba8Srgb => {
                let Some(base_mip) = texture_asset
                    .mips
                    .iter()
                    .find(|mip| mip.level == 0)
                    .or_else(|| texture_asset.mips.first())
                else {
                    let message = "runtime RGBA texture has no base mip".to_owned();
                    newengine_ulog_api::ulog::warn!(
                        "render controller: material texture rejected path='{}' err='{}'",
                        path,
                        message
                    );
                    self.gpu
                        .material
                        .textures
                        .insert(path, MaterialTextureGpuResidency::Failed { message });
                    return;
                };
                let expected_base_bytes = (texture_width as usize)
                    .saturating_mul(texture_height as usize)
                    .saturating_mul(4);
                if base_mip.level != 0
                    || base_mip.width != texture_width
                    || base_mip.height != texture_height
                    || base_mip.bytes.len() != expected_base_bytes
                {
                    let message = format!(
                        "runtime RGBA base mip contract mismatch level={} extent={}x{} bytes={} expected_level=0 expected_extent={}x{} expected_bytes={}",
                        base_mip.level,
                        base_mip.width,
                        base_mip.height,
                        base_mip.bytes.len(),
                        texture_width,
                        texture_height,
                        expected_base_bytes,
                    );
                    newengine_ulog_api::ulog::warn!(
                        "render controller: material texture rejected path='{}' err='{}'",
                        path,
                        message
                    );
                    self.gpu
                        .material
                        .textures
                        .insert(path, MaterialTextureGpuResidency::Failed { message });
                    return;
                }
                (base_mip.bytes.clone(), Vec::new(), "rgba-base+backend-mips")
            }
            _ => {
                let (payload, layout) = texture_asset.into_concatenated_payload_and_layout();
                let mip_data = layout
                    .into_iter()
                    .map(|mip| {
                        TextureMipDataDesc::new(
                            mip.level,
                            mip.width,
                            mip.height,
                            mip.offset,
                            mip.byte_len,
                        )
                    })
                    .collect();
                (payload, mip_data, "bcn-authored-mips")
            }
        };
        let payload_bytes = payload.len();
        if payload_bytes > super::super::render_quality::MATERIAL_TEXTURE_MAX_UPLOAD_PAYLOAD_BYTES {
            let message = format!(
                "texture upload payload exceeds runtime safety limit bytes={} limit={} format={:?} extent={}x{}; use BC-compressed runtime assets",
                payload_bytes,
                super::super::render_quality::MATERIAL_TEXTURE_MAX_UPLOAD_PAYLOAD_BYTES,
                texture_format,
                texture_width,
                texture_height,
            );
            newengine_ulog_api::ulog::warn!(
                "render controller: material texture rejected path='{}' err='{}'",
                path,
                message
            );
            self.gpu
                .material
                .textures
                .insert(path, MaterialTextureGpuResidency::Failed { message });
            return;
        }
        let upload_started = Instant::now();
        let texture_desc = TextureDesc::new(
            extent,
            render_texture_format_from_runtime(texture_format),
            TextureUsage::Sampled,
        )
        .with_label(format!("material_tex:{path}"))
        .with_mips(mip_levels);
        let texture_desc = if mip_data.is_empty() {
            texture_desc.with_deferred_data(payload)
        } else {
            texture_desc.with_deferred_mip_data(mip_data, payload)
        };
        match r.create_texture(texture_desc) {
            Ok(texture) => {
                newengine_ulog_api::ulog::debug!(
                    "render controller: material texture packet upload queued path='{}' method='assets.textures.entry_runtime_v1' contract='{}' texture={:?} frame={}",
                    path,
                    upload_contract,
                    texture,
                    self.frame.frame_index
                );
                let upload_elapsed_ms = upload_started.elapsed().as_secs_f32() * 1000.0;
                if upload_elapsed_ms >= MATERIAL_TEXTURE_ALLOCATION_STALL_WARN_MS {
                    newengine_ulog_api::ulog::warn!(
                        "render controller: texture allocation exceeded frame budget path='{}' bytes={} elapsed_ms={:.2} budget_ms={:.2} stall_warn_ms={:.2}",
                        path,
                        payload_bytes,
                        upload_elapsed_ms,
                        super::super::render_quality::MATERIAL_TEXTURE_DECODE_PUMP_BUDGET_MS,
                        MATERIAL_TEXTURE_ALLOCATION_STALL_WARN_MS,
                    );
                } else if upload_elapsed_ms
                    >= super::super::render_quality::MATERIAL_TEXTURE_DECODE_PUMP_BUDGET_MS
                {
                    newengine_ulog_api::ulog::debug!(
                        "render controller: texture allocation above pump target path='{}' bytes={} elapsed_ms={:.2} budget_ms={:.2}",
                        path,
                        payload_bytes,
                        upload_elapsed_ms,
                        super::super::render_quality::MATERIAL_TEXTURE_DECODE_PUMP_BUDGET_MS,
                    );
                }
                self.gpu.material.textures.insert(
                    path,
                    MaterialTextureGpuResidency::GpuLoading {
                        texture,
                        requested_frame: self.frame.frame_index,
                        last_residency_poll_frame: None,
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
            .filter(|(_path, job)| job.is_complete())
            .map(|(path, _job)| path.clone())
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
                Ok(texture_asset) => self.upload_decoded_material_texture(r, path, texture_asset),
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
                        self.enqueue_material_texture_path(path);
                    }
                }
                Err(e) => {
                    let message = format!("assets.textures.entry_runtime_v1 failed err='{e}'");
                    let line = format!(
                        "render controller: material texture packet lookup failed path='{}' method='assets.textures.entry_runtime_v1' kind='{}' err='{}'",
                        path, e.kind, e,
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

    #[inline]
    fn enqueue_material_texture_path(&mut self, path: String) {
        if self.gpu.material.texture_queued.insert(path.clone()) {
            self.gpu.material.texture_queue.push_back(path);
        }
    }

    #[inline]
    fn pop_material_texture_path(&mut self) -> Option<String> {
        let path = self.gpu.material.texture_queue.pop_front()?;
        self.gpu.material.texture_queued.remove(&path);
        Some(path)
    }

    pub(in crate::render_controller) fn request_material_texture(&mut self, path: &str) {
        let path = path.trim();
        if path.is_empty() || self.gpu.material.textures.contains_key(path) {
            return;
        }
        self.gpu
            .material
            .textures
            .insert(path.to_owned(), MaterialTextureGpuResidency::Requested);
        self.enqueue_material_texture_path(path.to_owned());
    }

    pub(in crate::render_controller) fn pump_material_texture_requests(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        thread_pool: Option<&ThreadPoolHandle>,
        max_start_jobs: u32,
        max_decode_jobs: u32,
    ) {
        let max_start_jobs = max_start_jobs.max(1);
        let max_decode_jobs = max_decode_jobs
            .max(1)
            .min(super::super::render_quality::MATERIAL_TEXTURE_MAX_ASYNC_DECODE_JOBS as u32);
        let max_jobs_this_pump = max_start_jobs.min(max_decode_jobs).max(1);
        let pump_started = Instant::now();
        let decode_budget_ms =
            super::super::render_quality::MATERIAL_TEXTURE_DECODE_PUMP_BUDGET_MS.max(0.25);

        self.poll_material_texture_decode_jobs(
            r,
            super::super::render_quality::MATERIAL_TEXTURE_MAX_UPLOADS_PER_FRAME,
        );

        let loading_retry_paths = self
            .gpu
            .material
            .textures
            .iter()
            .filter_map(|(path, state)| match state {
                MaterialTextureGpuResidency::AssetLoading {
                    requested_frame, ..
                } if self.frame.frame_index.saturating_sub(*requested_frame)
                    >= MATERIAL_TEXTURE_ASSET_RETRY_FRAMES =>
                {
                    Some(path.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        for path in loading_retry_paths {
            self.gpu
                .material
                .textures
                .insert(path.clone(), MaterialTextureGpuResidency::Requested);
            self.enqueue_material_texture_path(path);
        }

        let mut started_jobs = 0_u32;
        while started_jobs < max_jobs_this_pump {
            let active_jobs = self.gpu.material.texture_decode_jobs.len() as u32;
            if active_jobs >= max_decode_jobs {
                break;
            }

            let Some(path) = self.pop_material_texture_path() else {
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
    pub(in crate::render_controller) fn material_texture_if_ready(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        path: &str,
        status_owner: &'static str,
    ) -> Option<TextureId> {
        self.request_material_texture(path);
        match self.material_texture_ready_state(r, path, status_owner) {
            MaterialTextureReadyState::Ready(texture) => Some(texture),
            MaterialTextureReadyState::Waiting | MaterialTextureReadyState::Failed => None,
        }
    }

    #[inline]
    pub(in crate::render_controller) fn material_texture_or_default(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        path: Option<&str>,
        fallback: TextureId,
    ) -> TextureId {
        let Some(path) = path else {
            return fallback;
        };

        self.request_material_texture(path);
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
}

#[cfg(test)]
mod material_texture_decode_policy_tests {
    use super::*;

    #[test]
    fn texture_decode_is_asset_io_normal_priority_not_frame_interactive() {
        let request = material_texture_decode_request("textures/characters/abby.ytd@m00_base", 42);
        assert_eq!(request.lane, TaskLane::AssetIo);
        assert_eq!(request.priority, TaskPriority::Normal);
        assert_eq!(request.frame_id, Some(42));
        assert_eq!(request.task_domain, task_domain::ENGINE_ASSETS);
        assert_eq!(request.task_pass, task_pass::TEXTURE_DECODE);
        assert!(request
            .dependency_group
            .as_deref()
            .is_some_and(|group| group == "frame.42.asset-io.texture-decode"));
    }
}
