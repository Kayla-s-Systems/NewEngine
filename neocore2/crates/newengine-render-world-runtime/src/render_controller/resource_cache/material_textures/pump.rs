impl RuntimeRenderController {
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
}
