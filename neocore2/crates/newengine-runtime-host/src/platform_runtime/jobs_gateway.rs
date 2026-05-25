#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_core::{JobLane, JobPriority, JobRequest, JobSystemHandle, JobTaskStatus};
use newengine_jobs_api::{
    jobs_method, EngineJobEventV1, JobControlResponseV1, JobExecutorKind, JobIdRequestV1,
    JobProgressEventV1, JobServiceCallAcceptedV1, JobServiceCallRequestV1, JobStartRequestV1,
    JobsServiceInfoV1, JobsSnapshotJsonV1, JobStatusJsonV1, JobTraceJsonV1,
    ENGINE_JOBS_SERVICE_ID, JOBS_BACKEND_CAPABILITY_ID, JOBS_RUNTIME_CONTRACT, JOBS_SERVICE_ID,
    JOBS_SERVICE_METHODS, EngineTaskControlAction, EngineTaskEvent, EngineTaskPhase,
};
use newengine_plugin_api::Blob;
use newengine_plugin_host::host_context::publish_event;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_owned_gateway_service_dynamic_best_effort, EngineOwnedGatewayDeclDynamic,
    JsonServiceRouter,
};

const OWNER: &str = "newengine-runtime-host.jobs-gateway";

#[derive(Clone)]
struct JobsGatewayState {
    jobs: JobSystemHandle,
    events: newengine_core::EventHub,
}

fn status_from_core(status: JobTaskStatus) -> JobStatusJsonV1 {
    JobStatusJsonV1 {
        job_id: status.task_id,
        name: status.label.to_owned(),
        lane: status.lane.as_str().to_owned(),
        priority: status.priority.as_str().to_owned(),
        phase: status.phase,
        can_pause: status.can_pause,
        can_cancel: status.can_cancel,
        cancel_requested: status.cancel_requested,
        pause_requested: status.pause_requested,
        found: true,
    }
}

fn missing_status(job_id: impl Into<String>) -> JobStatusJsonV1 {
    JobStatusJsonV1 { job_id: job_id.into(), found: false, ..Default::default() }
}

fn publish_task_event(events: &newengine_core::EventHub, event: EngineTaskEvent) {
    let job_event = EngineJobEventV1::new(
        event.clone(),
        JobExecutorKind::ExternalProvider,
        "engine-jobs-gateway",
    );
    if let Ok(payload) = serde_json::to_vec(&event) {
        let _ = publish_event(newengine_jobs_api::ENGINE_TASK_EVENT_TOPIC_V1, &payload);
    }
    if let Ok(payload) = serde_json::to_vec(&job_event) {
        let _ = publish_event(newengine_jobs_api::ENGINE_JOB_EVENT_TOPIC_V1, &payload);
    }
    let _ = events.publish(event);
    let _ = events.publish(job_event);
}

fn invoke(state: &mut JobsGatewayState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let method = value
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or(jobs_method::SNAPSHOT_JSON_V1);
    match method {
        jobs_method::SNAPSHOT_JSON_V1 => ok_json(snapshot(state)),
        other => RResult::RErr(RString::from(format!("engine.jobs: unknown invoke method '{other}'"))),
    }
}

fn snapshot(state: &mut JobsGatewayState) -> JobsSnapshotJsonV1 {
    let snapshot = state.jobs.snapshot();
    JobsSnapshotJsonV1 {
        worker_threads: snapshot.worker_threads,
        pending_jobs: snapshot.pending_jobs,
        running_jobs: snapshot.running_jobs,
        paused_jobs: snapshot.paused_jobs,
        submitted_jobs: snapshot.submitted_jobs,
        completed_jobs: snapshot.completed_jobs,
        cancelled_jobs: snapshot.cancelled_jobs,
        panicked_jobs: snapshot.panicked_jobs,
    }
}


fn lane_from_str(value: &str) -> JobLane {
    match value.trim().to_ascii_lowercase().as_str() {
        "simulation" => JobLane::Simulation,
        "render-prep" | "render_prep" | "renderprep" => JobLane::RenderPrep,
        "streaming" => JobLane::Streaming,
        "asset-io" | "asset_io" | "asset" => JobLane::AssetIo,
        "plugin" | "plugins" => JobLane::Plugin,
        "background" | "bg" => JobLane::Background,
        _ => JobLane::Plugin,
    }
}

fn priority_from_str(value: &str) -> JobPriority {
    match value.trim().to_ascii_lowercase().as_str() {
        "critical" => JobPriority::Critical,
        "interactive" => JobPriority::Interactive,
        "normal" => JobPriority::Normal,
        "background" | "bg" => JobPriority::Background,
        _ => JobPriority::Background,
    }
}

fn profiler_job_request(request: &JobServiceCallRequestV1) -> JobRequest {
    let label = match request.category.as_str() {
        "profiler.report.flush" => "profiler.report.flush",
        _ => "service.call",
    };
    let source = "engine.jobs.service-call";
    let owner = match request.owner.as_str() {
        "profiler.api" => "profiler.api",
        _ => "engine.jobs",
    };
    let category = match request.category.as_str() {
        "profiler.report.flush" => "profiler.report.flush",
        _ => "service-call",
    };

    let mut job = JobRequest::new(label)
        .with_source(source)
        .with_owner(owner)
        .with_category(category)
        .with_lane(lane_from_str(request.lane.as_str()))
        .with_priority(priority_from_str(request.priority.as_str()))
        .pausable(request.can_pause)
        .cancellable(request.can_cancel);
    if !request.job_id.trim().is_empty() {
        job = job.with_task_id(request.job_id.trim().to_owned());
    }
    job
}

fn submit_service_call_job(state: &mut JobsGatewayState, request: JobServiceCallRequestV1) -> JobServiceCallAcceptedV1 {
    let target_gateway = request.target.gateway.trim().to_owned();
    let target_method = request.target.method.trim().to_owned();
    let payload_json = request.target.payload_json.clone();
    let job = profiler_job_request(&request);
    let requested_job_id = request.job_id.trim().to_owned();

    if target_gateway.is_empty() || target_method.is_empty() {
        return JobServiceCallAcceptedV1 {
            job_id: requested_job_id,
            accepted: false,
            gateway: target_gateway,
            method: target_method,
            status: "rejected".to_owned(),
            detail: "job.invoke_service_v1 requires target.gateway and target.method".to_owned(),
        };
    }

    let ticket = state.jobs.submit_controlled(job, move |control| {
        if !control.checkpoint() {
            control.publish_progress(
                1.0,
                "Service call job cancelled",
                "Task was cancelled before invoking the target service.",
            );
            return;
        }

        control.publish_progress(
            0.20,
            "Invoking target service",
            format!("Calling {target_gateway}/{target_method} through engine job worker."),
        );

        let payload = match serde_json::to_vec(&payload_json) {
            Ok(bytes) => Blob::from(bytes),
            Err(e) => {
                control.publish_progress(1.0, "Service call job failed", format!("Failed to serialize target payload: {e}"));
                panic!("engine.jobs service-call payload serialization failed: {e}");
            }
        };

        let response = newengine_plugin_host::call_service_v1(
            newengine_plugin_api::CapabilityId::from(target_gateway.as_str()),
            newengine_plugin_api::MethodName::from(target_method.as_str()),
            payload,
        );

        match response.into_result() {
            Ok(blob) => {
                control.publish_progress(
                    1.0,
                    "Target service completed",
                    format!("Service call completed output_bytes={}", blob.len()),
                );
            }
            Err(e) => {
                control.publish_progress(1.0, "Target service failed", e.to_string());
                panic!("engine.jobs service-call target failed: {e}");
            }
        }
    });

    JobServiceCallAcceptedV1 {
        job_id: ticket.task_id().to_owned(),
        accepted: true,
        gateway: request.target.gateway,
        method: request.target.method,
        status: "scheduled".to_owned(),
        detail: "Service call scheduled on the engine-owned job system; no plugin-owned background worker was created.".to_owned(),
    }
}

fn service(state: JobsGatewayState) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        JOBS_SERVICE_ID,
        OWNER,
        JOBS_BACKEND_CAPABILITY_ID,
        JOBS_SERVICE_METHODS.iter().copied(),
    )
    .protocol(JOBS_RUNTIME_CONTRACT)
    .features([
        "job-lifecycle-events",
        "cooperative-cancel",
        "cooperative-pause-resume",
        "job-status-read-model",
        "event-bus-progress",
    ])
    .gateway("engine.jobs")
    .notes("Runtime job/task gateway. Every long-running engine operation should have a JobId and publish progress through engine.task.event.v1.");

    JsonServiceRouter::with_state(JOBS_SERVICE_ID, state)
        .describe_json(&description)
        .info(JobsServiceInfoV1::default)
        .get_json(jobs_method::SNAPSHOT_JSON_V1, snapshot)
        .post_json::<JobServiceCallRequestV1, JobServiceCallAcceptedV1, _>(jobs_method::INVOKE_SERVICE_V1, |state, request| {
            submit_service_call_job(state, request)
        })
        .post_json::<JobIdRequestV1, JobStatusJsonV1, _>(jobs_method::STATUS_JSON_V1, |state, request| {
            state.jobs.task_status(request.job_id.trim())
                .map(status_from_core)
                .unwrap_or_else(|| missing_status(request.job_id))
        })
        .post_json::<JobIdRequestV1, JobControlResponseV1, _>(jobs_method::CANCEL_V1, |state, request| {
            let accepted = state.jobs.cancel_task(request.job_id.trim());
            let event = request.control_event(EngineTaskControlAction::Cancel);
            let _ = state.events.publish(event.clone());
            if let Ok(payload) = serde_json::to_vec(&event) {
                let _ = publish_event(newengine_jobs_api::ENGINE_TASK_CONTROL_TOPIC_V1, &payload);
            }
            JobControlResponseV1 { job_id: request.job_id, action: "cancel".to_owned(), accepted }
        })
        .post_json::<JobIdRequestV1, JobControlResponseV1, _>(jobs_method::PAUSE_V1, |state, request| {
            let accepted = state.jobs.pause_task(request.job_id.trim());
            let event = request.control_event(EngineTaskControlAction::Pause);
            let _ = state.events.publish(event.clone());
            if let Ok(payload) = serde_json::to_vec(&event) {
                let _ = publish_event(newengine_jobs_api::ENGINE_TASK_CONTROL_TOPIC_V1, &payload);
            }
            JobControlResponseV1 { job_id: request.job_id, action: "pause".to_owned(), accepted }
        })
        .post_json::<JobIdRequestV1, JobControlResponseV1, _>(jobs_method::RESUME_V1, |state, request| {
            let accepted = state.jobs.resume_task(request.job_id.trim());
            let event = request.control_event(EngineTaskControlAction::Resume);
            let _ = state.events.publish(event.clone());
            if let Ok(payload) = serde_json::to_vec(&event) {
                let _ = publish_event(newengine_jobs_api::ENGINE_TASK_CONTROL_TOPIC_V1, &payload);
            }
            JobControlResponseV1 { job_id: request.job_id, action: "resume".to_owned(), accepted }
        })
        .post_json::<JobProgressEventV1, EngineTaskEvent, _>(jobs_method::PROGRESS_EVENT_V1, |state, event| {
            let event = event.into_task_event();
            publish_task_event(&state.events, event.clone());
            event
        })
        .post_json::<JobStartRequestV1, EngineTaskEvent, _>(jobs_method::START_V1, |state, request| {
            let mut event = EngineTaskEvent::new(
                request.job_id,
                "engine.jobs",
                request.owner,
                request.category,
                request.name,
                request.lane,
                EngineTaskPhase::Scheduled,
                "Job scheduled",
                "External/runtime job announced through engine.jobs.",
            ).with_controls(request.can_pause, request.can_cancel).with_progress(0.0);
            if event.task_id.trim().is_empty() {
                event.task_id = format!("external.job.{}", state.jobs.snapshot().submitted_jobs.saturating_add(1));
            }
            publish_task_event(&state.events, event.clone());
            event
        })
        .post_json::<JobIdRequestV1, JobTraceJsonV1, _>(jobs_method::TRACE_JSON_V1, |state, request| {
            let status = state.jobs.task_status(request.job_id.trim())
                .map(status_from_core)
                .unwrap_or_else(|| missing_status(request.job_id.clone()));
            JobTraceJsonV1 {
                job_id: request.job_id,
                status,
                note: "Trace history is event-bus owned; subscribe to engine.task.event.v1 for full live trace.".to_owned(),
            }
        })
        .blob(jobs_method::INVOKE_JSON, invoke)
        .blob(jobs_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

pub(crate) fn register_jobs_gateway_service_best_effort(
    jobs: JobSystemHandle,
    events: newengine_core::EventHub,
) -> bool {
    if newengine_core::has_engine_gateway_route(ENGINE_JOBS_SERVICE_ID) || newengine_core::has_engine_gateway_route(JOBS_SERVICE_ID) {
        return true;
    }
    register_engine_owned_gateway_service_dynamic_best_effort(EngineOwnedGatewayDeclDynamic {
        gateway: ENGINE_JOBS_SERVICE_ID,
        service_kind: "jobs",
        provider_service: JOBS_SERVICE_ID,
        capability: JOBS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: OWNER,
        service: service(JobsGatewayState { jobs, events }),
    })
}
