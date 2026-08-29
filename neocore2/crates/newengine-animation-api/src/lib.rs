#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable DTO contract for the `engine.animation` gateway.

use newengine_entity_api::EntityHandle;
use newengine_tags_api::TagId;
use newengine_tasks_api::TaskId;
use serde::{Deserialize, Serialize};

pub const ENGINE_ANIMATION_SERVICE_ID: &str = "engine.animation";
pub const ANIMATION_SERVICE_ID: &str = "animation.api";
pub const ANIMATION_BACKEND_CAPABILITY_ID: &str = "animation.backend";
pub const ANIMATION_RUNTIME_CONTRACT: &str = "newengine.animation-api/v1";

pub mod animation_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const DESCRIBE_GRAPHS_JSON_V1: &str = "animation.describe_graphs_json_v1";
    pub const PLAN_JSON_V1: &str = "animation.plan_json_v1";
    pub const VALIDATE_INTENT_JSON_V1: &str = "animation.validate_intent_json_v1";
}

pub const ANIMATION_SERVICE_METHODS: &[&str] = &[
    animation_method::INFO_JSON,
    animation_method::INVOKE_JSON,
    animation_method::SHUTDOWN_V1,
    animation_method::DESCRIBE_GRAPHS_JSON_V1,
    animation_method::PLAN_JSON_V1,
    animation_method::VALIDATE_INTENT_JSON_V1,
];

pub const ANIMATION_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "animation",
        ENGINE_ANIMATION_SERVICE_ID,
        ANIMATION_SERVICE_ID,
        ANIMATION_BACKEND_CAPABILITY_ID,
    );

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AnimationGraphRef(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AnimationClipRef(pub String);

/// Runtime occurrence of an authored animation timeline marker.
///
/// The DTO intentionally carries semantic tag/payload data only. Pose buffers, clip ownership,
/// skeleton bindings and backend-specific event cursors remain private to animation runtimes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationTimelineEventV1 {
    pub entity: EntityHandle,
    pub clip: AnimationClipRef,
    #[serde(default)]
    pub channel: String,
    pub tag: TagId,
    pub clip_time_seconds: f32,
    pub playback_time_seconds: f32,
    #[serde(default)]
    pub loop_index: u64,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// Frame/runtime-owned timeline event queue. Consumers drain semantic occurrences without owning
/// animation runtime state. A bounded queue prevents an authored malformed loop from growing memory
/// without limit even if the consumer temporarily stops draining it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimationTimelineEventQueueV1 {
    #[serde(default)]
    pub events: Vec<AnimationTimelineEventV1>,
}

impl AnimationTimelineEventQueueV1 {
    pub const MAX_RETAINED_EVENTS: usize = 1024;

    #[inline]
    pub fn emit(&mut self, event: AnimationTimelineEventV1) {
        if self.events.len() >= Self::MAX_RETAINED_EVENTS {
            let overflow = self.events.len() + 1 - Self::MAX_RETAINED_EVENTS;
            self.events.drain(0..overflow);
        }
        self.events.push(event);
    }

    #[inline]
    pub fn drain(&mut self) -> Vec<AnimationTimelineEventV1> {
        std::mem::take(&mut self.events)
    }
}

impl Extend<AnimationTimelineEventV1> for AnimationTimelineEventQueueV1 {
    fn extend<T: IntoIterator<Item = AnimationTimelineEventV1>>(&mut self, iter: T) {
        for event in iter {
            self.emit(event);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum AnimationIntentKind {
    #[default]
    PlayClip,
    Stop,
    BlendToState,
    SetParameter,
    AttachTask,
    Custom(String),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationIntentDtoV1 {
    pub entity: EntityHandle,
    #[serde(default)]
    pub intent: AnimationIntentKind,
    #[serde(default)]
    pub graph: Option<AnimationGraphRef>,
    #[serde(default)]
    pub clip: Option<AnimationClipRef>,
    #[serde(default)]
    pub task: Option<TaskId>,
    #[serde(default)]
    pub tags: Vec<TagId>,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationGraphDescriptorV1 {
    pub graph: AnimationGraphRef,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub tags: Vec<TagId>,
    #[serde(default)]
    pub states: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimationDescribeGraphsRequestV1 {
    #[serde(default)]
    pub tag_filter: Vec<TagId>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimationDescribeGraphsResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub graphs: Vec<AnimationGraphDescriptorV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimationPlanRequestV1 {
    #[serde(default)]
    pub intents: Vec<AnimationIntentDtoV1>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimationPlanResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub accepted_intents: Vec<AnimationIntentDtoV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationValidateIntentRequestV1 {
    pub intent: AnimationIntentDtoV1,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimationValidateIntentResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub normalized: Option<AnimationIntentDtoV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationServiceInfoV1 {
    pub protocol: String,
    pub provider: String,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

impl Default for AnimationServiceInfoV1 {
    fn default() -> Self {
        Self {
            protocol: ANIMATION_RUNTIME_CONTRACT.to_owned(),
            provider: "engine.animation.foundation".to_owned(),
            methods: ANIMATION_SERVICE_METHODS
                .iter()
                .map(|it| (*it).to_owned())
                .collect(),
            features: vec!["animation-intents".to_owned(), "task-bindings".to_owned()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(index: usize) -> AnimationTimelineEventV1 {
        AnimationTimelineEventV1 {
            entity: EntityHandle::new(7),
            clip: AnimationClipRef("test.ycd@idle".to_owned()),
            channel: "character.locomotion".to_owned(),
            tag: TagId::new(format!("event.{index}")),
            clip_time_seconds: 0.0,
            playback_time_seconds: index as f32,
            loop_index: 0,
            parameters: serde_json::Value::Null,
        }
    }

    #[test]
    fn timeline_queue_is_bounded_and_drains() {
        let mut queue = AnimationTimelineEventQueueV1::default();
        queue.extend((0..=AnimationTimelineEventQueueV1::MAX_RETAINED_EVENTS).map(event));
        assert_eq!(
            queue.events.len(),
            AnimationTimelineEventQueueV1::MAX_RETAINED_EVENTS
        );
        assert_eq!(queue.events[0].tag.as_str(), "event.1");
        let drained = queue.drain();
        assert_eq!(
            drained.len(),
            AnimationTimelineEventQueueV1::MAX_RETAINED_EVENTS
        );
        assert!(queue.events.is_empty());
    }
}
