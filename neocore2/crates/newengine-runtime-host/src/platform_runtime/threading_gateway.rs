#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_core::ThreadPoolHandle;
use newengine_plugin_api::Blob;
use newengine_plugin_host::host_context::publish_event;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service_dynamic_best_effort, EngineGatewayProviderDeclDynamic,
    JsonServiceRouter,
};
use newengine_task_api::{
    task_method, EngineTaskControlAction, EngineTaskEnvelopeV1, EngineTaskEvent, EngineTaskPhase,
    TaskControlResponseV1, TaskExecutorKind, TaskIdRequestV1, TaskProgressEventV1,
    TaskQueueSnapshotJsonV1, TaskRunProcessStartRequestV1, TaskRunProcessStartedV1,
    TaskServiceCallAcceptedV1, TaskServiceCallRequestV1, TaskServiceInfoV1, TaskStartRequestV1,
    TaskStatusJsonV1, TaskTraceJsonV1, ENGINE_THREADING_SERVICE_ID, TASK_SERVICE_METHODS,
    THREADING_BACKEND_CAPABILITY_ID, THREADING_RUNTIME_CONTRACT, THREADING_SERVICE_ID,
};

#[path = "threading_gateway_parts/mod.rs"]
mod threading_gateway_parts;

const OWNER: &str = "newengine-runtime-host.threading-gateway";

#[derive(Clone)]
struct ThreadingGatewayState {
    thread_pool: ThreadPoolHandle,
    events: newengine_core::EventHub,
    process_results: ProcessResults,
}

use threading_gateway_parts::process_runner::{
    process_status_from_record, result_bin, submit_process_task, ProcessResults,
};
use threading_gateway_parts::service_call::submit_service_call_task;
use threading_gateway_parts::status::{missing_status, status_from_core};

fn publish_task_event(events: &newengine_core::EventHub, event: EngineTaskEvent) {
    let job_event = EngineTaskEnvelopeV1::new(
        event.clone(),
        TaskExecutorKind::ExternalProvider,
        "engine-threading-gateway",
    );
    if let Ok(payload) = serde_json::to_vec(&event) {
        let _ = publish_event(newengine_task_api::ENGINE_TASK_EVENT_TOPIC_V1, &payload);
    }
    if let Ok(payload) = serde_json::to_vec(&job_event) {
        let _ = publish_event(newengine_task_api::ENGINE_TASK_ENVELOPE_TOPIC_V1, &payload);
    }
    let _ = events.publish(event);
    let _ = events.publish(job_event);
}

fn invoke(state: &mut ThreadingGatewayState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let method = value
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or(task_method::SNAPSHOT_JSON_V1);
    match method {
        task_method::SNAPSHOT_JSON_V1 => ok_json(snapshot(state)),
        other => RResult::RErr(RString::from(format!(
            "engine.threading: unknown invoke method '{other}'"
        ))),
    }
}

fn snapshot(state: &mut ThreadingGatewayState) -> TaskQueueSnapshotJsonV1 {
    snapshot_from_pool(&state.thread_pool)
}

use threading_gateway_parts::snapshot::snapshot_from_pool;

#[derive(Clone, Copy)]
struct ThreadingGatewayIdentity {
    gateway: &'static str,
    service_kind: &'static str,
    service_id: &'static str,
    provider_route: &'static str,
    capability: &'static str,
    contract: &'static str,
    provider_name: &'static str,
    notes: &'static str,
}

const THREADING_GATEWAY_IDENTITY: ThreadingGatewayIdentity = ThreadingGatewayIdentity {
    gateway: ENGINE_THREADING_SERVICE_ID,
    service_kind: "threading",
    service_id: THREADING_SERVICE_ID,
    provider_route: "engine.threading.core",
    capability: THREADING_BACKEND_CAPABILITY_ID,
    contract: THREADING_RUNTIME_CONTRACT,
    provider_name: "ThreadPoolManager",
    notes: "Host-owned CPU execution gateway. Runtime systems must request worker execution and CPU budget through engine.threading instead of spawning private threads.",
};

fn service(
    state: ThreadingGatewayState,
    identity: ThreadingGatewayIdentity,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        identity.service_id,
        OWNER,
        identity.capability,
        TASK_SERVICE_METHODS.iter().copied(),
    )
    .protocol(identity.contract)
    .features([
        "job-lifecycle-events",
        "cooperative-cancel",
        "cooperative-pause-resume",
        "job-status-read-model",
        "event-bus-progress",
        "external-process-ticket-poll",
        "binary-result-readback",
        "domain-job-pass-metadata",
        "frame-dependency-correlation",
        "per-lane-snapshot",
    ])
    .gateway(identity.gateway)
    .notes(identity.notes);

    JsonServiceRouter::with_state(identity.service_id, state)
        .describe_json(&description)
        .info(move || TaskServiceInfoV1 {
            service_id: identity.service_id.to_owned(),
            gateway: identity.gateway.to_owned(),
            provider: identity.provider_name.to_owned(),
            contract: identity.contract.to_owned(),
            authority: newengine_task_api::TaskAuthorityV1 {
                gateway: identity.gateway.to_owned(),
                provider: identity.service_id.to_owned(),
                contract: identity.contract.to_owned(),
                ..Default::default()
            },
            ..Default::default()
        })
        .get_json(task_method::SNAPSHOT_JSON_V1, snapshot)
        .post_json::<TaskServiceCallRequestV1, TaskServiceCallAcceptedV1, _>(task_method::INVOKE_SERVICE_V1, |state, request| {
            submit_service_call_task(&state.thread_pool, request)
        })
        .post_json::<TaskIdRequestV1, TaskStatusJsonV1, _>(task_method::STATUS_JSON_V1, |state, request| {
            let task_id = request.task_id.trim();
            if let Some(status) = state.thread_pool.task_status(task_id).map(status_from_core) {
                return status;
            }
            if let Some(record) = state.process_results.lock().expect("engine.threading process_results mutex poisoned").get(task_id).cloned() {
                return process_status_from_record(request.task_id, record);
            }
            missing_status(request.task_id)
        })
        .post_json::<TaskRunProcessStartRequestV1, TaskRunProcessStartedV1, _>(task_method::RUN_PROCESS_START_V1, |state, request| {
            submit_process_task(&state.thread_pool, &state.process_results, request)
        })
        .blob(task_method::RESULT_BIN_V1, |state, payload| result_bin(&state.process_results, payload))
        .post_json::<TaskIdRequestV1, TaskControlResponseV1, _>(task_method::CANCEL_V1, |state, request| {
            let accepted = state.thread_pool.cancel_task(request.task_id.trim());
            let event = request.control_event(EngineTaskControlAction::Cancel);
            let _ = state.events.publish(event.clone());
            if let Ok(payload) = serde_json::to_vec(&event) {
                let _ = publish_event(newengine_task_api::ENGINE_TASK_CONTROL_TOPIC_V1, &payload);
            }
            TaskControlResponseV1 { task_id: request.task_id, action: "cancel".to_owned(), accepted }
        })
        .post_json::<TaskIdRequestV1, TaskControlResponseV1, _>(task_method::PAUSE_V1, |state, request| {
            let accepted = state.thread_pool.pause_task(request.task_id.trim());
            let event = request.control_event(EngineTaskControlAction::Pause);
            let _ = state.events.publish(event.clone());
            if let Ok(payload) = serde_json::to_vec(&event) {
                let _ = publish_event(newengine_task_api::ENGINE_TASK_CONTROL_TOPIC_V1, &payload);
            }
            TaskControlResponseV1 { task_id: request.task_id, action: "pause".to_owned(), accepted }
        })
        .post_json::<TaskIdRequestV1, TaskControlResponseV1, _>(task_method::RESUME_V1, |state, request| {
            let accepted = state.thread_pool.resume_task(request.task_id.trim());
            let event = request.control_event(EngineTaskControlAction::Resume);
            let _ = state.events.publish(event.clone());
            if let Ok(payload) = serde_json::to_vec(&event) {
                let _ = publish_event(newengine_task_api::ENGINE_TASK_CONTROL_TOPIC_V1, &payload);
            }
            TaskControlResponseV1 { task_id: request.task_id, action: "resume".to_owned(), accepted }
        })
        .post_json::<TaskProgressEventV1, EngineTaskEvent, _>(task_method::PROGRESS_EVENT_V1, |state, event| {
            let event = event.into_task_event();
            publish_task_event(&state.events, event.clone());
            event
        })
        .post_json::<TaskStartRequestV1, EngineTaskEvent, _>(task_method::START_V1, |state, request| {
            let mut event = EngineTaskEvent::new(
                request.task_id,
                "engine.threading",
                request.owner,
                request.category,
                request.name,
                request.lane,
                EngineTaskPhase::Scheduled,
                "Job scheduled",
                "External/runtime task announced through engine.threading.",
            )
            .with_controls(request.can_pause, request.can_cancel)
            .with_progress(0.0)
            .with_priority(request.priority)
            .with_task_pass(request.task_pass)
            .with_task_domain(request.task_domain)
            .with_executor("external-provider");
            if let Some(frame_id) = request.frame_id {
                event = event.with_frame_id(frame_id);
            }
            if !request.dependency_group.trim().is_empty() {
                event = event.with_dependency_group(request.dependency_group);
            }
            if event.task_id.trim().is_empty() {
                event.task_id = format!("external.job.{}", state.thread_pool.snapshot().submitted_jobs.saturating_add(1));
            }
            publish_task_event(&state.events, event.clone());
            event
        })
        .post_json::<TaskIdRequestV1, TaskTraceJsonV1, _>(task_method::TRACE_JSON_V1, |state, request| {
            let status = state.thread_pool.task_status(request.task_id.trim())
                .map(status_from_core)
                .unwrap_or_else(|| missing_status(request.task_id.clone()));
            TaskTraceJsonV1 {
                task_id: request.task_id,
                status,
                note: "Trace history is event-bus owned; subscribe to engine.task.event.v1 for full live task trace.".to_owned(),
            }
        })
        .blob(task_method::INVOKE_JSON, invoke)
        .blob(task_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

pub(crate) fn register_threading_gateway_service_best_effort(
    thread_pool: ThreadPoolHandle,
    events: newengine_core::EventHub,
) -> bool {
    if newengine_core::has_engine_gateway_route(ENGINE_THREADING_SERVICE_ID)
        || newengine_core::has_engine_gateway_route(THREADING_SERVICE_ID)
    {
        return true;
    }

    register_engine_gateway_provider_service_dynamic_best_effort(EngineGatewayProviderDeclDynamic {
        gateway: THREADING_GATEWAY_IDENTITY.gateway,
        service_kind: THREADING_GATEWAY_IDENTITY.service_kind,
        provider_service: THREADING_GATEWAY_IDENTITY.service_id,
        provider_route: THREADING_GATEWAY_IDENTITY.provider_route,
        capability: THREADING_GATEWAY_IDENTITY.capability,
        priority: 0,
        owner: OWNER,
        service: service(
            ThreadingGatewayState {
                thread_pool,
                events,
                process_results: ProcessResults::default(),
            },
            THREADING_GATEWAY_IDENTITY,
        ),
    })
}
