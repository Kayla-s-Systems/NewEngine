use abi_stable::std_types::{RResult, RString};
use newengine_core::{TaskRequest, ThreadPoolHandle};
use newengine_plugin_api::Blob;
use newengine_task_api::{
    EngineTaskPhase, TaskIdRequestV1, TaskRunProcessStartRequestV1, TaskRunProcessStartedV1,
    TaskStatusJsonV1,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::mapping::{
    lane_from_str, priority_from_str, task_domain_from_request, task_pass_from_category,
};

pub(crate) type ProcessResults = Arc<Mutex<HashMap<String, ProcessResultRecord>>>;

pub(crate) fn process_status_from_record(
    task_id: String,
    record: ProcessResultRecord,
) -> TaskStatusJsonV1 {
    TaskStatusJsonV1 {
        task_id,
        name: "external-process".to_owned(),
        lane: record.lane,
        priority: record.priority,
        frame_id: record.frame_id,
        dependency_group: record.dependency_group,
        task_domain: record.task_domain,
        task_pass: record.task_pass,
        phase: record.phase,
        can_pause: false,
        can_cancel: record.can_cancel,
        found: true,
        ..Default::default()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessResultRecord {
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
    task_domain: String,
    task_pass: String,
    can_cancel: bool,
}

impl ProcessResultRecord {
    fn running_from_process_request(request: &TaskRunProcessStartRequestV1) -> Self {
        let owner = match request.owner.as_str() {
            "engine.render" | "engine.render.vulkan" => "engine.render",
            _ => "engine.threading",
        };
        let task_pass = task_pass_from_category(request.category.as_str(), "process").to_owned();
        Self {
            phase: EngineTaskPhase::Running,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            result_bytes: None,
            error: None,
            lane: lane_from_str(request.lane.as_str()).as_str().to_owned(),
            priority: priority_from_str(request.priority.as_str())
                .as_str()
                .to_owned(),
            frame_id: request.frame_id,
            dependency_group: if request.dependency_group.trim().is_empty() {
                format!("{}.process", task_pass)
            } else {
                request.dependency_group.trim().to_owned()
            },
            task_domain: task_domain_from_request(request.task_domain.as_str(), owner).to_owned(),
            task_pass,
            can_cancel: request.can_cancel,
        }
    }
}

fn process_task_request(request: &TaskRunProcessStartRequestV1) -> TaskRequest {
    let label = match request.category.as_str() {
        "shader.compile" => "shader.compile",
        "shader.validate" => "shader.validate",
        "tool.process" => "tool.process",
        _ => "external.process",
    };
    let owner = match request.owner.as_str() {
        "engine.render" | "engine.render.vulkan" => "engine.render",
        _ => "engine.threading",
    };
    let category = match request.category.as_str() {
        "shader.compile" => "shader.compile",
        "shader.validate" => "shader.validate",
        _ => "tool.process",
    };
    let mut job = TaskRequest::new(label)
        .with_source("engine.threading.process")
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
            "process",
        ))
        .pausable(false)
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

pub(crate) fn submit_process_task(
    thread_pool: &ThreadPoolHandle,
    process_results: &ProcessResults,
    request: TaskRunProcessStartRequestV1,
) -> TaskRunProcessStartedV1 {
    let executable = request.executable.trim().to_owned();
    let requested_task_id = request.task_id.trim().to_owned();
    if executable.is_empty() {
        return TaskRunProcessStartedV1 {
            task_id: requested_task_id,
            accepted: false,
            status: "rejected".to_owned(),
            detail: "task.run_process_start_v1 requires executable".to_owned(),
            result_path: request.result_path,
        };
    }

    let job = process_task_request(&request);
    let args = request.args.clone();
    let cwd = request.cwd.trim().to_owned();
    let env = request.env.clone();
    let result_path = request.result_path.trim().to_owned();
    let results = process_results.clone();
    let base_record = ProcessResultRecord::running_from_process_request(&request);

    let ticket = thread_pool.submit_controlled(job, move |control| {
        let task_id = control.task_id().to_owned();
        results
            .lock()
            .expect("engine.threading process_results mutex poisoned")
            .insert(task_id.clone(), base_record.clone());
        if !control.checkpoint() {
            results
                .lock()
                .expect("engine.threading process_results mutex poisoned")
                .insert(
                    task_id.clone(),
                    ProcessResultRecord {
                        phase: EngineTaskPhase::Cancelled,
                        error: Some("process task cancelled before spawn".to_owned()),
                        ..base_record.clone()
                    },
                );
            return;
        }
        control.publish_progress(
            0.10,
            "Starting external process",
            format!(
                "Launching tracked process executable='{}' args={}.",
                executable,
                args.len()
            ),
        );
        // no-hidden-thread-scan: engine.threading owns tracked process execution; callers receive a thread task id and poll result/status.
        // External process execution is intentionally centralized behind engine.threading;
        // render/tool consumers receive a task id and must poll instead of blocking a frame caller.
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
                results
                    .lock()
                    .expect("engine.threading process_results mutex poisoned")
                    .insert(
                        task_id.clone(),
                        ProcessResultRecord {
                            phase: EngineTaskPhase::Failed,
                            error: Some(format!(
                                "process spawn failed executable='{}' err='{e}'",
                                executable
                            )),
                            ..base_record.clone()
                        },
                    );
                control.publish_progress(
                    1.0,
                    "External process failed",
                    format!("Spawn failed: {e}"),
                );
                return;
            }
        };

        let started = Instant::now();
        if !control.checkpoint() {
            let _ = child.kill();
            let _ = child.wait();
            results
                .lock()
                .expect("engine.threading process_results mutex poisoned")
                .insert(
                    task_id.clone(),
                    ProcessResultRecord {
                        phase: EngineTaskPhase::Cancelled,
                        error: Some(format!(
                            "process task cancelled executable='{}' elapsed_ms={}",
                            executable,
                            started.elapsed().as_millis()
                        )),
                        ..base_record.clone()
                    },
                );
            control.publish_progress(
                1.0,
                "External process cancelled",
                "Child process was killed before process wait began.",
            );
            return;
        }

        control.publish_progress(
            0.50,
            "External process running",
            format!(
                "Process wait armed executable='{}' policy='os-process-completion-event'",
                executable
            ),
        );

        let output = match child.wait_with_output() {
            Ok(output) => output,
            Err(e) => {
                results
                    .lock()
                    .expect("engine.threading process_results mutex poisoned")
                    .insert(
                        task_id.clone(),
                        ProcessResultRecord {
                            phase: EngineTaskPhase::Failed,
                            error: Some(format!(
                                "process wait/read failed executable='{}' err='{e}'",
                                executable
                            )),
                            ..base_record.clone()
                        },
                    );
                control.publish_progress(
                    1.0,
                    "External process failed",
                    format!("Wait/read failed: {e}"),
                );
                return;
            }
        };
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let result_bytes = if !result_path.is_empty() {
            match std::fs::read(&result_path) {
                Ok(bytes) => Some(bytes),
                Err(e) => {
                    results
                        .lock()
                        .expect("engine.threading process_results mutex poisoned")
                        .insert(
                            task_id.clone(),
                            ProcessResultRecord {
                                phase: EngineTaskPhase::Failed,
                                exit_code: output.status.code(),
                                stdout,
                                stderr,
                                result_bytes: None,
                                error: Some(format!(
                                    "process result read failed path='{}' err='{e}'",
                                    result_path
                                )),
                                ..base_record.clone()
                            },
                        );
                    control.publish_progress(
                        1.0,
                        "External process failed",
                        "Process completed but result file could not be read.",
                    );
                    return;
                }
            }
        } else {
            Some(output.stdout.clone())
        };
        let phase = if output.status.success() {
            EngineTaskPhase::Completed
        } else {
            EngineTaskPhase::Failed
        };
        let error = if output.status.success() {
            None
        } else {
            Some(format!("process exited with status {}", output.status))
        };
        results
            .lock()
            .expect("engine.threading process_results mutex poisoned")
            .insert(
                task_id.clone(),
                ProcessResultRecord {
                    phase,
                    exit_code: output.status.code(),
                    stdout,
                    stderr,
                    result_bytes,
                    error,
                    ..base_record.clone()
                },
            );
        control.publish_progress(
            1.0,
            if output.status.success() {
                "External process completed"
            } else {
                "External process failed"
            },
            format!(
                "Process exited status={} result_path='{}' elapsed_ms={}",
                output.status,
                result_path,
                started.elapsed().as_millis()
            ),
        );
    });

    TaskRunProcessStartedV1 {
        task_id: ticket.task_id().to_owned(),
        accepted: true,
        status: "scheduled".to_owned(),
        detail: "External process scheduled on engine.threading; caller must poll status/result through the threading gateway.".to_owned(),
        result_path: request.result_path,
    }
}

pub(crate) fn result_bin(
    process_results: &ProcessResults,
    payload: Blob,
) -> RResult<Blob, RString> {
    let request: TaskIdRequestV1 = match serde_json::from_slice(payload.as_slice()) {
        Ok(request) => request,
        Err(e) => {
            return RResult::RErr(RString::from(format!(
                "task.result_bin_v1 invalid request json: {e}"
            )))
        }
    };
    let task_id = request.task_id.trim();
    let Some(record) = process_results
        .lock()
        .expect("engine.threading process_results mutex poisoned")
        .get(task_id)
        .cloned()
    else {
        return RResult::RErr(RString::from(format!(
            "task.result_bin_v1 task_id='{task_id}' has no process result"
        )));
    };
    match record.phase {
        EngineTaskPhase::Completed => RResult::ROk(Blob::from(record.result_bytes.unwrap_or_default())),
        EngineTaskPhase::Failed => RResult::RErr(RString::from(format!(
            "task.result_bin_v1 task_id='{task_id}' failed exit_code={:?} error='{}' stdout='{}' stderr='{}'",
            record.exit_code,
            record.error.unwrap_or_default(),
            record.stdout,
            record.stderr,
        ))),
        other => RResult::RErr(RString::from(format!("task.result_bin_v1 task_id='{task_id}' not ready phase={other:?}"))),
    }
}
