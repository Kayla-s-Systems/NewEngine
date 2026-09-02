use newengine_core::{TaskRequest, ThreadPoolHandle};
use newengine_plugin_api::Blob;
use newengine_task_api::{TaskServiceCallAcceptedV1, TaskServiceCallRequestV1};

use super::mapping::{
    lane_from_str, priority_from_str, task_domain_from_request, task_pass_from_category,
};

fn service_call_task_request(request: &TaskServiceCallRequestV1) -> TaskRequest {
    let label = match request.category.as_str() {
        "profiler.report.flush" => "profiler.report.flush",
        _ => "service.call",
    };
    let source = "engine.threading.service-call";
    let owner = match request.owner.as_str() {
        "profiler.api" => "profiler.api",
        _ => "engine.threading",
    };
    let category = match request.category.as_str() {
        "profiler.report.flush" => "profiler.report.flush",
        _ => "service-call",
    };

    let mut job = TaskRequest::new(label)
        .with_source(source)
        .with_owner(owner)
        .with_category(category)
        .with_lane(lane_from_str(request.lane.as_str()))
        .with_priority(priority_from_str(request.priority.as_str()))
        .with_task_domain(task_domain_from_request(
            request.task_domain.as_str(),
            owner,
        ))
        .with_task_pass(task_pass_from_category(
            request.category.as_str(),
            "service-call",
        ))
        .pausable(request.can_pause)
        .cancellable(request.can_cancel);
    if let Some(frame_id) = request.frame_id {
        job = job.with_frame_id(frame_id);
    }
    if !request.dependency_group.trim().is_empty() {
        job = job.with_dependency_group(request.dependency_group.trim().to_owned());
    }
    if !request.task_id.trim().is_empty() {
        job = job.with_task_id(request.task_id.trim().to_owned());
    }
    job
}

fn resolve_target_service_id(target_gateway: &str) -> Result<String, String> {
    if target_gateway.starts_with("engine.") {
        return newengine_core::resolve_service_for_engine_gateway(target_gateway).ok_or_else(
            || {
                format!(
                    "engine gateway '{}' has no active provider route in the current composition",
                    target_gateway
                )
            },
        );
    }
    Ok(target_gateway.to_owned())
}

pub(crate) fn submit_service_call_task(
    thread_pool: &ThreadPoolHandle,
    request: TaskServiceCallRequestV1,
) -> TaskServiceCallAcceptedV1 {
    let target_gateway = request.target.gateway.trim().to_owned();
    let target_method = request.target.method.trim().to_owned();
    let payload_json = request.target.payload_json.clone();
    let job = service_call_task_request(&request);
    let requested_task_id = request.task_id.trim().to_owned();

    if target_gateway.is_empty() || target_method.is_empty() {
        return TaskServiceCallAcceptedV1 {
            task_id: requested_task_id.clone(),
            job_id: requested_task_id,
            accepted: false,
            gateway: target_gateway,
            method: target_method,
            status: "rejected".to_owned(),
            detail: "task.invoke_service_v1 requires target.gateway and target.method".to_owned(),
        };
    }

    // HostContext is deliberately thread-local. Engine worker threads must never
    // construct an implicit empty context: capture the submitting Engine instance
    // and re-bind it only for this service-call task.
    let host_context = newengine_plugin_host::current_host_context();
    let ticket = thread_pool.submit_controlled(job, move |control| {
        newengine_plugin_host::with_host_context(&host_context, || {
            if !control.checkpoint() {
                control.publish_progress(
                    1.0,
                    "Service call task cancelled",
                    "Task was cancelled before invoking the target service.",
                );
                return;
            }

            let target_service = match resolve_target_service_id(&target_gateway) {
                Ok(service_id) => service_id,
                Err(error) => {
                    control.publish_progress(1.0, "Target service unavailable", error.clone());
                    newengine_ulog_api::ulog::error!(
                        "engine.threading service-call route resolution failed gateway='{}' method='{}' err='{}'",
                        target_gateway,
                        target_method,
                        error
                    );
                    return;
                }
            };

            control.publish_progress(
                0.20,
                "Invoking target service",
                format!(
                    "Calling {target_gateway}/{target_method} via provider service '{target_service}' through engine threading worker."
                ),
            );

            let payload = match serde_json::to_vec(&payload_json) {
                Ok(bytes) => Blob::from(bytes),
                Err(error) => {
                    control.publish_progress(
                        1.0,
                        "Service call task failed",
                        format!("Failed to serialize target payload: {error}"),
                    );
                    newengine_ulog_api::ulog::error!(
                        "engine.threading service-call payload serialization failed gateway='{}' method='{}' err='{}'",
                        target_gateway,
                        target_method,
                        error
                    );
                    return;
                }
            };

            let response = newengine_plugin_host::call_service_v1(
                newengine_plugin_api::CapabilityId::from(target_service.as_str()),
                newengine_plugin_api::MethodName::from(target_method.as_str()),
                payload,
            );

            match response.into_result() {
                Ok(blob) => {
                    control.publish_progress(
                        1.0,
                        "Target service completed",
                        format!(
                            "Service call completed gateway='{}' provider='{}' output_bytes={}",
                            target_gateway,
                            target_service,
                            blob.len()
                        ),
                    );
                }
                Err(error) => {
                    control.publish_progress(1.0, "Target service failed", error.to_string());
                    newengine_ulog_api::ulog::error!(
                        "engine.threading service-call target failed gateway='{}' provider='{}' method='{}' err='{}'",
                        target_gateway,
                        target_service,
                        target_method,
                        error
                    );
                }
            }
        });
    });

    let task_id = ticket.task_id().to_owned();
    TaskServiceCallAcceptedV1 {
        task_id: task_id.clone(),
        job_id: task_id,
        accepted: true,
        gateway: request.target.gateway,
        method: request.target.method,
        status: "scheduled".to_owned(),
        detail: "Service call scheduled on the engine-runtime thread pool; Engine HostContext and gateway routing are rebound on the worker for this task.".to_owned(),
    }
}
