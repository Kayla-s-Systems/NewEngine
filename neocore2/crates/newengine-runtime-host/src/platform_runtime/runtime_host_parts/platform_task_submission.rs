use abi_stable::std_types::RString;
use newengine_core::{TaskRequest, ThreadPoolHandle};
use newengine_platform_api::{
    PlatformHostJobCallbackV1, PlatformHostTaskRequestV1, PlatformHostTaskTicketV1,
};

use super::mapping::{leak_task_label, platform_task_lane, platform_task_priority};

pub(crate) fn submit_platform_task(
    thread_pool: &ThreadPoolHandle,
    request: PlatformHostTaskRequestV1,
    callback: PlatformHostJobCallbackV1,
    callback_user_data: usize,
) -> PlatformHostTaskTicketV1 {
    if callback.is_null() {
        return PlatformHostTaskTicketV1 {
            accepted: false,
            status: RString::from("rejected"),
            detail: RString::from("platform task callback was null"),
            ..Default::default()
        };
    }

    let callback_addr = callback.callback_addr;
    let label = leak_task_label(request.label.as_str(), "platform.task");
    let source = leak_task_label(request.source.as_str(), "engine.platform");
    let owner = leak_task_label(request.owner.as_str(), "platform-runtime");
    let category = leak_task_label(request.category.as_str(), "platform");
    let mut job = TaskRequest::new(label)
        .with_source(source)
        .with_owner(owner)
        .with_category(category)
        .with_lane(platform_task_lane(request.lane.as_str()))
        .with_priority(platform_task_priority(request.priority.as_str()))
        .pausable(false)
        .cancellable(request.can_cancel);
    if !request.task_id.trim().is_empty() {
        job = job.with_task_id(request.task_id.to_string());
    }

    let ticket = thread_pool.submit_controlled(job, move |control| {
        control.publish_progress(
            0.0,
            "Platform task entered",
            "Platform provider callback is running on engine.threading.",
        );
        // SAFETY: platform providers build this handle with
        // `PlatformHostJobCallbackV1::from_fn`. The handle crosses the ABI
        // as a plain address because `abi_stable` does not derive
        // `StableAbi` for function-pointer parameters nested inside another
        // function-pointer signature. The callback is executed once by the
        // submitted engine.threading task.
        let callback_fn: extern "C" fn(usize) -> abi_stable::std_types::RResult<(), RString> =
            unsafe { std::mem::transmute(callback_addr) };
        let result = callback_fn(callback_user_data);
        match result {
            abi_stable::std_types::RResult::ROk(()) => {
                control.publish_progress(
                    1.0,
                    "Platform task completed",
                    "Platform provider callback completed normally.",
                );
            }
            abi_stable::std_types::RResult::RErr(e) => {
                control.publish_progress(1.0, "Platform task failed", e.to_string());
            }
        }
    });

    PlatformHostTaskTicketV1 {
        accepted: true,
        job_id: RString::from(ticket.task_id()),
        status: RString::from("scheduled"),
        detail: RString::from("Platform task submitted to engine.threading."),
    }
}
