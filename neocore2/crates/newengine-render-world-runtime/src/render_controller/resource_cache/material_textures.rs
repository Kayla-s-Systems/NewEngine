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
use super::super::state::{
    MaterialTextureDecodeJob, MaterialTexturePriority, MaterialTextureQueueEntry,
    MaterialTextureStreamingClass, MaterialTextureUploadCandidate,
};

const MATERIAL_TEXTURE_ASSET_RETRY_FRAMES: u64 = 4;
const MATERIAL_TEXTURE_ALLOCATION_STALL_WARN_MS: f32 = 16.67;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::render_controller) enum MaterialTextureReadyState {
    Ready(TextureId),
    Waiting,
    Failed,
}

#[inline]
fn quantize_unit_f32(value: f32) -> u16 {
    if !value.is_finite() {
        return 0;
    }
    (value.clamp(0.0, 1.0) * u16::MAX as f32).round() as u16
}

#[inline]
fn proximity_score(distance_m: f32) -> u16 {
    if !distance_m.is_finite() || distance_m < 0.0 {
        return 0;
    }
    // Hyperbolic falloff keeps useful ordering from first-person distances through large rooms
    // without baking a game-specific far plane into the scheduler.
    quantize_unit_f32(1.0 / (1.0 + distance_m))
}

#[inline]
fn runtime_texture_payload_bytes(asset: &RuntimeTextureAsset) -> usize {
    asset.mips.iter().map(|mip| mip.bytes.len()).sum()
}

#[inline]
fn streaming_priority_from_hints(
    class: MaterialTextureStreamingClass,
    visible_now: bool,
    screen_coverage: f32,
    distance_m: f32,
    material_importance: u8,
    player_weapon_relevance: u8,
    mip_urgency: u8,
) -> MaterialTexturePriority {
    MaterialTexturePriority {
        class,
        visible_now,
        screen_coverage_q: quantize_unit_f32(screen_coverage),
        proximity_q: proximity_score(distance_m),
        material_importance,
        player_weapon_relevance,
        mip_urgency,
    }
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

    fn poll_material_texture_decode_jobs(&mut self, max_completions: u32) {
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
                Ok(texture_asset) => {
                    let payload_bytes = runtime_texture_payload_bytes(&texture_asset);
                    let priority = self
                        .gpu
                        .material
                        .texture_priorities
                        .get(&path)
                        .copied()
                        .unwrap_or_else(MaterialTexturePriority::secondary);
                    let frame = self.frame.frame_index;
                    self.gpu.material.texture_upload_queue.insert(
                        path.clone(),
                        MaterialTextureUploadCandidate {
                            asset: texture_asset,
                            payload_bytes,
                            priority,
                            decoded_frame: frame,
                            last_touched_frame: frame,
                        },
                    );
                    self.gpu.material.textures.insert(
                        path,
                        MaterialTextureGpuResidency::GpuQueued {
                            payload_bytes,
                            requested_frame: frame,
                        },
                    );
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
                        let priority = self
                            .gpu
                            .material
                            .texture_priorities
                            .get(&path)
                            .copied()
                            .unwrap_or_else(MaterialTexturePriority::secondary);
                        self.enqueue_material_texture_path_with_priority(path, priority);
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
    fn merge_material_texture_priority(
        current: MaterialTexturePriority,
        incoming: MaterialTexturePriority,
    ) -> MaterialTexturePriority {
        MaterialTexturePriority {
            class: current.class.max(incoming.class),
            visible_now: current.visible_now || incoming.visible_now,
            screen_coverage_q: current.screen_coverage_q.max(incoming.screen_coverage_q),
            proximity_q: current.proximity_q.max(incoming.proximity_q),
            material_importance: current
                .material_importance
                .max(incoming.material_importance),
            player_weapon_relevance: current
                .player_weapon_relevance
                .max(incoming.player_weapon_relevance),
            mip_urgency: current.mip_urgency.max(incoming.mip_urgency),
        }
    }

    #[inline]
    fn material_texture_queue_rank(
        entry: &MaterialTextureQueueEntry,
        current_frame: u64,
    ) -> (u8, u8, u16, u16, u8, u8, u8, u16) {
        let visible_recently = entry.priority.visible_now
            && current_frame.saturating_sub(entry.last_touched_frame) <= 2;
        let age = current_frame
            .saturating_sub(entry.enqueued_frame)
            .min(u16::MAX as u64) as u16;
        let coverage = if visible_recently {
            entry.priority.screen_coverage_q
        } else {
            0
        };
        let proximity = if visible_recently {
            entry.priority.proximity_q
        } else {
            0
        };
        (
            entry.priority.class as u8,
            u8::from(visible_recently),
            coverage,
            proximity,
            entry.priority.player_weapon_relevance,
            entry.priority.material_importance,
            entry.priority.mip_urgency,
            age,
        )
    }

    #[inline]
    fn material_texture_upload_rank(
        candidate: &MaterialTextureUploadCandidate,
        current_frame: u64,
    ) -> (u8, u8, u16, u16, u8, u8, u8, u16) {
        let visible_recently = candidate.priority.visible_now
            && current_frame.saturating_sub(candidate.last_touched_frame) <= 2;
        let age = current_frame
            .saturating_sub(candidate.decoded_frame)
            .min(u16::MAX as u64) as u16;
        (
            candidate.priority.class as u8,
            u8::from(visible_recently),
            if visible_recently {
                candidate.priority.screen_coverage_q
            } else {
                0
            },
            if visible_recently {
                candidate.priority.proximity_q
            } else {
                0
            },
            candidate.priority.player_weapon_relevance,
            candidate.priority.material_importance,
            candidate.priority.mip_urgency,
            age,
        )
    }

    fn pump_material_texture_uploads(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        max_uploads: u32,
        max_bytes: usize,
    ) {
        if max_uploads == 0 || max_bytes == 0 {
            return;
        }
        let mut uploads = 0_u32;
        let mut uploaded_bytes = 0_usize;
        while uploads < max_uploads {
            let frame = self.frame.frame_index;
            let Some(path) = self
                .gpu
                .material
                .texture_upload_queue
                .iter()
                .max_by(|(a_path, a), (b_path, b)| {
                    Self::material_texture_upload_rank(a, frame)
                        .cmp(&Self::material_texture_upload_rank(b, frame))
                        .then_with(|| b_path.cmp(a_path))
                })
                .map(|(path, _)| path.clone())
            else {
                break;
            };
            let payload_bytes = self
                .gpu
                .material
                .texture_upload_queue
                .get(&path)
                .map(|candidate| candidate.payload_bytes)
                .unwrap_or(0);
            let would_exceed = uploaded_bytes.saturating_add(payload_bytes) > max_bytes;
            if uploads > 0 && would_exceed {
                break;
            }
            let Some(candidate) = self.gpu.material.texture_upload_queue.remove(&path) else {
                continue;
            };
            if would_exceed {
                newengine_ulog_api::ulog::debug!(
                    "render controller: priority texture upload exceeds soft frame byte budget path='{}' bytes={} budget_bytes={} policy='allow one highest-priority packet to avoid starvation; single-payload hard cap still applies'",
                    path,
                    candidate.payload_bytes,
                    max_bytes,
                );
            }
            uploaded_bytes = uploaded_bytes.saturating_add(candidate.payload_bytes);
            uploads = uploads.saturating_add(1);
            self.upload_decoded_material_texture(r, path, candidate.asset);
        }
        if uploads > 0
            && (self.frame.frame_index <= 4 || self.frame.frame_index.is_multiple_of(120))
        {
            newengine_ulog_api::ulog::debug!(
                "render controller: texture upload pump frame={} uploads={} bytes={} budget_uploads={} budget_bytes={} decoded_waiting={} policy='CPU decode and GPU upload budgets are independent'",
                self.frame.frame_index,
                uploads,
                uploaded_bytes,
                max_uploads,
                max_bytes,
                self.gpu.material.texture_upload_queue.len(),
            );
        }
    }

    fn enqueue_material_texture_path_with_priority(
        &mut self,
        path: String,
        priority: MaterialTexturePriority,
    ) {
        let frame = self.frame.frame_index;
        let effective = self
            .gpu
            .material
            .texture_priorities
            .entry(path.clone())
            .and_modify(|current| {
                *current = Self::merge_material_texture_priority(*current, priority);
            })
            .or_insert(priority)
            .to_owned();
        self.gpu
            .material
            .texture_queue
            .entry(path)
            .and_modify(|entry| {
                entry.priority = Self::merge_material_texture_priority(entry.priority, effective);
                entry.last_touched_frame = frame;
            })
            .or_insert(MaterialTextureQueueEntry {
                priority: effective,
                enqueued_frame: frame,
                last_touched_frame: frame,
            });
    }

    fn pop_material_texture_path(&mut self) -> Option<String> {
        let frame = self.frame.frame_index;
        let path = self
            .gpu
            .material
            .texture_queue
            .iter()
            .max_by(|(a_path, a), (b_path, b)| {
                Self::material_texture_queue_rank(a, frame)
                    .cmp(&Self::material_texture_queue_rank(b, frame))
                    .then_with(|| b_path.cmp(a_path))
            })
            .map(|(path, _)| path.clone())?;
        self.gpu.material.texture_queue.remove(&path);
        Some(path)
    }

    pub(in crate::render_controller) fn request_material_texture_with_priority(
        &mut self,
        path: &str,
        priority: MaterialTexturePriority,
    ) {
        let path = path.trim();
        if path.is_empty() {
            return;
        }
        let frame = self.frame.frame_index;
        let effective = self
            .gpu
            .material
            .texture_priorities
            .entry(path.to_owned())
            .and_modify(|current| {
                *current = Self::merge_material_texture_priority(*current, priority);
            })
            .or_insert(priority)
            .to_owned();
        if let Some(candidate) = self.gpu.material.texture_upload_queue.get_mut(path) {
            candidate.priority =
                Self::merge_material_texture_priority(candidate.priority, effective);
            candidate.last_touched_frame = frame;
        }

        match self.gpu.material.textures.get(path) {
            Some(MaterialTextureGpuResidency::Requested) => {
                self.enqueue_material_texture_path_with_priority(path.to_owned(), effective);
            }
            None => {
                self.gpu
                    .material
                    .textures
                    .insert(path.to_owned(), MaterialTextureGpuResidency::Requested);
                self.enqueue_material_texture_path_with_priority(path.to_owned(), effective);
            }
            Some(
                MaterialTextureGpuResidency::AssetLoading { .. }
                | MaterialTextureGpuResidency::CpuDecoding { .. }
                | MaterialTextureGpuResidency::GpuQueued { .. }
                | MaterialTextureGpuResidency::GpuLoading { .. }
                | MaterialTextureGpuResidency::Ready { .. }
                | MaterialTextureGpuResidency::Failed { .. },
            ) => {}
        }
    }

    pub(in crate::render_controller) fn request_material_texture(&mut self, path: &str) {
        self.request_material_texture_with_priority(path, MaterialTexturePriority::secondary());
    }

    /// Re-score a not-yet-started request as currently visible streaming work. Draw paths call this
    /// after culling, so a camera turn automatically promotes newly visible textures without
    /// cancelling decode jobs that are already in flight.
    pub(in crate::render_controller) fn request_visible_material_texture(&mut self, path: &str) {
        self.request_material_texture_with_priority(
            path,
            MaterialTexturePriority::streaming_visible(),
        );
    }

    /// Dynamic view-driven priority update. Call this only after visibility/culling has admitted
    /// the surface. The scheduler combines semantic class with projected coverage, distance,
    /// material importance, player/weapon relevance and current mip urgency.
    pub(in crate::render_controller) fn request_material_texture_with_view_hints(
        &mut self,
        path: &str,
        class: MaterialTextureStreamingClass,
        screen_coverage: f32,
        distance_m: f32,
        material_importance: u8,
        player_weapon_relevance: u8,
        mip_urgency: u8,
    ) {
        self.request_material_texture_with_priority(
            path,
            streaming_priority_from_hints(
                class,
                true,
                screen_coverage,
                distance_m,
                material_importance,
                player_weapon_relevance,
                mip_urgency,
            ),
        );
    }

    /// Re-score the three canonical PBR texture channels for a surface that survived culling.
    /// Base color is always the most urgent; normal remains streaming-critical for visible geometry;
    /// roughness is secondary unless the material belongs to a player/weapon complete-PBR surface.
    pub(in crate::render_controller) fn request_material_set_with_view_hints(
        &mut self,
        base_color: Option<&str>,
        normal: Option<&str>,
        roughness: Option<&str>,
        screen_coverage: f32,
        distance_m: f32,
        player_weapon_relevance: u8,
        complete_pbr_surface: bool,
    ) {
        if let Some(path) = base_color {
            self.request_material_texture_with_view_hints(
                path,
                MaterialTextureStreamingClass::StreamingCritical,
                screen_coverage,
                distance_m,
                240,
                player_weapon_relevance,
                240,
            );
        }
        if let Some(path) = normal {
            self.request_material_texture_with_view_hints(
                path,
                MaterialTextureStreamingClass::StreamingCritical,
                screen_coverage,
                distance_m,
                160,
                player_weapon_relevance,
                160,
            );
        }
        if let Some(path) = roughness {
            self.request_material_texture_with_view_hints(
                path,
                if complete_pbr_surface || player_weapon_relevance > 0 {
                    MaterialTextureStreamingClass::StreamingCritical
                } else {
                    MaterialTextureStreamingClass::Secondary
                },
                screen_coverage,
                distance_m,
                96,
                player_weapon_relevance,
                96,
            );
        }
    }

    /// Launch-critical world texture request. This replaces the old FIFO `push_front` workaround
    /// with a persistent score that composes with visibility, age and future coverage/distance hints.
    pub(in crate::render_controller) fn prioritize_material_texture(&mut self, path: &str) {
        self.request_material_texture_with_priority(path, MaterialTexturePriority::launch_world());
    }

    pub(in crate::render_controller) fn prioritize_player_weapon_texture(&mut self, path: &str) {
        self.request_material_texture_with_priority(
            path,
            MaterialTexturePriority::launch_player_weapon(),
        );
    }

    pub(in crate::render_controller) fn pump_material_texture_requests(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        thread_pool: Option<&ThreadPoolHandle>,
        max_start_jobs: u32,
        max_decode_jobs: u32,
    ) {
        let max_start_jobs = max_start_jobs.max(1);
        let adaptive_ceiling = super::super::render_quality::material_texture_async_decode_ceiling(
            self.runtime_profile.hardware_tier(),
        ) as u32;
        let max_decode_jobs = max_decode_jobs.max(1).min(adaptive_ceiling.max(1));
        let max_jobs_this_pump = max_start_jobs.min(max_decode_jobs).max(1);
        let pump_started = Instant::now();
        let decode_budget_ms =
            super::super::render_quality::MATERIAL_TEXTURE_DECODE_PUMP_BUDGET_MS.max(0.25);

        let completion_harvest = max_decode_jobs.saturating_mul(2).max(4);
        self.poll_material_texture_decode_jobs(completion_harvest);
        let (upload_count_budget, upload_byte_budget) =
            super::super::render_quality::material_texture_gpu_upload_budget(
                self.runtime_profile.hardware_tier(),
            );
        self.pump_material_texture_uploads(
            r,
            upload_count_budget
                .min(super::super::render_quality::MATERIAL_TEXTURE_MAX_UPLOADS_PER_FRAME),
            upload_byte_budget,
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
            let priority = self
                .gpu
                .material
                .texture_priorities
                .get(&path)
                .copied()
                .unwrap_or_else(MaterialTexturePriority::secondary);
            self.enqueue_material_texture_path_with_priority(path, priority);
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
                | MaterialTextureGpuResidency::CpuDecoding { .. }
                | MaterialTextureGpuResidency::GpuQueued { .. },
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

    #[test]
    fn launch_priority_outranks_streaming_and_secondary() {
        use super::super::super::state::{MaterialTexturePriority, MaterialTextureQueueEntry};
        let frame = 100;
        let launch = MaterialTextureQueueEntry {
            priority: MaterialTexturePriority::launch_world(),
            enqueued_frame: 100,
            last_touched_frame: 100,
        };
        let streaming = MaterialTextureQueueEntry {
            priority: MaterialTexturePriority::streaming_visible(),
            enqueued_frame: 1,
            last_touched_frame: 100,
        };
        let secondary = MaterialTextureQueueEntry {
            priority: MaterialTexturePriority::secondary(),
            enqueued_frame: 1,
            last_touched_frame: 1,
        };
        assert!(
            RuntimeRenderController::material_texture_queue_rank(&launch, frame)
                > RuntimeRenderController::material_texture_queue_rank(&streaming, frame)
        );
        assert!(
            RuntimeRenderController::material_texture_queue_rank(&streaming, frame)
                > RuntimeRenderController::material_texture_queue_rank(&secondary, frame)
        );
    }

    #[test]
    fn priority_merge_is_monotonic() {
        use super::super::super::state::MaterialTexturePriority;
        let merged = RuntimeRenderController::merge_material_texture_priority(
            MaterialTexturePriority::secondary(),
            MaterialTexturePriority::launch_player_weapon(),
        );
        assert_eq!(
            merged.class,
            super::super::super::state::MaterialTextureStreamingClass::LaunchCritical
        );
        assert!(merged.visible_now);
        assert_eq!(merged.player_weapon_relevance, u8::MAX);
    }

    #[test]
    fn view_hint_score_prefers_large_near_visible_surface() {
        let near = streaming_priority_from_hints(
            MaterialTextureStreamingClass::StreamingCritical,
            true,
            0.5,
            2.0,
            128,
            0,
            128,
        );
        let far = streaming_priority_from_hints(
            MaterialTextureStreamingClass::StreamingCritical,
            true,
            0.02,
            40.0,
            128,
            0,
            128,
        );
        assert!(near.screen_coverage_q > far.screen_coverage_q);
        assert!(near.proximity_q > far.proximity_q);
    }

    #[test]
    fn stale_visibility_stops_using_geometry_boost() {
        use super::super::super::state::MaterialTextureQueueEntry;
        let priority = streaming_priority_from_hints(
            MaterialTextureStreamingClass::StreamingCritical,
            true,
            1.0,
            1.0,
            100,
            0,
            100,
        );
        let entry = MaterialTextureQueueEntry {
            priority,
            enqueued_frame: 1,
            last_touched_frame: 1,
        };
        let recent = RuntimeRenderController::material_texture_queue_rank(&entry, 2);
        let stale = RuntimeRenderController::material_texture_queue_rank(&entry, 10);
        assert!(recent > stale);
    }
}
