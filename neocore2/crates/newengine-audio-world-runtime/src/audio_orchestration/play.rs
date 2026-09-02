impl AudioOrchestrationRuntimeModule {
    fn play_instance(
        &mut self,
        instance_id: AudioInstanceId,
        object_id: AudioObjectId,
        request: AudioPlayInstanceRequest,
        transport_start_sample: u64,
        transport_dispatch_sample: u64,
        render_start_sample: Option<u64>,
    ) {
        let request = match request.sanitized() {
            Ok(request) => request,
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "audio orchestration: play rejected object_id={} instance_id={} err='{}'",
                    object_id.0,
                    instance_id.0,
                    error
                );
                return;
            }
        };
        let Some(object) = self.objects.get(&object_id).cloned() else {
            newengine_ulog_api::ulog::warn!(
                "audio orchestration: play ignored unknown object_id={} instance_id={}",
                object_id.0,
                instance_id.0
            );
            return;
        };
        if !request.route.0.is_empty()
            && self
                .mix_graph
                .as_ref()
                .is_some_and(|graph| !graph.contains_route(&request.route))
        {
            newengine_ulog_api::ulog::warn!(
                "audio orchestration: play rejected unknown route='{}' instance_id={}",
                request.route.0,
                instance_id.0
            );
            return;
        }
        self.stop_instance(instance_id);

        let mut play =
            newengine_audio_api::AudioCuePlayRequest::new(request.cue.logical_path.clone());
        play.route = request.route.clone();
        play.position = request.spatial.then_some(object.state.position);
        play.gain = object.state.gain * request.gain;
        play.pitch = request.pitch;
        play.seed = request
            .seed
            .or(Some(instance_id.0 ^ object_id.0.rotate_left(23)));
        play.scope_id = Some(object_id.0);
        play.start_sample_offset = transport_dispatch_sample.saturating_sub(transport_start_sample);
        if play.start_sample_offset > 0 {
            play.transport_sample_rate = self.transport.sample_rate();
        }
        play.acoustic = object.state.acoustic;
        play.environment = object.state.environment;
        let mut parameters = self.global_parameters.clone();
        parameters.overlay_from(&object.state.parameters);
        parameters.overlay_from(&request.parameters);
        play.parameters = parameters.sanitized();
        play.render_start_sample = render_start_sample;

        match play_audio_cue(&play) {
            Ok(Some(ack)) if ack.accepted => {
                let mut voice_ids = ack.voice_ids;
                if voice_ids.is_empty() {
                    voice_ids.extend(ack.voice_id);
                }
                voice_ids.sort_unstable();
                voice_ids.dedup();
                if voice_ids.is_empty() {
                    newengine_ulog_api::ulog::warn!(
                        "audio orchestration: accepted cue returned no voice handles cue='{}' instance_id={}",
                        request.cue.logical_path,
                        instance_id.0
                    );
                    return;
                }
                self.instances.insert(
                    instance_id,
                    RuntimeInstance {
                        object_id,
                        voice_ids,
                        route: request.route,
                        tags: request.tags,
                        gain: request.gain,
                        spatial: request.spatial,
                        parameters: request.parameters,
                        transport_start_sample,
                        transport_dispatch_sample,
                        render_armed: render_start_sample.is_some(),
                    },
                );
            }
            Ok(Some(ack)) => {
                newengine_ulog_api::ulog::trace!(
                    "audio orchestration: cue rejected cue='{}' instance_id={} message='{}'",
                    request.cue.logical_path,
                    instance_id.0,
                    ack.message
                );
            }
            Ok(None) => {}
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "audio orchestration: cue play failed cue='{}' instance_id={} err='{}'",
                    request.cue.logical_path,
                    instance_id.0,
                    error
                );
            }
        }
    }

    fn play_stream_instance(
        &mut self,
        instance_id: AudioInstanceId,
        object_id: AudioObjectId,
        request: AudioPlayStreamInstanceRequest,
        transport_start_sample: u64,
        transport_dispatch_sample: u64,
        render_start_sample: Option<u64>,
    ) {
        let request = match request.sanitized() {
            Ok(request) => request,
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "audio orchestration: stream play rejected object_id={} instance_id={} err='{}'",
                    object_id.0,
                    instance_id.0,
                    error
                );
                return;
            }
        };
        let Some(object) = self.objects.get(&object_id).cloned() else {
            newengine_ulog_api::ulog::warn!(
                "audio orchestration: stream play ignored unknown object_id={} instance_id={}",
                object_id.0,
                instance_id.0
            );
            return;
        };
        if !request.route.0.is_empty()
            && self
                .mix_graph
                .as_ref()
                .is_some_and(|graph| !graph.contains_route(&request.route))
        {
            newengine_ulog_api::ulog::warn!(
                "audio orchestration: stream play rejected unknown route='{}' instance_id={}",
                request.route.0,
                instance_id.0
            );
            return;
        }
        self.stop_instance(instance_id);

        let mut stream = request.stream.clone();
        stream.route = request.route.clone();
        let instance_gain = newengine_audio_api::sanitize_gain(request.gain * stream.gain);
        stream.gain = newengine_audio_api::sanitize_gain(object.state.gain * instance_gain);
        if request.spatial {
            stream.spatial = Some(newengine_audio_api::AudioSpatialParams {
                position: object.state.position,
            });
        }
        stream.scope_id = Some(object_id.0);
        stream.acoustic = object.state.acoustic;
        stream.environment = object.state.environment;
        let lateness_samples = transport_dispatch_sample.saturating_sub(transport_start_sample);
        if lateness_samples > 0 {
            stream.start_seconds = (stream.start_seconds
                + lateness_samples as f64 / f64::from(self.transport.sample_rate()))
            .clamp(0.0, 86_400.0);
        }
        stream.render_start_sample = render_start_sample;
        stream = stream.sanitized();

        match play_audio_stream(&stream) {
            Ok(Some(ack)) if ack.accepted => {
                let mut voice_ids = ack.voice_ids;
                if voice_ids.is_empty() {
                    voice_ids.extend(ack.voice_id);
                }
                voice_ids.sort_unstable();
                voice_ids.dedup();
                if voice_ids.is_empty() {
                    newengine_ulog_api::ulog::warn!(
                        "audio orchestration: accepted stream returned no voice handles uri='{}' instance_id={}",
                        stream.clip.uri,
                        instance_id.0
                    );
                    return;
                }
                self.instances.insert(
                    instance_id,
                    RuntimeInstance {
                        object_id,
                        voice_ids,
                        route: request.route,
                        tags: request.tags,
                        gain: instance_gain,
                        spatial: request.spatial,
                        parameters: request.parameters,
                        transport_start_sample,
                        transport_dispatch_sample,
                        render_armed: render_start_sample.is_some(),
                    },
                );
            }
            Ok(Some(ack)) => {
                newengine_ulog_api::ulog::trace!(
                    "audio orchestration: stream rejected uri='{}' instance_id={} message='{}'",
                    stream.clip.uri,
                    instance_id.0,
                    ack.message
                );
            }
            Ok(None) => {}
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "audio orchestration: stream play failed uri='{}' instance_id={} err='{}'",
                    stream.clip.uri,
                    instance_id.0,
                    error
                );
            }
        }
    }
}
