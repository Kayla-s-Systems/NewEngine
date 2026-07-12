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

const ENGINE_JOBS_GATEWAY_ID: &str = "engine.jobs";
const JOBS_COMPAT_SERVICE_KIND: &str = "jobs";
const JOBS_COMPAT_PROVIDER_ROUTE: &str = "engine.jobs.core";

mod job_compat_method {
    pub const START_V1: &str = "job.start_v1";
    pub const RUN_PROCESS_START_V1: &str = "job.run_process_start_v1";
    pub const RESULT_BIN_V1: &str = "job.result_bin_v1";
    pub const INVOKE_SERVICE_V1: &str = "job.invoke_service_v1";
    pub const CANCEL_V1: &str = "job.cancel_v1";
    pub const PAUSE_V1: &str = "job.pause_v1";
    pub const RESUME_V1: &str = "job.resume_v1";
    pub const STATUS_JSON_V1: &str = "job.status_json_v1";
    pub const PROGRESS_EVENT_V1: &str = "job.progress_event_v1";
    pub const TRACE_JSON_V1: &str = "job.trace_json_v1";
    pub const SNAPSHOT_JSON_V1: &str = "job.snapshot_json_v1";
}

const JOB_COMPAT_SERVICE_METHODS: &[&str] = &[
    job_compat_method::START_V1,
    job_compat_method::RUN_PROCESS_START_V1,
    job_compat_method::RESULT_BIN_V1,
    job_compat_method::INVOKE_SERVICE_V1,
    job_compat_method::CANCEL_V1,
    job_compat_method::PAUSE_V1,
    job_compat_method::RESUME_V1,
    job_compat_method::STATUS_JSON_V1,
    job_compat_method::PROGRESS_EVENT_V1,
    job_compat_method::TRACE_JSON_V1,
    job_compat_method::SNAPSHOT_JSON_V1,
];

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

fn invoke_service(
    state: &mut ThreadingGatewayState,
    request: TaskServiceCallRequestV1,
) -> TaskServiceCallAcceptedV1 {
    submit_service_call_task(&state.thread_pool, request)
}

fn task_status(state: &mut ThreadingGatewayState, request: TaskIdRequestV1) -> TaskStatusJsonV1 {
    let task_id = request.task_id.trim();
    if let Some(status) = state.thread_pool.task_status(task_id).map(status_from_core) {
        return status;
    }
    if let Some(record) = state
        .process_results
        .lock()
        .expect("engine.threading process_results mutex poisoned")
        .get(task_id)
        .cloned()
    {
        return process_status_from_record(request.task_id, record);
    }
    missing_status(request.task_id)
}

fn run_process_start(
    state: &mut ThreadingGatewayState,
    request: TaskRunProcessStartRequestV1,
) -> TaskRunProcessStartedV1 {
    submit_process_task(&state.thread_pool, &state.process_results, request)
}

fn task_result_bin(state: &mut ThreadingGatewayState, payload: Blob) -> RResult<Blob, RString> {
    result_bin(&state.process_results, payload)
}

fn cancel_task(
    state: &mut ThreadingGatewayState,
    request: TaskIdRequestV1,
) -> TaskControlResponseV1 {
    let accepted = state.thread_pool.cancel_task(request.task_id.trim());
    let event = request.control_event(EngineTaskControlAction::Cancel);
    let _ = state.events.publish(event.clone());
    if let Ok(payload) = serde_json::to_vec(&event) {
        let _ = publish_event(newengine_task_api::ENGINE_TASK_CONTROL_TOPIC_V1, &payload);
    }
    let task_id = request.task_id;
    TaskControlResponseV1 {
        task_id: task_id.clone(),
        job_id: task_id,
        action: "cancel".to_owned(),
        accepted,
    }
}

fn pause_task(
    state: &mut ThreadingGatewayState,
    request: TaskIdRequestV1,
) -> TaskControlResponseV1 {
    let accepted = state.thread_pool.pause_task(request.task_id.trim());
    let event = request.control_event(EngineTaskControlAction::Pause);
    let _ = state.events.publish(event.clone());
    if let Ok(payload) = serde_json::to_vec(&event) {
        let _ = publish_event(newengine_task_api::ENGINE_TASK_CONTROL_TOPIC_V1, &payload);
    }
    let task_id = request.task_id;
    TaskControlResponseV1 {
        task_id: task_id.clone(),
        job_id: task_id,
        action: "pause".to_owned(),
        accepted,
    }
}

fn resume_task(
    state: &mut ThreadingGatewayState,
    request: TaskIdRequestV1,
) -> TaskControlResponseV1 {
    let accepted = state.thread_pool.resume_task(request.task_id.trim());
    let event = request.control_event(EngineTaskControlAction::Resume);
    let _ = state.events.publish(event.clone());
    if let Ok(payload) = serde_json::to_vec(&event) {
        let _ = publish_event(newengine_task_api::ENGINE_TASK_CONTROL_TOPIC_V1, &payload);
    }
    let task_id = request.task_id;
    TaskControlResponseV1 {
        task_id: task_id.clone(),
        job_id: task_id,
        action: "resume".to_owned(),
        accepted,
    }
}

fn progress_event(
    state: &mut ThreadingGatewayState,
    event: TaskProgressEventV1,
) -> EngineTaskEvent {
    let event = event.into_task_event();
    publish_task_event(&state.events, event.clone());
    event
}

fn start_task(state: &mut ThreadingGatewayState, request: TaskStartRequestV1) -> EngineTaskEvent {
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
        event.task_id = format!(
            "external.job.{}",
            state
                .thread_pool
                .snapshot()
                .submitted_jobs
                .saturating_add(1)
        );
    }
    publish_task_event(&state.events, event.clone());
    event
}

fn task_trace(state: &mut ThreadingGatewayState, request: TaskIdRequestV1) -> TaskTraceJsonV1 {
    let status = state
        .thread_pool
        .task_status(request.task_id.trim())
        .map(status_from_core)
        .unwrap_or_else(|| missing_status(request.task_id.clone()));
    let task_id = request.task_id;
    TaskTraceJsonV1 {
        task_id: task_id.clone(),
        job_id: task_id,
        status,
        note: "Trace history is event-bus owned; subscribe to engine.task.event.v1 for full live task trace."
            .to_owned(),
    }
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
        TASK_SERVICE_METHODS
            .iter()
            .copied()
            .chain(JOB_COMPAT_SERVICE_METHODS.iter().copied()),
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
        "engine-jobs-gateway-compat",
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
        .get_json(job_compat_method::SNAPSHOT_JSON_V1, snapshot)
        .post_json::<TaskServiceCallRequestV1, TaskServiceCallAcceptedV1, _>(
            task_method::INVOKE_SERVICE_V1,
            invoke_service,
        )
        .post_json::<TaskServiceCallRequestV1, TaskServiceCallAcceptedV1, _>(
            job_compat_method::INVOKE_SERVICE_V1,
            invoke_service,
        )
        .post_json::<TaskIdRequestV1, TaskStatusJsonV1, _>(task_method::STATUS_JSON_V1, task_status)
        .post_json::<TaskIdRequestV1, TaskStatusJsonV1, _>(
            job_compat_method::STATUS_JSON_V1,
            task_status,
        )
        .post_json::<TaskRunProcessStartRequestV1, TaskRunProcessStartedV1, _>(
            task_method::RUN_PROCESS_START_V1,
            run_process_start,
        )
        .post_json::<TaskRunProcessStartRequestV1, TaskRunProcessStartedV1, _>(
            job_compat_method::RUN_PROCESS_START_V1,
            run_process_start,
        )
        .blob(task_method::RESULT_BIN_V1, task_result_bin)
        .blob(job_compat_method::RESULT_BIN_V1, task_result_bin)
        .post_json::<TaskIdRequestV1, TaskControlResponseV1, _>(task_method::CANCEL_V1, cancel_task)
        .post_json::<TaskIdRequestV1, TaskControlResponseV1, _>(
            job_compat_method::CANCEL_V1,
            cancel_task,
        )
        .post_json::<TaskIdRequestV1, TaskControlResponseV1, _>(task_method::PAUSE_V1, pause_task)
        .post_json::<TaskIdRequestV1, TaskControlResponseV1, _>(
            job_compat_method::PAUSE_V1,
            pause_task,
        )
        .post_json::<TaskIdRequestV1, TaskControlResponseV1, _>(task_method::RESUME_V1, resume_task)
        .post_json::<TaskIdRequestV1, TaskControlResponseV1, _>(
            job_compat_method::RESUME_V1,
            resume_task,
        )
        .post_json::<TaskProgressEventV1, EngineTaskEvent, _>(
            task_method::PROGRESS_EVENT_V1,
            progress_event,
        )
        .post_json::<TaskProgressEventV1, EngineTaskEvent, _>(
            job_compat_method::PROGRESS_EVENT_V1,
            progress_event,
        )
        .post_json::<TaskStartRequestV1, EngineTaskEvent, _>(task_method::START_V1, start_task)
        .post_json::<TaskStartRequestV1, EngineTaskEvent, _>(
            job_compat_method::START_V1,
            start_task,
        )
        .post_json::<TaskIdRequestV1, TaskTraceJsonV1, _>(task_method::TRACE_JSON_V1, task_trace)
        .post_json::<TaskIdRequestV1, TaskTraceJsonV1, _>(
            job_compat_method::TRACE_JSON_V1,
            task_trace,
        )
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
        return register_jobs_gateway_alias_best_effort();
    }

    let registered = register_engine_gateway_provider_service_dynamic_best_effort(
        EngineGatewayProviderDeclDynamic {
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
        },
    );

    registered && register_jobs_gateway_alias_best_effort()
}

fn register_jobs_gateway_alias_best_effort() -> bool {
    if newengine_core::has_engine_gateway_route(ENGINE_JOBS_GATEWAY_ID) {
        return true;
    }

    if !newengine_plugin_host::has_service(THREADING_SERVICE_ID) {
        newengine_ulog_api::ulog::warn!(
            "engine-runtime jobs route skipped gateway='{}' provider_service='{}' reason='threading service missing'",
            ENGINE_JOBS_GATEWAY_ID,
            THREADING_SERVICE_ID
        );
        return false;
    }

    match newengine_plugin_host::register_engine_gateway_provider_route(
        ENGINE_JOBS_GATEWAY_ID,
        JOBS_COMPAT_SERVICE_KIND,
        THREADING_SERVICE_ID,
        JOBS_COMPAT_PROVIDER_ROUTE,
        THREADING_BACKEND_CAPABILITY_ID,
        0,
        OWNER,
    ) {
        Ok(()) => {
            newengine_ulog_api::ulog::info!(
                "engine-runtime jobs route registered gateway='{}' provider_route='{}' provider_service='{}' owner='{}' policy='visible gateway alias to engine.threading'",
                ENGINE_JOBS_GATEWAY_ID,
                JOBS_COMPAT_PROVIDER_ROUTE,
                THREADING_SERVICE_ID,
                OWNER
            );
            true
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "engine-runtime jobs route skipped gateway='{}' provider_route='{}' provider_service='{}' owner='{}' err='{}'",
                ENGINE_JOBS_GATEWAY_ID,
                JOBS_COMPAT_PROVIDER_ROUTE,
                THREADING_SERVICE_ID,
                OWNER,
                e
            );
            false
        }
    }
}
