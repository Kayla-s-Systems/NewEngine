use std::collections::BTreeMap;

use newengine_ai_api::{
    AiDecisionTraceV1, AiFrameInputV1, AiFrameOutputV1, AiIntentDtoV1, AiIntentKind,
    AiServiceInfoV1, AiValidateIntentsRequestV1, AiValidateIntentsResponseV1,
};
use newengine_animation_api::{
    AnimationDescribeGraphsRequestV1, AnimationDescribeGraphsResponseV1, AnimationIntentDtoV1,
    AnimationPlanRequestV1, AnimationPlanResponseV1, AnimationServiceInfoV1,
    AnimationValidateIntentRequestV1, AnimationValidateIntentResponseV1,
};
use newengine_navigation_api::{
    NavPathDtoV1, NavPathPointV1, NavPlanPathRequestV1, NavPlanPathResponseV1,
    NavProjectPointRequestV1, NavProjectPointResponseV1, NavQueryStatusRequestV1,
    NavQueryStatusResponseV1, NavigationServiceInfoV1,
};
use newengine_tags_api::{
    TagDescriptorV1, TagId, TagsDescribeRequestV1, TagsDescribeResponseV1, TagsResolveRequestV1,
    TagsResolveResponseV1, TagsServiceInfoV1, TagsSnapshotRequestV1, TagsSnapshotResponseV1,
    TagsValidateSetRequestV1, TagsValidateSetResponseV1,
};
use newengine_tasks_api::{
    TaskDescriptorV1, TaskQueueSnapshotV1, TaskRequestDtoV1, TasksDescribeRequestV1,
    TasksDescribeResponseV1, TasksPlanQueueRequestV1, TasksPlanQueueResponseV1, TasksServiceInfoV1,
    TasksValidateRequestV1, TasksValidateResponseV1,
};

use crate::config;

#[derive(Debug, Clone)]
pub struct GameplayFoundationState {
    tags: BTreeMap<String, TagDescriptorV1>,
    tasks: BTreeMap<String, TaskDescriptorV1>,
    animation_graphs: Vec<newengine_animation_api::AnimationGraphDescriptorV1>,
}

impl Default for GameplayFoundationState {
    fn default() -> Self {
        let cfg = config::GameplayFoundationConfig::load();
        let tags = cfg
            .tags
            .iter()
            .map(|raw| {
                let descriptor = config::tag_descriptor(raw);
                (descriptor.tag.0.clone(), descriptor)
            })
            .collect();
        let tasks = cfg
            .tasks
            .iter()
            .map(|raw| {
                let descriptor = config::task_descriptor(raw);
                (descriptor.task.0.clone(), descriptor)
            })
            .collect();
        let animation_graphs = cfg
            .animation_graphs
            .iter()
            .map(config::animation_graph_descriptor)
            .collect();
        Self {
            tags,
            tasks,
            animation_graphs,
        }
    }
}

impl GameplayFoundationState {
    pub fn tags_info(&self) -> TagsServiceInfoV1 {
        TagsServiceInfoV1::default()
    }
    pub fn tasks_info(&self) -> TasksServiceInfoV1 {
        TasksServiceInfoV1::default()
    }
    pub fn animation_info(&self) -> AnimationServiceInfoV1 {
        AnimationServiceInfoV1::default()
    }
    pub fn navigation_info(&self) -> NavigationServiceInfoV1 {
        NavigationServiceInfoV1::default()
    }
    pub fn ai_info(&self) -> AiServiceInfoV1 {
        AiServiceInfoV1::default()
    }

    pub fn describe_tags(&self, req: TagsDescribeRequestV1) -> TagsDescribeResponseV1 {
        let domain_filter = req.domain_filter.unwrap_or_default().to_ascii_lowercase();
        let tags = self
            .tags
            .values()
            .filter(|tag| {
                domain_filter.is_empty()
                    || format!("{:?}", tag.domain).to_ascii_lowercase() == domain_filter
            })
            .cloned()
            .collect();
        TagsDescribeResponseV1 {
            accepted: true,
            tags,
            diagnostics: Vec::new(),
        }
    }

    pub fn resolve_tag(&self, req: TagsResolveRequestV1) -> TagsResolveResponseV1 {
        let descriptor = self.tags.get(req.tag.trim()).cloned();
        TagsResolveResponseV1 {
            accepted: descriptor.is_some(),
            descriptor,
            diagnostics: if req.tag.trim().is_empty() {
                vec!["tag is empty".to_owned()]
            } else {
                Vec::new()
            },
        }
    }

    pub fn tags_snapshot(&self, req: TagsSnapshotRequestV1) -> TagsSnapshotResponseV1 {
        let tags = self.tags.keys().cloned().map(TagId::new).collect();
        TagsSnapshotResponseV1 {
            accepted: true,
            sets: vec![newengine_tags_api::TagSetSnapshotV1 {
                owner: if req.owner_prefix.is_empty() {
                    "engine.tags.registry".to_owned()
                } else {
                    req.owner_prefix
                },
                entity: None,
                tags,
                source: "config/gameplay/gameplay_foundation.v1.json".to_owned(),
            }],
            diagnostics: Vec::new(),
        }
    }

    pub fn validate_tag_set(&self, req: TagsValidateSetRequestV1) -> TagsValidateSetResponseV1 {
        let mut normalized = Vec::new();
        let mut unknown = Vec::new();
        for tag in req.tags {
            if self.tags.contains_key(tag.as_str()) {
                normalized.push(tag);
            } else {
                unknown.push(tag);
            }
        }
        TagsValidateSetResponseV1 {
            accepted: unknown.is_empty(),
            normalized_tags: normalized,
            unknown_tags: unknown,
            diagnostics: Vec::new(),
        }
    }

    pub fn describe_tasks(&self, _req: TasksDescribeRequestV1) -> TasksDescribeResponseV1 {
        TasksDescribeResponseV1 {
            accepted: true,
            tasks: self.tasks.values().cloned().collect(),
            diagnostics: Vec::new(),
        }
    }

    pub fn validate_task(&self, req: TasksValidateRequestV1) -> TasksValidateResponseV1 {
        let known = self.tasks.contains_key(req.request.task.as_str());
        TasksValidateResponseV1 {
            accepted: known,
            normalized: known.then_some(req.request),
            diagnostics: if known {
                Vec::new()
            } else {
                vec!["unknown task".to_owned()]
            },
        }
    }

    pub fn plan_queue(&self, req: TasksPlanQueueRequestV1) -> TasksPlanQueueResponseV1 {
        let planned_queues = req
            .queues
            .into_iter()
            .map(|mut q: TaskQueueSnapshotV1| {
                q.pending.sort_by_key(|task| -task.priority);
                q
            })
            .collect();
        TasksPlanQueueResponseV1 {
            accepted: true,
            planned_queues,
            diagnostics: Vec::new(),
        }
    }

    pub fn describe_animation_graphs(
        &self,
        _req: AnimationDescribeGraphsRequestV1,
    ) -> AnimationDescribeGraphsResponseV1 {
        AnimationDescribeGraphsResponseV1 {
            accepted: true,
            graphs: self.animation_graphs.clone(),
            diagnostics: Vec::new(),
        }
    }

    pub fn plan_animation(&self, req: AnimationPlanRequestV1) -> AnimationPlanResponseV1 {
        AnimationPlanResponseV1 {
            accepted: true,
            accepted_intents: req.intents,
            diagnostics: Vec::new(),
        }
    }

    pub fn validate_animation_intent(
        &self,
        req: AnimationValidateIntentRequestV1,
    ) -> AnimationValidateIntentResponseV1 {
        AnimationValidateIntentResponseV1 {
            accepted: true,
            normalized: Some(req.intent),
            diagnostics: Vec::new(),
        }
    }

    pub fn plan_path(&self, req: NavPlanPathRequestV1) -> NavPlanPathResponseV1 {
        let path = NavPathDtoV1 {
            points: vec![
                NavPathPointV1 {
                    position: req.start,
                    flags: req.tags.clone(),
                },
                NavPathPointV1 {
                    position: req.goal,
                    flags: req.tags,
                },
            ],
            cost: ((req.goal.x - req.start.x).powi(2)
                + (req.goal.y - req.start.y).powi(2)
                + (req.goal.z - req.start.z).powi(2))
            .sqrt(),
            complete: true,
        };
        NavPlanPathResponseV1 {
            accepted: true,
            path: Some(path),
            diagnostics: vec![
                "foundation provider returned deterministic straight-line path DTO".to_owned(),
            ],
        }
    }

    pub fn project_point(&self, req: NavProjectPointRequestV1) -> NavProjectPointResponseV1 {
        NavProjectPointResponseV1 {
            accepted: true,
            projected: Some(req.point),
            diagnostics: Vec::new(),
        }
    }

    pub fn query_status(&self, req: NavQueryStatusRequestV1) -> NavQueryStatusResponseV1 {
        NavQueryStatusResponseV1 {
            accepted: true,
            status: if req.query_id.is_empty() {
                "ready".to_owned()
            } else {
                "known".to_owned()
            },
            diagnostics: Vec::new(),
        }
    }

    pub fn ai_frame(&self, input: AiFrameInputV1) -> AiFrameOutputV1 {
        let mut intents = Vec::new();
        let mut trace = Vec::new();
        for agent in input.agents {
            let alert = agent.tags.iter().any(|tag| tag.as_str() == "state.alert");
            let idle = agent.tags.iter().any(|tag| tag.as_str() == "state.idle");
            let kind = if alert {
                AiIntentKind::RequestTask
            } else {
                AiIntentKind::Idle
            };
            let task = if alert {
                Some(TaskRequestDtoV1 {
                    task: newengine_tasks_api::TaskId::new("move_to"),
                    issuer: Some(agent.entity),
                    target: None,
                    priority: 100,
                    parameters: serde_json::json!({ "reason": "alert-agent-foundation-intent" }),
                    tags: agent.tags.clone(),
                })
            } else {
                None
            };
            intents.push(AiIntentDtoV1 {
                intent_id: format!("ai.intent.{}.{}", input.fixed_tick, agent.agent_id),
                agent: agent.entity,
                kind,
                target_position: agent.position,
                path: None,
                task,
                animation: idle.then(|| AnimationIntentDtoV1 {
                    entity: agent.entity,
                    intent: newengine_animation_api::AnimationIntentKind::PlayClip,
                    graph: Some(newengine_animation_api::AnimationGraphRef(
                        "humanoid.locomotion".to_owned(),
                    )),
                    clip: Some(newengine_animation_api::AnimationClipRef("idle".to_owned())),
                    task: None,
                    tags: agent.tags.clone(),
                    parameters: serde_json::json!({}),
                }),
                tags: agent.tags.clone(),
                payload: serde_json::json!({ "apply_stage_required": true }),
            });
            trace.push(AiDecisionTraceV1 {
                agent: agent.entity,
                selected_pattern: if alert {
                    "foundation.alert.request_task".to_owned()
                } else {
                    "foundation.idle".to_owned()
                },
                score: if alert { 1.0 } else { 0.1 },
                notes: vec![
                    "AI emitted intent DTO only; runtime apply stage owns mutation.".to_owned(),
                ],
            });
        }
        AiFrameOutputV1 {
            accepted: true,
            fixed_tick: input.fixed_tick,
            intents,
            decision_trace: trace,
            diagnostics: Vec::new(),
        }
    }

    pub fn validate_ai_intents(
        &self,
        req: AiValidateIntentsRequestV1,
    ) -> AiValidateIntentsResponseV1 {
        AiValidateIntentsResponseV1 {
            accepted: true,
            intents: req.intents,
            rejected: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
