use newengine_animation_api::{
    AnimationClipRef, AnimationTimelineEventQueueV1, AnimationTimelineEventV1,
};
use newengine_animation_runtime::{AnimationClip, AnimationEventCursor, AnimationEventOccurrence};
use newengine_engine_runtime::gameplay::{publish_gameplay_event, GameplayEvent};
use newengine_tags_api::TagId;

pub(crate) fn timeline_event(
    entity: newengine_ecs::EntityId,
    clip_ref: &str,
    channel: &str,
    clip: &AnimationClip,
    occurrence: AnimationEventOccurrence,
) -> Option<AnimationTimelineEventV1> {
    let marker = clip.events.get(occurrence.event_index)?;
    let parameters = serde_json::Value::Object(
        marker
            .parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.key.clone(),
                    serde_json::Value::String(parameter.value.clone()),
                )
            })
            .collect(),
    );
    Some(AnimationTimelineEventV1 {
        entity: entity.into(),
        clip: AnimationClipRef(clip_ref.to_owned()),
        channel: channel.to_owned(),
        tag: TagId::new(marker.tag.clone()),
        clip_time_seconds: marker.time_seconds,
        playback_time_seconds: occurrence.playback_time_seconds,
        loop_index: occurrence.loop_index,
        parameters,
    })
}

pub(crate) fn collect_timeline_events(
    entity: newengine_ecs::EntityId,
    clip_ref: &str,
    channel: &str,
    clip: &AnimationClip,
    cursor: &mut AnimationEventCursor,
    playback_time_seconds: f32,
    occurrence_scratch: &mut Vec<AnimationEventOccurrence>,
    out: &mut Vec<AnimationTimelineEventV1>,
) -> Result<usize, String> {
    occurrence_scratch.clear();
    let count = cursor.advance(clip, playback_time_seconds, occurrence_scratch)?;
    if count == 0 {
        return Ok(0);
    }
    out.reserve(count);
    for occurrence in occurrence_scratch.iter().copied() {
        if let Some(event) = timeline_event(entity, clip_ref, channel, clip, occurrence) {
            out.push(event);
        }
    }
    Ok(count)
}

pub(crate) fn publish_timeline_events(
    world: &mut newengine_ecs::World,
    events: Vec<AnimationTimelineEventV1>,
) {
    if events.is_empty() {
        return;
    }
    // Animation marker tags are project-authored semantic event ids. Publish them unchanged to
    // the generic gameplay bus; no native code infers a handler or asset from the marker name.
    for event in &events {
        let gameplay_event = GameplayEvent::new(event.tag.as_str().to_owned())
            .with_stable_source(event.entity.stable_id)
            .with_payload(serde_json::json!({
                "source_kind": "animation_timeline",
                "clip": event.clip.0,
                "channel": event.channel,
                "clip_time_seconds": event.clip_time_seconds,
                "playback_time_seconds": event.playback_time_seconds,
                "loop_index": event.loop_index,
                "parameters": event.parameters,
            }));
        if let Err(error) = publish_gameplay_event(world, gameplay_event) {
            newengine_ulog_api::ulog::warn!(
                "animation timeline event publish rejected tag='{}' entity={} err='{}'",
                event.tag.as_str(),
                event.entity.stable_id,
                error
            );
        }
    }

    if world.resource::<AnimationTimelineEventQueueV1>().is_none() {
        world.insert_resource(AnimationTimelineEventQueueV1::default());
    }
    if let Some(queue) = world.resource_mut::<AnimationTimelineEventQueueV1>() {
        queue.extend(events);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_animation_runtime::{AnimationEvent, AnimationEventParameter};

    #[test]
    fn timeline_event_preserves_semantic_tag_and_payload() {
        let clip = AnimationClip {
            name: "reload".to_owned(),
            skeleton_ref: String::new(),
            source: "test".to_owned(),
            duration_seconds: 1.0,
            sample_rate_hz: 30.0,
            looped: false,
            joint_tags: vec![0],
            events: vec![AnimationEvent {
                time_seconds: 0.4,
                tag: "weapon.mag.detach".to_owned(),
                parameters: vec![AnimationEventParameter {
                    key: "socket".to_owned(),
                    value: "magazine".to_owned(),
                }],
            }],
            poses: vec![newengine_animation_runtime::JointLocalPose {
                translation: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: Some([1.0, 1.0, 1.0]),
            }],
        };
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        let event = timeline_event(
            entity,
            "shared/weapon/reload.ycd@reload",
            "weapon.reload",
            &clip,
            AnimationEventOccurrence {
                event_index: 0,
                playback_time_seconds: 0.4,
                loop_index: 0,
            },
        )
        .expect("timeline event");
        assert_eq!(event.tag.as_str(), "weapon.mag.detach");
        assert_eq!(event.channel, "weapon.reload");
        assert_eq!(event.parameters["socket"], "magazine");
    }

    #[test]
    fn published_timeline_events_are_available_as_world_resource() {
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        let clip = AnimationClip {
            name: "fire".to_owned(),
            skeleton_ref: String::new(),
            source: "test".to_owned(),
            duration_seconds: 0.2,
            sample_rate_hz: 30.0,
            looped: false,
            joint_tags: vec![0],
            events: vec![AnimationEvent::new(0.0, "weapon.fire")],
            poses: vec![newengine_animation_runtime::JointLocalPose {
                translation: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: Some([1.0, 1.0, 1.0]),
            }],
        };
        let mut cursor = AnimationEventCursor::default();
        cursor.restart();
        let mut occurrences = Vec::new();
        let mut events = Vec::new();
        collect_timeline_events(
            entity,
            "shared/weapon/fire.ycd@fire",
            "weapon.fire",
            &clip,
            &mut cursor,
            0.0,
            &mut occurrences,
            &mut events,
        )
        .expect("collect event");
        publish_timeline_events(&mut world, events);
        let queue = world
            .resource::<AnimationTimelineEventQueueV1>()
            .expect("animation event queue resource");
        assert_eq!(queue.events.len(), 1);
        assert_eq!(queue.events[0].tag.as_str(), "weapon.fire");
        assert_eq!(queue.events[0].entity.stable_id, entity.stable_u64());
    }
}
