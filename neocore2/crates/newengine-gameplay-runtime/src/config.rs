use newengine_animation_api::{AnimationGraphDescriptorV1, AnimationGraphRef};
use newengine_tags_api::{TagDescriptorV1, TagDomain, TagId};
use newengine_tasks_api::{TaskDescriptorV1, TaskId, TaskKind};
use serde::Deserialize;

const CONFIG_JSON: &str = include_str!("../../../config/gameplay/gameplay_foundation.v1.json");

#[derive(Debug, Deserialize)]
pub(crate) struct GameplayFoundationConfig {
    #[serde(default)]
    pub tags: Vec<RawTag>,
    #[serde(default)]
    pub tasks: Vec<RawTask>,
    #[serde(default)]
    pub animation_graphs: Vec<RawAnimationGraph>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawTag {
    pub tag: String,
    #[serde(default)]
    pub domain: serde_json::Value,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawTask {
    pub task: String,
    #[serde(default)]
    pub kind: serde_json::Value,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub required_parameters: Vec<String>,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawAnimationGraph {
    pub graph: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub states: Vec<String>,
}

impl GameplayFoundationConfig {
    pub(crate) fn load() -> Self {
        serde_json::from_str(CONFIG_JSON).unwrap_or_else(|_| Self {
            tags: Vec::new(),
            tasks: Vec::new(),
            animation_graphs: Vec::new(),
        })
    }
}

pub(crate) fn raw_domain(value: &serde_json::Value) -> TagDomain {
    match value.as_str().unwrap_or_default() {
        "Gameplay" => TagDomain::Gameplay,
        "State" => TagDomain::State,
        "Faction" => TagDomain::Faction,
        "Item" => TagDomain::Item,
        "Weapon" => TagDomain::Weapon,
        "Mission" => TagDomain::Mission,
        "Animation" => TagDomain::Animation,
        "Navigation" => TagDomain::Navigation,
        "Debug" => TagDomain::Debug,
        other if !other.is_empty() => TagDomain::Custom(other.to_owned()),
        _ => TagDomain::Gameplay,
    }
}

pub(crate) fn raw_task_kind(value: &serde_json::Value) -> TaskKind {
    match value.as_str().unwrap_or_default() {
        "MoveTo" => TaskKind::MoveTo,
        "Wait" => TaskKind::Wait,
        "PlayAnimation" => TaskKind::PlayAnimation,
        "AttachEntity" => TaskKind::AttachEntity,
        "RequestDialogue" => TaskKind::RequestDialogue,
        "ClaimResource" => TaskKind::ClaimResource,
        other if !other.is_empty() => TaskKind::Custom(other.to_owned()),
        _ => TaskKind::Custom("unknown".to_owned()),
    }
}

pub(crate) fn tag_descriptor(raw: &RawTag) -> TagDescriptorV1 {
    TagDescriptorV1 {
        tag: TagId::new(raw.tag.clone()),
        domain: raw_domain(&raw.domain),
        display_name: raw.display_name.clone(),
        description: raw.description.clone(),
        parent: None,
        aliases: Vec::new(),
    }
}

pub(crate) fn task_descriptor(raw: &RawTask) -> TaskDescriptorV1 {
    TaskDescriptorV1 {
        task: TaskId::new(raw.task.clone()),
        kind: raw_task_kind(&raw.kind),
        display_name: raw.display_name.clone(),
        tags: raw.tags.iter().cloned().map(TagId::new).collect(),
        required_parameters: raw.required_parameters.clone(),
        description: raw.description.clone(),
    }
}

pub(crate) fn animation_graph_descriptor(raw: &RawAnimationGraph) -> AnimationGraphDescriptorV1 {
    AnimationGraphDescriptorV1 {
        graph: AnimationGraphRef(raw.graph.clone()),
        display_name: raw.display_name.clone(),
        tags: raw.tags.iter().cloned().map(TagId::new).collect(),
        states: raw.states.clone(),
    }
}
