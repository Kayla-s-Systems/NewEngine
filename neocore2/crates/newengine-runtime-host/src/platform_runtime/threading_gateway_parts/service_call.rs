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

    let ticket = thread_pool.submit_controlled(job, move |control| {
        if !control.checkpoint() {
            control.publish_progress(
                1.0,
                "Service call task cancelled",
                "Task was cancelled before invoking the target service.",
            );
            return;
        }

        control.publish_progress(
            0.20,
            "Invoking target service",
            format!("Calling {target_gateway}/{target_method} through engine threading worker."),
        );

        let payload = match serde_json::to_vec(&payload_json) {
            Ok(bytes) => Blob::from(bytes),
            Err(e) => {
                control.publish_progress(
                    1.0,
                    "Service call task failed",
                    format!("Failed to serialize target payload: {e}"),
                );
                panic!("engine.threading service-call payload serialization failed: {e}");
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
                panic!("engine.threading service-call target failed: {e}");
            }
        }
    });

    let task_id = ticket.task_id().to_owned();
    TaskServiceCallAcceptedV1 {
        task_id: task_id.clone(),
        job_id: task_id,
        accepted: true,
        gateway: request.target.gateway,
        method: request.target.method,
        status: "scheduled".to_owned(),
        detail: "Service call scheduled on the engine-runtime thread pool; no plugin-owned background worker was created.".to_owned(),
    }
}
