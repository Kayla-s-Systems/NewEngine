#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::time::{Duration, Instant};

use newengine_core::render::{RenderApi, RenderBackendEvent, RenderBackendEventKind};
use newengine_core::{EngineError, EngineResult as CoreResult};
use newengine_material_domain_api::{
    MaterialDomainError, MaterialGpuPipeline, MaterialGpuPipelineKey, MaterialGpuPipelineProvider,
    MaterialPipelineBuildProfile, MaterialRenderDevice,
};

struct CoreRenderMaterialDevice<'a> {
    inner: &'a mut dyn RenderApi,
}

impl<'a> CoreRenderMaterialDevice<'a> {
    #[inline]
    fn new(inner: &'a mut dyn RenderApi) -> Self {
        Self { inner }
    }

    #[inline]
    fn map_err(e: newengine_core::EngineError) -> MaterialDomainError {
        MaterialDomainError::other(e.to_string())
    }
}

impl MaterialRenderDevice for CoreRenderMaterialDevice<'_> {
    fn create_bind_group_layout(
        &mut self,
        desc: newengine_core::render::BindGroupLayoutDesc,
    ) -> Result<newengine_core::render::BindGroupLayoutId, MaterialDomainError> {
        self.inner
            .create_bind_group_layout(desc)
            .map_err(Self::map_err)
    }

    fn create_texture(
        &mut self,
        desc: newengine_core::render::TextureDesc,
    ) -> Result<newengine_core::render::TextureId, MaterialDomainError> {
        self.inner.create_texture(desc).map_err(Self::map_err)
    }

    fn create_sampler(
        &mut self,
        desc: newengine_core::render::SamplerDesc,
    ) -> Result<newengine_core::render::SamplerId, MaterialDomainError> {
        self.inner.create_sampler(desc).map_err(Self::map_err)
    }

    fn create_shader(
        &mut self,
        desc: newengine_core::render::ShaderDesc,
    ) -> Result<newengine_core::render::ShaderId, MaterialDomainError> {
        self.inner.create_shader(desc).map_err(Self::map_err)
    }

    fn create_pipeline(
        &mut self,
        desc: newengine_core::render::PipelineDesc,
    ) -> Result<newengine_core::render::PipelineId, MaterialDomainError> {
        self.inner.create_pipeline(desc).map_err(Self::map_err)
    }
}

/// GPU material registry owned by the engine runtime side of the renderer.
///
/// Reusable runtime orchestration no longer owns GameReady/FPS shader paths or
/// material presets. It only stores host-side material-domain providers that are
/// registered by the game/profile layer, then asks the selected provider to build
/// a backend-neutral pipeline bundle through `RenderApi`.
#[derive(Default)]
pub struct MaterialGpuRegistry {
    providers: HashMap<&'static str, Box<dyn MaterialGpuPipelineProvider>>,
    resolved_pipelines: HashMap<String, MaterialGpuPipeline>,
    pending_pipelines: HashMap<String, PendingMaterialPipelineState>,
    shader_event_generation: u64,
}

#[derive(Clone, Debug)]
struct PendingMaterialPipelineState {
    key: MaterialGpuPipelineKey,
    shader_event_generation: u64,
    last_error: String,
    wait_logged: bool,
    last_attempt_at: Instant,
    retry_count: u32,
}

impl MaterialGpuRegistry {
    pub fn register_provider(&mut self, provider: Box<dyn MaterialGpuPipelineProvider>) {
        let key = provider.key();
        let replaced = self.providers.insert(key.as_str(), provider).is_some();
        if replaced {
            self.resolved_pipelines
                .retain(|cache_key, _| !cache_key.starts_with(key.as_str()));
            self.pending_pipelines
                .retain(|cache_key, _| !cache_key.starts_with(key.as_str()));
            newengine_ulog_api::ulog::warn!(
                "render material registry: replaced material-domain provider key='{}'; invalidated cached pipelines for this provider",
                key.as_str()
            );
        }
    }

    pub(crate) fn observe_backend_event(&mut self, event: &RenderBackendEvent) {
        let readiness_event = matches!(
            event.kind,
            RenderBackendEventKind::ShaderCompileCompleted
                | RenderBackendEventKind::ShaderCompileFailed
                | RenderBackendEventKind::ShaderCompileDegradedFallback
        );
        if !readiness_event {
            return;
        }

        self.shader_event_generation = self.shader_event_generation.wrapping_add(1).max(1);
        if !self.pending_pipelines.is_empty() {
            newengine_ulog_api::ulog::info!(
                "render material registry: shader readiness event observed generation={} kind={:?} phase='{}' pending_pipelines={} detail='{}'",
                self.shader_event_generation,
                event.kind,
                event.phase,
                self.pending_pipelines.len(),
                event.detail
            );
        }
    }

    pub(crate) fn require_pipeline(
        &mut self,
        key: MaterialGpuPipelineKey,
        profile: MaterialPipelineBuildProfile,
        r: &mut dyn RenderApi,
    ) -> CoreResult<MaterialGpuPipeline> {
        let cache_key = material_pipeline_cache_key(key, profile);
        if let Some(pipeline) = self.resolved_pipelines.get(&cache_key).copied() {
            self.pending_pipelines.remove(&cache_key);
            return Ok(pipeline);
        }

        if let Some(pending) = self.pending_pipelines.get_mut(&cache_key) {
            let shader_event_observed =
                pending.shader_event_generation != self.shader_event_generation;
            let retry_due = pending.last_attempt_at.elapsed() >= Duration::from_millis(100);

            if !shader_event_observed && !retry_due {
                if !pending.wait_logged {
                    newengine_ulog_api::ulog::warn!(
                        "render material registry: pipeline pending key='{}' cache_key='{}' waiting_for='renderer.shader_compile_event_or_retry_poll' generation={} retry_count={} err='{}'",
                        pending.key.as_str(),
                        cache_key,
                        pending.shader_event_generation,
                        pending.retry_count,
                        pending.last_error
                    );
                    pending.wait_logged = true;
                }
                return Err(EngineError::other(format!(
                    "render material registry: pipeline pending_event key='{}' cache_key='{}' waiting_for='renderer.shader_compile_event_or_retry_poll' err='{}'",
                    pending.key.as_str(),
                    cache_key,
                    pending.last_error
                )));
            }

            pending.last_attempt_at = Instant::now();
            pending.retry_count = pending.retry_count.saturating_add(1);
            if shader_event_observed {
                newengine_ulog_api::ulog::info!(
                    "render material registry: retrying pending pipeline after shader event key='{}' cache_key='{}' previous_generation={} current_generation={} retry_count={}",
                    pending.key.as_str(),
                    cache_key,
                    pending.shader_event_generation,
                    self.shader_event_generation,
                    pending.retry_count
                );
            } else {
                newengine_ulog_api::ulog::debug!(
                    "render material registry: retrying pending pipeline by renderer poll key='{}' cache_key='{}' generation={} retry_count={} err='{}'",
                    pending.key.as_str(),
                    cache_key,
                    self.shader_event_generation,
                    pending.retry_count,
                    pending.last_error
                );
            }
        }

        let Some(provider) = self.providers.get_mut(key.as_str()) else {
            return Err(EngineError::other(format!(
                "render material registry: no material-domain provider registered key='{}'",
                key.as_str()
            )));
        };

        let started_at = Instant::now();
        newengine_ulog_api::ulog::info!(
            "render material registry: pipeline request begin key='{}' cache_key='{}' provider_registered=true cache_miss=true",
            key.as_str(),
            cache_key
        );
        let mut device = CoreRenderMaterialDevice::new(r);
        match provider.require_pipeline(profile, &mut device) {
            Ok(pipeline) => {
                self.resolved_pipelines.insert(cache_key.clone(), pipeline);
                self.pending_pipelines.remove(&cache_key);
                newengine_ulog_api::ulog::info!(
                    "render material registry: pipeline request completed key='{}' cache_key='{}' elapsed_ms={:.2} cached=true",
                    key.as_str(),
                    cache_key,
                    started_at.elapsed().as_secs_f64() * 1000.0
                );
                Ok(pipeline)
            }
            Err(e) => {
                if is_transient_material_pipeline_error(&e) {
                    let state = self
                        .pending_pipelines
                        .entry(cache_key.clone())
                        .or_insert_with(|| PendingMaterialPipelineState {
                            key,
                            shader_event_generation: self.shader_event_generation,
                            last_error: String::new(),
                            wait_logged: false,
                            last_attempt_at: Instant::now(),
                            retry_count: 0,
                        });
                    state.shader_event_generation = self.shader_event_generation;
                    state.last_error = e.to_string();
                    state.wait_logged = false;
                    state.last_attempt_at = Instant::now();
                    newengine_ulog_api::ulog::warn!(
                        "render material registry: pipeline request pending key='{}' cache_key='{}' generation={} err='{}' elapsed_ms={:.2} action='wait_for_renderer_shader_event_or_retry_poll'",
                        key.as_str(),
                        cache_key,
                        self.shader_event_generation,
                        e,
                        started_at.elapsed().as_secs_f64() * 1000.0
                    );
                } else {
                    newengine_ulog_api::ulog::error!(
                        "render material registry: pipeline request failed key='{}' err='{}' elapsed_ms={:.2}",
                        key.as_str(),
                        e,
                        started_at.elapsed().as_secs_f64() * 1000.0
                    );
                }
                Err(EngineError::other(format!(
                    "render material registry: {}",
                    e
                )))
            }
        }
    }
}

fn material_pipeline_cache_key(
    key: MaterialGpuPipelineKey,
    profile: MaterialPipelineBuildProfile,
) -> String {
    format!(
        "{}|scene={:?}|shadow={:?}",
        key.as_str(),
        profile.scene_hdr_color_format,
        profile.shadow_map_color_format
    )
}

fn is_transient_material_pipeline_error(error: &MaterialDomainError) -> bool {
    let mut text = error.to_string();
    text.make_ascii_lowercase();
    text.contains("shader compile queued")
        || text.contains("shader compile pending")
        || text.contains("shader pending")
        || text.contains("shader is not ready yet")
        || text.contains("shader compile job is still pending")
        || text.contains("shader compile job is still pending")
        || text.contains("engine.threading shader admission timeout")
        || text.contains("leave_pending_and_retry_later")
        || text.contains("pipeline pending_event")
}
