impl RuntimeRenderController {
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
}
