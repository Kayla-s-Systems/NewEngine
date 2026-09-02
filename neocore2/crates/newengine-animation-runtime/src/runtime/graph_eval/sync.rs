fn sync_marker_time(track: &CompiledSyncMarkerTrack, marker_index: usize) -> Option<f32> {
    track
        .markers
        .iter()
        .find(|marker| marker.marker_index == marker_index)
        .map(|marker| marker.time_seconds)
}

fn sync_marker_interval_phase(
    clip: &AnimationClip,
    track: &CompiledSyncMarkerTrack,
    playback_time_seconds: f32,
) -> Option<SyncMarkerIntervalPhase> {
    if track.markers.len() < 2 || clip.duration_seconds <= 1.0e-6 {
        return None;
    }
    let duration = clip.duration_seconds;
    let playback = playback_time_seconds.max(0.0);
    let clip_cycle = (playback / duration).floor() as i64;
    let local_time = playback.rem_euclid(duration);
    let next_index = track
        .markers
        .iter()
        .position(|marker| marker.time_seconds > local_time)
        .unwrap_or(0);
    let previous_index = if next_index == 0 {
        track.markers.len() - 1
    } else {
        next_index - 1
    };
    let previous = track.markers[previous_index];
    let next = track.markers[next_index];
    let previous_clip_cycle = if next_index == 0 && local_time < track.markers[0].time_seconds {
        clip_cycle - 1
    } else {
        clip_cycle
    };
    let start = previous.time_seconds + previous_clip_cycle as f32 * duration;
    let next_clip_cycle = if next_index == 0 {
        previous_clip_cycle + 1
    } else {
        previous_clip_cycle
    };
    let end = next.time_seconds + next_clip_cycle as f32 * duration;
    let marker_zero_time = sync_marker_time(track, 0)?;
    let semantic_cycle =
        previous_clip_cycle - i64::from(previous.time_seconds + 1.0e-6 < marker_zero_time);
    let span = (end - start).max(1.0e-6);
    Some(SyncMarkerIntervalPhase {
        from_marker: previous.marker_index,
        to_marker: next.marker_index,
        alpha: ((playback - start) / span).clamp(0.0, 1.0),
        semantic_cycle,
    })
}

fn map_marker_phase_raw(
    target_clip: &AnimationClip,
    target_track: &CompiledSyncMarkerTrack,
    phase: SyncMarkerIntervalPhase,
) -> Option<f32> {
    let target_duration = target_clip.duration_seconds.max(1.0e-6);
    let pair_index = target_track
        .markers
        .iter()
        .enumerate()
        .find_map(|(index, marker)| {
            let next = target_track.markers[(index + 1) % target_track.markers.len()];
            (marker.marker_index == phase.from_marker && next.marker_index == phase.to_marker)
                .then_some(index)
        })?;
    let start_marker = target_track.markers[pair_index];
    let next_index = (pair_index + 1) % target_track.markers.len();
    let next_marker = target_track.markers[next_index];
    let marker_zero_time = sync_marker_time(target_track, 0)?;
    let target_clip_cycle =
        phase.semantic_cycle + i64::from(start_marker.time_seconds + 1.0e-6 < marker_zero_time);
    let start = start_marker.time_seconds + target_clip_cycle as f32 * target_duration;
    let end_cycle = if next_index == 0 {
        target_clip_cycle + 1
    } else {
        target_clip_cycle
    };
    let end = next_marker.time_seconds + end_cycle as f32 * target_duration;
    Some(start + (end - start) * phase.alpha)
}

fn remap_marker_playback_time(
    source_clip: &AnimationClip,
    source_track: &CompiledSyncMarkerTrack,
    target_clip: &AnimationClip,
    target_track: &CompiledSyncMarkerTrack,
    source_playback_time_seconds: f32,
) -> Option<f32> {
    let phase =
        sync_marker_interval_phase(source_clip, source_track, source_playback_time_seconds)?;
    let raw = map_marker_phase_raw(target_clip, target_track, phase)?;
    // Marker rotations may make the mathematically corresponding target occurrence precede t=0.
    // Apply one constant whole-cycle offset chosen from the source origin so the remapped playback
    // clock stays non-negative and monotonic instead of wrapping backward at a marker boundary.
    let origin_phase = sync_marker_interval_phase(source_clip, source_track, 0.0)?;
    let origin_raw = map_marker_phase_raw(target_clip, target_track, origin_phase)?;
    let target_duration = target_clip.duration_seconds.max(1.0e-6);
    let cycle_offset = if origin_raw < 0.0 {
        (-origin_raw / target_duration).ceil()
    } else {
        0.0
    };
    Some(raw + cycle_offset * target_duration)
}

fn marker_matched_playback_time(
    graph: &CompiledAnimationGraph,
    marker_group_index: usize,
    source_clip_index: usize,
    target_clip_index: usize,
    source_playback_time_seconds: f32,
) -> Option<f32> {
    let source_track = graph
        .sync_marker_tracks
        .get(marker_group_index)?
        .get(source_clip_index)?
        .as_ref()?;
    let target_track = graph
        .sync_marker_tracks
        .get(marker_group_index)?
        .get(target_clip_index)?
        .as_ref()?;
    remap_marker_playback_time(
        &graph.clips[source_clip_index].clip,
        source_track,
        &graph.clips[target_clip_index].clip,
        target_track,
        source_playback_time_seconds,
    )
}

fn sync_matched_motion_time(
    graph: &CompiledAnimationGraph,
    source_motion: &CompiledAnimationMotion,
    source_motion_time: f32,
    target_motion: &CompiledAnimationMotion,
) -> Option<f32> {
    let source_sync = source_motion.sync()?;
    let target_sync = target_motion.sync()?;
    if source_sync.key != target_sync.key {
        return None;
    }
    let source_playback = source_motion_time * source_motion.first_speed();
    if let (Some(source_group), Some(target_group)) = (
        source_sync.marker_group_index,
        target_sync.marker_group_index,
    ) {
        if source_group == target_group {
            if let Some(target_playback) = marker_matched_playback_time(
                graph,
                source_group,
                source_motion.first_clip_index(),
                target_motion.first_clip_index(),
                source_playback,
            ) {
                return Some(target_playback / target_motion.first_speed().max(1.0e-6));
            }
        }
    }
    let phase = motion_unwrapped_phase(graph, source_motion, source_motion_time);
    let target_duration = motion_reference_duration(graph, target_motion);
    Some(phase * target_duration / target_motion.first_speed().max(1.0e-6))
}

fn synchronized_blend_sample_playback_time(
    graph: &CompiledAnimationGraph,
    leader_clip_index: usize,
    leader_speed: f32,
    target_clip_index: usize,
    target_speed: f32,
    sync: Option<&CompiledMotionSync>,
    motion_time_seconds: f32,
) -> f32 {
    let Some(sync) = sync else {
        return motion_time_seconds * target_speed;
    };
    let leader_playback = motion_time_seconds * leader_speed;
    if target_clip_index == leader_clip_index {
        return leader_playback;
    }
    if let Some(group_index) = sync.marker_group_index {
        if let Some(mapped) = marker_matched_playback_time(
            graph,
            group_index,
            leader_clip_index,
            target_clip_index,
            leader_playback,
        ) {
            return mapped;
        }
    }
    let leader_duration = graph.clips[leader_clip_index]
        .clip
        .duration_seconds
        .max(1.0e-6);
    let phase = leader_playback / leader_duration;
    phase
        * graph.clips[target_clip_index]
            .clip
            .duration_seconds
            .max(1.0e-6)
}

fn sample_playback_time(
    graph: &CompiledAnimationGraph,
    motion: &CompiledAnimationMotion,
    sample_index: usize,
    motion_time_seconds: f32,
) -> f32 {
    match motion {
        CompiledAnimationMotion::Clip { speed, .. } => motion_time_seconds * *speed,
        CompiledAnimationMotion::Blend1D { samples, sync, .. } => {
            let sample = samples[sample_index];
            synchronized_blend_sample_playback_time(
                graph,
                samples[0].clip_index,
                samples[0].speed,
                sample.clip_index,
                sample.speed,
                sync.as_ref(),
                motion_time_seconds,
            )
        }
        CompiledAnimationMotion::Blend2D { samples, sync, .. } => {
            let sample = samples[sample_index];
            synchronized_blend_sample_playback_time(
                graph,
                samples[0].clip_index,
                samples[0].speed,
                sample.clip_index,
                sample.speed,
                sync.as_ref(),
                motion_time_seconds,
            )
        }
    }
}

fn seek_motion_cursors(
    graph: &CompiledAnimationGraph,
    motion: &CompiledAnimationMotion,
    motion_time_seconds: f32,
    runtime: &mut MotionPlaybackRuntime,
) -> Result<(), String> {
    if runtime.cursors.len() != motion.sample_count() {
        return Err("animation graph motion cursor shape mismatch".to_owned());
    }
    for sample_index in 0..runtime.cursors.len() {
        let time = sample_playback_time(graph, motion, sample_index, motion_time_seconds);
        if time <= f32::EPSILON {
            runtime.cursors[sample_index].restart();
        } else {
            runtime.cursors[sample_index].seek(time)?;
        }
    }
    Ok(())
}

struct ClipEventEmission<'a> {
    graph: &'a CompiledAnimationGraph,
    clip_index: usize,
    playback_time_seconds: f32,
    emit: bool,
    source: AnimationGraphEventSource,
    blend_weight: f32,
}

fn append_clip_events(
    emission: ClipEventEmission<'_>,
    cursor: &mut AnimationEventCursor,
    event_scratch: &mut Vec<AnimationEventOccurrence>,
    out: &mut Vec<AnimationGraphEventOccurrence>,
) -> Result<(), String> {
    event_scratch.clear();
    let clip = &emission.graph.clips[emission.clip_index].clip;
    cursor.advance_prevalidated(clip, emission.playback_time_seconds, event_scratch)?;
    if emission.emit {
        out.extend(
            event_scratch
                .iter()
                .copied()
                .map(|occurrence| AnimationGraphEventOccurrence {
                    source: emission.source,
                    clip_index: emission.clip_index,
                    event_index: occurrence.event_index,
                    playback_time_seconds: occurrence.playback_time_seconds,
                    loop_index: occurrence.loop_index,
                    blend_weight: emission.blend_weight,
                }),
        );
    }
    Ok(())
}

struct MotionEvaluationContext<'a> {
    graph: &'a CompiledAnimationGraph,
    skeleton: &'a AnimationSkeletonRuntime,
    parameters: &'a [AnimationGraphParameterValue],
    source: AnimationGraphEventSource,
    emit_events: bool,
    source_weight: f32,
}

struct MotionEvaluationScratch<'a> {
    a: &'a mut Vec<JointLocalPose>,
    b: &'a mut Vec<JointLocalPose>,
    event_scratch: &'a mut Vec<AnimationEventOccurrence>,
    events: &'a mut Vec<AnimationGraphEventOccurrence>,
}
