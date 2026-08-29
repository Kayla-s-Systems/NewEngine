#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_json, payload_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use newengine_tags_api::TagId;
use newengine_tasks_api::{
    tasks_method, TaskDescriptorV1, TaskKind, TaskQueueSnapshotV1, TaskRequestDtoV1,
    TasksDescribeRequestV1, TasksDescribeResponseV1, TasksPlanQueueRequestV1,
    TasksPlanQueueResponseV1, TasksServiceInfoV1, TasksValidateRequestV1, TasksValidateResponseV1,
    TASKS_BACKEND_CAPABILITY_ID, TASKS_SERVICE_ID, TASKS_SERVICE_METHODS,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub const PROVIDER_ROUTE: &str = "engine.tasks.foundation";
const OWNER: &str = "newengine-tasks-runtime.foundation-provider";
const CONFIG_JSON: &str = include_str!("../../../config/gameplay/gameplay_foundation.v1.json");

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default)]
    tasks: Vec<RawTask>,
}
#[derive(Debug, Deserialize)]
struct RawTask {
    task: String,
    #[serde(default)]
    kind: Value,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    required_parameters: Vec<String>,
    #[serde(default)]
    description: String,
}

#[derive(Default)]
struct TasksState {
    tasks: BTreeMap<String, TaskDescriptorV1>,
}
impl TasksState {
    fn load() -> Self {
        let config: Config =
            serde_json::from_str(CONFIG_JSON).unwrap_or(Config { tasks: Vec::new() });
        let tasks = config
            .tasks
            .into_iter()
            .map(|raw| {
                let descriptor = TaskDescriptorV1 {
                    task: newengine_tasks_api::TaskId::new(raw.task),
                    kind: parse_kind(&raw.kind),
                    display_name: raw.display_name,
                    tags: raw.tags.into_iter().map(TagId::new).collect(),
                    required_parameters: raw.required_parameters,
                    description: raw.description,
                };
                (descriptor.task.0.clone(), descriptor)
            })
            .collect();
        Self { tasks }
    }
    fn info(&self) -> TasksServiceInfoV1 {
        TasksServiceInfoV1::default()
    }
    fn describe(&self, _: TasksDescribeRequestV1) -> TasksDescribeResponseV1 {
        TasksDescribeResponseV1 {
            accepted: true,
            tasks: self.tasks.values().cloned().collect(),
            diagnostics: Vec::new(),
        }
    }
    fn validate(&self, req: TasksValidateRequestV1) -> TasksValidateResponseV1 {
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
    fn plan(&self, req: TasksPlanQueueRequestV1) -> TasksPlanQueueResponseV1 {
        let planned_queues = req
            .queues
            .into_iter()
            .map(|mut queue: TaskQueueSnapshotV1| {
                queue
                    .pending
                    .sort_by_key(|task: &TaskRequestDtoV1| -task.priority);
                queue
            })
            .collect();
        TasksPlanQueueResponseV1 {
            accepted: true,
            planned_queues,
            diagnostics: Vec::new(),
        }
    }
}
fn parse_kind(value: &Value) -> TaskKind {
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
fn envelope(payload: &Blob, default_method: &str) -> Result<(String, Value), RString> {
    let value = payload_json(payload).map_err(RString::from)?;
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or(default_method)
        .to_owned();
    Ok((method, value.get("request").cloned().unwrap_or(Value::Null)))
}
fn invoke(state: &mut TasksState, payload: Blob) -> RResult<Blob, RString> {
    let (method, request) = match envelope(&payload, tasks_method::DESCRIBE_TASKS_JSON_V1) {
        Ok(v) => v,
        Err(e) => return RResult::RErr(e),
    };
    let decode = |e: serde_json::Error| RResult::RErr(RString::from(e.to_string()));
    match method.as_str() {
        tasks_method::INFO_JSON => ok_json(state.info()),
        tasks_method::DESCRIBE_TASKS_JSON_V1 => match serde_json::from_value(request) {
            Ok(v) => ok_json(state.describe(v)),
            Err(e) => decode(e),
        },
        tasks_method::VALIDATE_TASK_JSON_V1 => match serde_json::from_value(request) {
            Ok(v) => ok_json(state.validate(v)),
            Err(e) => decode(e),
        },
        tasks_method::PLAN_QUEUE_JSON_V1 => match serde_json::from_value(request) {
            Ok(v) => ok_json(state.plan(v)),
            Err(e) => decode(e),
        },
        other => RResult::RErr(RString::from(format!(
            "tasks.api: unknown invoke method '{other}'"
        ))),
    }
}
pub fn register_tasks_gateway_best_effort() -> bool {
    let description = engine_gateway_provider_service_description(
        TASKS_SERVICE_ID, PROVIDER_ROUTE, TASKS_BACKEND_CAPABILITY_ID, TASKS_SERVICE_METHODS.iter().copied(),
    ).gateway("engine.tasks")
     .protocol("newengine.tasks.foundation/v1")
     .features(["single-purpose-provider", "replaceable-gateway-route", "dto-only-boundary"])
     .notes("Owns only baseline task catalog/queue planning; no tags registry, animation, navigation, AI, or world mutation.");
    let service = JsonServiceRouter::with_state(TASKS_SERVICE_ID, TasksState::load())
        .describe_json(&description)
        .get_json(tasks_method::INFO_JSON, |state| state.info())
        .post_json(tasks_method::DESCRIBE_TASKS_JSON_V1, |state, req| {
            state.describe(req)
        })
        .post_json(tasks_method::VALIDATE_TASK_JSON_V1, |state, req| {
            state.validate(req)
        })
        .post_json(tasks_method::PLAN_QUEUE_JSON_V1, |state, req| {
            state.plan(req)
        })
        .blob(tasks_method::INVOKE_JSON, invoke)
        .shutdown()
        .into_service_v1();
    register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: "engine.tasks",
        service_kind: newengine_service_api::EngineServiceKind::Tasks,
        provider_service: TASKS_SERVICE_ID,
        provider_route: PROVIDER_ROUTE,
        capability: TASKS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: OWNER,
        service,
    })
    .is_ok()
}

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.tasks",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_tasks_api::TASKS_BACKEND_CAPABILITY_ID],
        &[],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let _ = register_tasks_gateway_best_effort();
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_loads_task_domain_only() {
        assert!(!TasksState::load().tasks.is_empty());
    }
}
