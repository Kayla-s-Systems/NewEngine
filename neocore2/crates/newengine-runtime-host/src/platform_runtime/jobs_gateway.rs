#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_core::{JobLane, JobPriority, JobRequest, JobSystemHandle, JobTaskStatus};
use newengine_jobs_api::{
    jobs_method, EngineJobEventV1, JobControlResponseV1, JobExecutorKind, JobIdRequestV1,
    JobProgressEventV1, JobRunProcessStartRequestV1, JobRunProcessStartedV1,
    JobServiceCallAcceptedV1, JobServiceCallRequestV1, JobStartRequestV1,
    JobsLaneSnapshotJsonV1, JobsServiceInfoV1, JobsSnapshotJsonV1, JobStatusJsonV1, JobTraceJsonV1,
    ENGINE_JOBS_SERVICE_ID, JOBS_BACKEND_CAPABILITY_ID, JOBS_RUNTIME_CONTRACT, JOBS_SERVICE_ID,
    JOBS_SERVICE_METHODS, EngineTaskControlAction, EngineTaskEvent, EngineTaskPhase,
};
use newengine_plugin_api::Blob;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;
use std::sync::{Arc, Mutex};
use newengine_plugin_host::host_context::publish_event;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service_dynamic_best_effort, EngineGatewayProviderDeclDynamic,
    JsonServiceRouter,
};

const OWNER: &str = "newengine-runtime-host.jobs-gateway";

#[derive(Clone)]
struct JobsGatewayState {
    jobs: JobSystemHandle,
    events: newengine_core::EventHub,
    process_results: Arc<Mutex<HashMap<String, ProcessResultRecord>>>,
}

#[derive(Clone, Debug)]
struct ProcessResultRecord {
    phase: EngineTaskPhase,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    result_bytes: Option<Vec<u8>>,
    error: Option<String>,
    lane: String,
    priority: String,
    frame_id: Option<u64>,
    dependency_group: String,
    job_domain: String,
    job_pass: String,
    can_cancel: bool,
}

impl ProcessResultRecord {
    fn running_from_process_request(request: &JobRunProcessStartRequestV1) -> Self {
        let owner = match request.owner.as_str() {
            "engine.render" => "engine.render",
            "vulkan_renderer" => "vulkan_renderer",
            _ => "engine.jobs",
        };
        let job_pass = job_pass_from_category(request.category.as_str(), "process").to_owned();
        Self {
            phase: EngineTaskPhase::Running,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            result_bytes: None,
            error: None,
            lane: lane_from_str(request.lane.as_str()).as_str().to_owned(),
            priority: priority_from_str(request.priority.as_str()).as_str().to_owned(),
            frame_id: request.frame_id,
            dependency_group: if request.dependency_group.trim().is_empty() {
                format!("{}.process", job_pass)
            } else {
                request.dependency_group.trim().to_owned()
            },
            job_domain: job_domain_from_request(request.job_domain.as_str(), owner).to_owned(),
            job_pass,
            can_cancel: request.can_cancel,
        }
    }
}


fn status_from_core(status: JobTaskStatus) -> JobStatusJsonV1 {
    JobStatusJsonV1 {
        job_id: status.task_id,
        name: status.label.to_owned(),
        lane: status.lane.as_str().to_owned(),
        priority: status.priority.as_str().to_owned(),
        frame_id: status.frame_id,
        dependency_group: status.dependency_group.unwrap_or_default(),
        job_pass: status.job_pass.to_owned(),
        job_domain: status.job_domain.to_owned(),
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
    let mut lanes = std::collections::BTreeMap::new();
    for lane in [
        JobLane::Simulation,
        JobLane::RenderPrep,
        JobLane::Streaming,
        JobLane::AssetIo,
        JobLane::Plugin,
        JobLane::Background,
    ] {
        lanes.insert(
            lane.as_str().to_owned(),
            JobsLaneSnapshotJsonV1 {
                pending_jobs: snapshot.pending_for_lane(lane),
                running_jobs: snapshot.running_for_lane(lane),
                completed_jobs: snapshot.completed_for_lane(lane),
            },
        );
    }
    JobsSnapshotJsonV1 {
        worker_threads: snapshot.worker_threads,
        pending_jobs: snapshot.pending_jobs,
        running_jobs: snapshot.running_jobs,
        paused_jobs: snapshot.paused_jobs,
        submitted_jobs: snapshot.submitted_jobs,
        completed_jobs: snapshot.completed_jobs,
        cancelled_jobs: snapshot.cancelled_jobs,
        panicked_jobs: snapshot.panicked_jobs,
        lanes,
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


fn job_domain_from_request(value: &str, fallback_owner: &str) -> &'static str {
    match value.trim() {
        "engine.render" | "vulkan_renderer" => "engine.render",
        "engine.assets" | "asset_manager" => "engine.assets",
        "engine.simulation" | "newengine-sim" => "engine.simulation",
        "profiler.api" => "engine.profiler",
        _ if fallback_owner == "engine.render" => "engine.render",
        _ if fallback_owner == "profiler.api" => "engine.profiler",
        _ => "engine.jobs",
    }
}

fn job_pass_from_category(category: &str, fallback: &str) -> &'static str {
    match category.trim() {
        "shader.compile" => "shader-compile",
        "shader.validate" => "shader-validate",
        "texture.decode" | "asset-decode" => "texture-decode",
        "profiler.report.flush" => "profiler-flush",
        "service-call" => "service-call",
        "tool.process" => "tool-process",
        _ if fallback == "process" => "process",
        _ => "runtime",
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
        .with_job_domain(job_domain_from_request(request.job_domain.as_str(), owner))
        .with_job_pass(job_pass_from_category(request.category.as_str(), "service-call"))
        .pausable(request.can_pause)
        .cancellable(request.can_cancel);
    if let Some(frame_id) = request.frame_id {
        job = job.with_frame_id(frame_id);
    }
    if !request.dependency_group.trim().is_empty() {
        job = job.with_dependency_group(request.dependency_group.trim().to_owned());
    }
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
        detail: "Service call scheduled on the engine-runtime job system; no plugin-owned background worker was created.".to_owned(),
    }
}


fn process_job_request(request: &JobRunProcessStartRequestV1) -> JobRequest {
    let label = match request.category.as_str() {
        "shader.compile" => "shader.compile",
        "shader.validate" => "shader.validate",
        "tool.process" => "tool.process",
        _ => "external.process",
    };
    let owner = match request.owner.as_str() {
        "engine.render" => "engine.render",
        "vulkan_renderer" => "vulkan_renderer",
        _ => "engine.jobs",
    };
    let category = match request.category.as_str() {
        "shader.compile" => "shader.compile",
        "shader.validate" => "shader.validate",
        _ => "tool.process",
    };
    let mut job = JobRequest::new(label)
        .with_source("engine.jobs.process")
        .with_owner(owner)
        .with_category(category)
        .with_lane(lane_from_str(request.lane.as_str()))
        .with_priority(priority_from_str(request.priority.as_str()))
        .with_job_domain(job_domain_from_request(request.job_domain.as_str(), owner))
        .with_job_pass(job_pass_from_category(request.category.as_str(), "process"))
        .pausable(false)
        .cancellable(request.can_cancel);
    if let Some(frame_id) = request.frame_id {
        job = job.with_frame_id(frame_id);
    }
    if !request.dependency_group.trim().is_empty() {
        job = job.with_dependency_group(request.dependency_group.trim().to_owned());
    }
    if !request.job_id.trim().is_empty() {
        job = job.with_task_id(request.job_id.trim().to_owned());
    }
    job
}

fn submit_process_job(state: &mut JobsGatewayState, request: JobRunProcessStartRequestV1) -> JobRunProcessStartedV1 {
    let executable = request.executable.trim().to_owned();
    let requested_job_id = request.job_id.trim().to_owned();
    if executable.is_empty() {
        return JobRunProcessStartedV1 {
            job_id: requested_job_id,
            accepted: false,
            status: "rejected".to_owned(),
            detail: "job.run_process_start_v1 requires executable".to_owned(),
            result_path: request.result_path,
        };
    }

    let job = process_job_request(&request);
    let args = request.args.clone();
    let cwd = request.cwd.trim().to_owned();
    let env = request.env.clone();
    let result_path = request.result_path.trim().to_owned();
    let results = state.process_results.clone();
    let base_record = ProcessResultRecord::running_from_process_request(&request);

    let ticket = state.jobs.submit_controlled(job, move |control| {
        let job_id = control.task_id().to_owned();
        results.lock().expect("engine.jobs process_results mutex poisoned").insert(job_id.clone(), base_record.clone());
        if !control.checkpoint() {
            results.lock().expect("engine.jobs process_results mutex poisoned").insert(job_id.clone(), ProcessResultRecord {
                phase: EngineTaskPhase::Cancelled,
                error: Some("process job cancelled before spawn".to_owned()),
                ..base_record.clone()
            });
            return;
        }
        control.publish_progress(
            0.10,
            "Starting external process",
            format!("Launching tracked process executable='{}' args={}.", executable, args.len()),
        );
        // no-hidden-thread-scan: engine.jobs owns ToolJobRunner process spawning; callers receive a JobId and poll result/status.
        // External process execution is intentionally centralized behind engine.jobs;
        // render/tool consumers receive a JobId and must poll instead of blocking a frame caller.
        let mut command = Command::new(&executable);
        command.args(&args);
        if !cwd.is_empty() {
            command.current_dir(PathBuf::from(&cwd));
        }
        for (key, value) in &env {
            command.env(key, value);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                results.lock().expect("engine.jobs process_results mutex poisoned").insert(job_id.clone(), ProcessResultRecord {
                    phase: EngineTaskPhase::Failed,
                    error: Some(format!("process spawn failed executable='{}' err='{e}'", executable)),
                    ..base_record.clone()
                });
                control.publish_progress(1.0, "External process failed", format!("Spawn failed: {e}"));
                return;
            }
        };

        let started = Instant::now();
        if !control.checkpoint() {
            let _ = child.kill();
            let _ = child.wait();
            results.lock().expect("engine.jobs process_results mutex poisoned").insert(job_id.clone(), ProcessResultRecord {
                phase: EngineTaskPhase::Cancelled,
                error: Some(format!("process job cancelled executable='{}' elapsed_ms={}", executable, started.elapsed().as_millis())),
                ..base_record.clone()
            });
            control.publish_progress(1.0, "External process cancelled", "Child process was killed before process wait began.");
            return;
        }

        control.publish_progress(
            0.50,
            "External process running",
            format!("Process wait armed executable='{}' policy='os-process-completion-event'", executable),
        );

        let output = match child.wait_with_output() {
            Ok(output) => output,
            Err(e) => {
                results.lock().expect("engine.jobs process_results mutex poisoned").insert(job_id.clone(), ProcessResultRecord {
                    phase: EngineTaskPhase::Failed,
                    error: Some(format!("process wait/read failed executable='{}' err='{e}'", executable)),
                    ..base_record.clone()
                });
                control.publish_progress(1.0, "External process failed", format!("Wait/read failed: {e}"));
                return;
            }
        };
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let result_bytes = if !result_path.is_empty() {
            match std::fs::read(&result_path) {
                Ok(bytes) => Some(bytes),
                Err(e) => {
                    results.lock().expect("engine.jobs process_results mutex poisoned").insert(job_id.clone(), ProcessResultRecord {
                        phase: EngineTaskPhase::Failed,
                        exit_code: output.status.code(),
                        stdout,
                        stderr,
                        result_bytes: None,
                        error: Some(format!("process result read failed path='{}' err='{e}'", result_path)),
                        ..base_record.clone()
                    });
                    control.publish_progress(1.0, "External process failed", "Process completed but result file could not be read.");
                    return;
                }
            }
        } else {
            Some(output.stdout.clone())
        };
        let phase = if output.status.success() { EngineTaskPhase::Completed } else { EngineTaskPhase::Failed };
        let error = if output.status.success() { None } else { Some(format!("process exited with status {}", output.status)) };
        results.lock().expect("engine.jobs process_results mutex poisoned").insert(job_id.clone(), ProcessResultRecord {
            phase,
            exit_code: output.status.code(),
            stdout,
            stderr,
            result_bytes,
            error,
            ..base_record.clone()
        });
        control.publish_progress(
            1.0,
            if output.status.success() { "External process completed" } else { "External process failed" },
            format!("Process exited status={} result_path='{}' elapsed_ms={}", output.status, result_path, started.elapsed().as_millis()),
        );
    });

    JobRunProcessStartedV1 {
        job_id: ticket.task_id().to_owned(),
        accepted: true,
        status: "scheduled".to_owned(),
        detail: "External process scheduled on engine.jobs; caller must poll job.status_json_v1 and job.result_bin_v1.".to_owned(),
        result_path: request.result_path,
    }
}

fn result_bin(state: &mut JobsGatewayState, payload: Blob) -> RResult<Blob, RString> {
    let request: JobIdRequestV1 = match serde_json::from_slice(payload.as_slice()) {
        Ok(request) => request,
        Err(e) => return RResult::RErr(RString::from(format!("job.result_bin_v1 invalid request json: {e}"))),
    };
    let job_id = request.job_id.trim();
    let Some(record) = state.process_results.lock().expect("engine.jobs process_results mutex poisoned").get(job_id).cloned() else {
        return RResult::RErr(RString::from(format!("job.result_bin_v1 job_id='{job_id}' has no process result")));
    };
    match record.phase {
        EngineTaskPhase::Completed => RResult::ROk(Blob::from(record.result_bytes.unwrap_or_default())),
        EngineTaskPhase::Failed => RResult::RErr(RString::from(format!(
            "job.result_bin_v1 job_id='{job_id}' failed exit_code={:?} error='{}' stdout='{}' stderr='{}'",
            record.exit_code,
            record.error.unwrap_or_default(),
            record.stdout,
            record.stderr,
        ))),
        other => RResult::RErr(RString::from(format!("job.result_bin_v1 job_id='{job_id}' not ready phase={other:?}"))),
    }
}

fn service(state: JobsGatewayState) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
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
        "external-process-ticket-poll",
        "binary-result-readback",
        "domain-job-pass-metadata",
        "frame-dependency-correlation",
        "per-lane-snapshot",
    ])
    .gateway("engine.jobs")
    .notes("Runtime job/task gateway. Every long-running engine operation should have a JobId and publish progress through engine.task.event.v1; external tool processes must use start/poll/result instead of blocking runtime callers.");

    JsonServiceRouter::with_state(JOBS_SERVICE_ID, state)
        .describe_json(&description)
        .info(JobsServiceInfoV1::default)
        .get_json(jobs_method::SNAPSHOT_JSON_V1, snapshot)
        .post_json::<JobServiceCallRequestV1, JobServiceCallAcceptedV1, _>(jobs_method::INVOKE_SERVICE_V1, |state, request| {
            submit_service_call_job(state, request)
        })
        .post_json::<JobIdRequestV1, JobStatusJsonV1, _>(jobs_method::STATUS_JSON_V1, |state, request| {
            let job_id = request.job_id.trim();
            if let Some(status) = state.jobs.task_status(job_id).map(status_from_core) {
                return status;
            }
            if let Some(record) = state.process_results.lock().expect("engine.jobs process_results mutex poisoned").get(job_id).cloned() {
                return JobStatusJsonV1 {
                    job_id: request.job_id,
                    name: "external-process".to_owned(),
                    lane: record.lane,
                    priority: record.priority,
                    frame_id: record.frame_id,
                    dependency_group: record.dependency_group,
                    job_domain: record.job_domain,
                    job_pass: record.job_pass,
                    phase: record.phase,
                    can_pause: false,
                    can_cancel: record.can_cancel,
                    found: true,
                    ..Default::default()
                };
            }
            missing_status(request.job_id)
        })
        .post_json::<JobRunProcessStartRequestV1, JobRunProcessStartedV1, _>(jobs_method::RUN_PROCESS_START_V1, |state, request| {
            submit_process_job(state, request)
        })
        .blob(jobs_method::RESULT_BIN_V1, result_bin)
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
            )
            .with_controls(request.can_pause, request.can_cancel)
            .with_progress(0.0)
            .with_priority(request.priority)
            .with_job_pass(request.job_pass)
            .with_job_domain(request.job_domain)
            .with_executor("external-provider");
            if let Some(frame_id) = request.frame_id {
                event = event.with_frame_id(frame_id);
            }
            if !request.dependency_group.trim().is_empty() {
                event = event.with_dependency_group(request.dependency_group);
            }
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
    register_engine_gateway_provider_service_dynamic_best_effort(EngineGatewayProviderDeclDynamic {
        gateway: ENGINE_JOBS_SERVICE_ID,
        service_kind: "jobs",
        provider_service: JOBS_SERVICE_ID,
        provider_route: "engine.jobs.forge",
        capability: JOBS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: OWNER,
        service: service(JobsGatewayState { jobs, events, process_results: Arc::new(Mutex::new(HashMap::new())) }),
    })
}
