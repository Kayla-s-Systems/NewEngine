#![forbid(unsafe_op_in_unsafe_fn)]

mod config;
mod control;
mod events;
mod id;
mod queue;
mod request;
mod service_model;
mod status;
mod worker;

pub(crate) use config::ThreadPoolCoreConfig;
pub use config::{
    TaskLane, TaskPriority, DEFAULT_FRAME_CPU_BUDGET_MS, JOB_LANE_COUNT, JOB_PRIORITY_COUNT,
};
pub(crate) use control::{CoreTaskControl, CoreTaskTicket};
pub use request::TaskRequest;
pub(crate) use service_model::{ThreadPoolCore, ThreadPoolCoreHandle};
pub(crate) use status::{CoreTaskRuntimeStatus, ThreadPoolCoreSnapshot};

#[cfg(test)]
mod tests {
    use super::{
        TaskLane, TaskPriority, TaskRequest, ThreadPoolCore, ThreadPoolCoreConfig,
        DEFAULT_FRAME_CPU_BUDGET_MS,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn worker_inherits_submission_host_context() {
        use newengine_plugin_host::{
            activate_host_context, create_host_context, current_host_context,
        };
        use std::sync::mpsc;
        use std::time::Duration;

        let submission_context = create_host_context();
        let expected_identity = submission_context.identity();
        let mut jobs = ThreadPoolCore::new(ThreadPoolCoreConfig {
            worker_threads: 1,
            frame_cpu_budget_ms: DEFAULT_FRAME_CPU_BUDGET_MS,
        });
        let handle = jobs.handle();
        let (identity_tx, identity_rx) = mpsc::channel();

        let ticket = handle.submit_request(
            TaskRequest::new("host-context-propagation")
                .with_task_id("test.host-context-propagation")
                .with_priority(TaskPriority::Critical),
            move || {
                identity_tx
                    .send(current_host_context().identity())
                    .expect("identity receiver dropped");
            },
        );

        let unrelated_context = create_host_context();
        activate_host_context(&unrelated_context);

        assert_eq!(
            identity_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("worker did not report host context"),
            expected_identity,
            "engine worker escaped the HostContext captured at task submission"
        );
        ticket.wait();
        jobs.shutdown_and_join();
    }

    #[test]
    fn pending_counter_returns_to_zero_after_task_completion() {
        let mut jobs = ThreadPoolCore::new(ThreadPoolCoreConfig {
            worker_threads: 1,
            frame_cpu_budget_ms: DEFAULT_FRAME_CPU_BUDGET_MS,
        });
        let ran = Arc::new(AtomicUsize::new(0));
        let ran_job = Arc::clone(&ran);

        let handle = jobs.handle();
        let ticket = handle.submit_request(
            TaskRequest::new("pending-counter-smoke")
                .with_lane(TaskLane::Background)
                .with_priority(TaskPriority::Critical),
            move || {
                ran_job.fetch_add(1, Ordering::SeqCst);
            },
        );
        ticket.wait();

        let snapshot = jobs.snapshot();
        assert_eq!(ran.load(Ordering::SeqCst), 1);
        assert_eq!(snapshot.pending_jobs, 0);
        assert_eq!(handle.pending_jobs(), 0);
        assert_eq!(handle.pending_for_lane(TaskLane::Background), 0);
        assert!(snapshot.total_cpu_time_ns > 0);

        jobs.shutdown_and_join();
    }

    #[test]
    fn prerequisite_task_does_not_occupy_worker_before_dependency_completes() {
        use std::sync::mpsc;
        use std::time::Duration;

        let mut jobs = ThreadPoolCore::new(ThreadPoolCoreConfig {
            worker_threads: 2,
            frame_cpu_budget_ms: DEFAULT_FRAME_CPU_BUDGET_MS,
        });
        let handle = jobs.handle();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let (order_tx, order_rx) = mpsc::channel::<&'static str>();

        let first = handle.submit_request(
            TaskRequest::new("graph-first")
                .with_task_id("graph.first")
                .with_priority(TaskPriority::Critical),
            move || {
                let _ = order_tx.send("first-start");
                let _ = release_rx.recv_timeout(Duration::from_secs(2));
            },
        );

        let ran_second = Arc::new(AtomicUsize::new(0));
        let ran_second_worker = Arc::clone(&ran_second);
        let second = handle.submit_request(
            TaskRequest::new("graph-second")
                .with_task_id("graph.second")
                .with_priority(TaskPriority::Critical)
                .after_task(first.task_id()),
            move || {
                ran_second_worker.fetch_add(1, Ordering::SeqCst);
            },
        );

        assert_eq!(
            order_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            "first-start"
        );
        std::thread::sleep(Duration::from_millis(25));
        assert_eq!(ran_second.load(Ordering::SeqCst), 0);
        release_tx.send(()).unwrap();
        first.wait();
        second.wait();
        assert_eq!(ran_second.load(Ordering::SeqCst), 1);

        jobs.shutdown_and_join();
    }

    #[test]
    fn prerequisite_can_be_submitted_after_dependent() {
        use std::time::Duration;

        let mut jobs = ThreadPoolCore::new(ThreadPoolCoreConfig {
            worker_threads: 1,
            frame_cpu_budget_ms: DEFAULT_FRAME_CPU_BUDGET_MS,
        });
        let handle = jobs.handle();
        let ran = Arc::new(AtomicUsize::new(0));
        let ran_dependent = Arc::clone(&ran);

        let dependent = handle.submit_request(
            TaskRequest::new("future-dependent")
                .with_task_id("graph.future-dependent")
                .with_priority(TaskPriority::Critical)
                .after_task("graph.future"),
            move || {
                ran_dependent.fetch_add(1, Ordering::SeqCst);
            },
        );

        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(ran.load(Ordering::SeqCst), 0);

        let future = handle.submit_request(
            TaskRequest::new("future-prerequisite")
                .with_task_id("graph.future")
                .with_priority(TaskPriority::Critical),
            || {},
        );
        future.wait();
        dependent.wait();
        assert_eq!(ran.load(Ordering::SeqCst), 1);

        jobs.shutdown_and_join();
    }

    #[test]
    fn fan_in_releases_only_after_last_prerequisite() {
        use std::sync::mpsc;
        use std::time::Duration;

        let mut jobs = ThreadPoolCore::new(ThreadPoolCoreConfig {
            worker_threads: 2,
            frame_cpu_budget_ms: DEFAULT_FRAME_CPU_BUDGET_MS,
        });
        let handle = jobs.handle();
        let (release_a_tx, release_a_rx) = mpsc::channel::<()>();
        let (release_b_tx, release_b_rx) = mpsc::channel::<()>();

        let a = handle.submit_request(
            TaskRequest::new("fan-in-a")
                .with_task_id("graph.fan-in.a")
                .with_priority(TaskPriority::Critical),
            move || {
                let _ = release_a_rx.recv_timeout(Duration::from_secs(2));
            },
        );
        let b = handle.submit_request(
            TaskRequest::new("fan-in-b")
                .with_task_id("graph.fan-in.b")
                .with_priority(TaskPriority::Critical),
            move || {
                let _ = release_b_rx.recv_timeout(Duration::from_secs(2));
            },
        );

        let ran = Arc::new(AtomicUsize::new(0));
        let ran_dependent = Arc::clone(&ran);
        let dependent = handle.submit_request(
            TaskRequest::new("fan-in-dependent")
                .with_task_id("graph.fan-in.dependent")
                .with_priority(TaskPriority::Critical)
                .after_tasks([a.task_id(), b.task_id()]),
            move || {
                ran_dependent.fetch_add(1, Ordering::SeqCst);
            },
        );

        release_a_tx.send(()).unwrap();
        a.wait();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(ran.load(Ordering::SeqCst), 0);

        release_b_tx.send(()).unwrap();
        b.wait();
        dependent.wait();
        assert_eq!(ran.load(Ordering::SeqCst), 1);

        jobs.shutdown_and_join();
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "native worker scheduling and timeout behavior is validated outside Miri; Miri is used for UB semantics"
    )]
    fn worker_wait_helps_ready_same_lane_task_instead_of_deadlocking() {
        use std::sync::mpsc;
        use std::time::Duration;

        let mut jobs = ThreadPoolCore::new(ThreadPoolCoreConfig {
            worker_threads: 1,
            frame_cpu_budget_ms: DEFAULT_FRAME_CPU_BUDGET_MS,
        });
        let handle = jobs.handle();
        let nested_handle = handle.clone();
        let (done_tx, done_rx) = mpsc::channel::<()>();

        let parent = handle.submit_request(
            TaskRequest::new("wait-helping-parent")
                .with_task_id("graph.wait-helping.parent")
                .with_lane(TaskLane::Plugin)
                .with_priority(TaskPriority::Critical),
            move || {
                let child = nested_handle.submit_request(
                    TaskRequest::new("wait-helping-child")
                        .with_task_id("graph.wait-helping.child")
                        .with_lane(TaskLane::Plugin)
                        .with_priority(TaskPriority::Critical),
                    || {},
                );
                child.wait();
                done_tx.send(()).unwrap();
            },
        );

        assert!(
            done_rx.recv_timeout(Duration::from_secs(1)).is_ok(),
            "worker blocked instead of helping a ready same-lane child task"
        );
        parent.wait();
        assert_eq!(jobs.snapshot().pending_jobs, 0);
        assert_eq!(jobs.snapshot().running_jobs, 0);

        jobs.shutdown_and_join();
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "native worker scheduling and timeout behavior is validated outside Miri; Miri is used for UB semantics"
    )]
    fn nested_completion_cascades_through_grandchildren() {
        use newengine_loading_api::EngineTaskPhase;
        use std::sync::{mpsc, Condvar, Mutex};
        use std::time::{Duration, Instant};

        let mut jobs = ThreadPoolCore::new(ThreadPoolCoreConfig {
            worker_threads: 2,
            frame_cpu_budget_ms: DEFAULT_FRAME_CPU_BUDGET_MS,
        });
        let handle = jobs.handle();
        let child_handle = handle.clone();
        let grandchild_handle = handle.clone();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let grandchild_gate = Arc::clone(&gate);
        let (parent_body_tx, parent_body_rx) = mpsc::channel::<()>();
        let (child_body_tx, child_body_rx) = mpsc::channel::<()>();
        let (grandchild_started_tx, grandchild_started_rx) = mpsc::channel::<()>();

        let parent = handle.submit_request(
            TaskRequest::new("nested-cascade-parent")
                .with_task_id("graph.nested-cascade.parent")
                .with_priority(TaskPriority::Critical),
            move || {
                let child_handle_for_body = grandchild_handle.clone();
                let _child = child_handle.submit_request(
                    TaskRequest::new("nested-cascade-child")
                        .with_task_id("graph.nested-cascade.child")
                        .with_parent_task_id("graph.nested-cascade.parent")
                        .with_priority(TaskPriority::Critical),
                    move || {
                        let _grandchild = child_handle_for_body.submit_request(
                            TaskRequest::new("nested-cascade-grandchild")
                                .with_task_id("graph.nested-cascade.grandchild")
                                .with_parent_task_id("graph.nested-cascade.child")
                                .with_priority(TaskPriority::Critical),
                            move || {
                                grandchild_started_tx.send(()).unwrap();
                                let (lock, wake) = &*grandchild_gate;
                                let mut released = lock.lock().unwrap_or_else(|e| e.into_inner());
                                while !*released {
                                    released =
                                        wake.wait(released).unwrap_or_else(|e| e.into_inner());
                                }
                            },
                        );
                        child_body_tx.send(()).unwrap();
                    },
                );
                parent_body_tx.send(()).unwrap();
            },
        );

        parent_body_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("parent body did not return");
        child_body_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("child body did not return");
        grandchild_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("grandchild did not start");

        let deadline = Instant::now() + Duration::from_secs(1);
        while parent.status().phase != EngineTaskPhase::Blocked && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(parent.status().phase, EngineTaskPhase::Blocked);
        assert!(!parent.is_complete());
        assert_eq!(
            handle
                .task_status("graph.nested-cascade.child")
                .expect("child status missing")
                .phase,
            EngineTaskPhase::Blocked
        );

        {
            let (lock, wake) = &*gate;
            *lock.lock().unwrap_or_else(|e| e.into_inner()) = true;
            wake.notify_all();
        }

        parent.wait();
        assert_eq!(
            handle
                .task_status("graph.nested-cascade.parent")
                .expect("parent status missing")
                .phase,
            EngineTaskPhase::Completed
        );
        assert_eq!(
            handle
                .task_status("graph.nested-cascade.child")
                .expect("child status missing")
                .phase,
            EngineTaskPhase::Completed
        );
        assert_eq!(
            handle
                .task_status("graph.nested-cascade.grandchild")
                .expect("grandchild status missing")
                .phase,
            EngineTaskPhase::Completed
        );
        assert_eq!(jobs.snapshot().pending_jobs, 0);
        assert_eq!(jobs.snapshot().running_jobs, 0);

        jobs.shutdown_and_join();
    }

    #[test]
    fn frame_budget_preserves_interactive_render_prep_and_defers_asset_io() {
        use std::sync::mpsc;
        use std::time::Duration;

        let mut jobs = ThreadPoolCore::new(ThreadPoolCoreConfig {
            worker_threads: 1,
            frame_cpu_budget_ms: 1,
        });
        let handle = jobs.handle();
        handle.begin_frame_budget(Duration::from_millis(1));

        let burn = handle.submit_request(
            TaskRequest::new("budget-burn")
                .with_lane(TaskLane::Background)
                .with_priority(TaskPriority::Critical),
            || std::thread::sleep(Duration::from_millis(3)),
        );
        burn.wait();
        let snapshot = jobs.snapshot();
        assert!(snapshot.frame_cpu_used_ns >= snapshot.frame_cpu_budget_ns);

        let (asset_tx, asset_rx) = mpsc::channel();
        let asset = handle.submit_request(
            TaskRequest::new("asset-deferred")
                .with_lane(TaskLane::AssetIo)
                .with_priority(TaskPriority::Interactive),
            move || {
                let _ = asset_tx.send(());
            },
        );

        let (render_tx, render_rx) = mpsc::channel();
        let render = handle.submit_request(
            TaskRequest::new("render-foreground")
                .with_lane(TaskLane::RenderPrep)
                .with_priority(TaskPriority::Interactive),
            move || {
                let _ = render_tx.send(());
            },
        );

        assert!(
            render_rx.recv_timeout(Duration::from_secs(1)).is_ok(),
            "interactive RenderPrep must retain foreground capacity after bulk budget exhaustion"
        );
        render.wait();
        assert!(
            asset_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "interactive AssetIo must be deferred until the next frame budget window"
        );

        handle.begin_frame_budget(Duration::from_millis(1));
        assert!(asset_rx.recv_timeout(Duration::from_secs(1)).is_ok());
        asset.wait();

        jobs.shutdown_and_join();
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "native worker scheduling and timeout behavior is validated outside Miri; Miri is used for UB semantics"
    )]
    fn parent_completion_waits_for_nested_child_and_delays_dependents() {
        use newengine_loading_api::EngineTaskPhase;
        use std::sync::{mpsc, Condvar, Mutex};
        use std::time::{Duration, Instant};

        let mut jobs = ThreadPoolCore::new(ThreadPoolCoreConfig {
            worker_threads: 2,
            frame_cpu_budget_ms: DEFAULT_FRAME_CPU_BUDGET_MS,
        });
        let handle = jobs.handle();
        let nested_handle = handle.clone();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let child_gate = Arc::clone(&gate);
        let (parent_body_tx, parent_body_rx) = mpsc::channel::<()>();
        let (child_started_tx, child_started_rx) = mpsc::channel::<()>();

        let parent = handle.submit_request(
            TaskRequest::new("nested-parent")
                .with_task_id("graph.nested.parent")
                .with_priority(TaskPriority::Critical),
            move || {
                let _child = nested_handle.submit_request(
                    TaskRequest::new("nested-child")
                        .with_task_id("graph.nested.child")
                        .with_parent_task_id("graph.nested.parent")
                        .with_priority(TaskPriority::Critical),
                    move || {
                        child_started_tx.send(()).unwrap();
                        let (lock, wake) = &*child_gate;
                        let mut released = lock.lock().unwrap_or_else(|e| e.into_inner());
                        while !*released {
                            released = wake.wait(released).unwrap_or_else(|e| e.into_inner());
                        }
                    },
                );
                parent_body_tx.send(()).unwrap();
            },
        );

        parent_body_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("parent body did not return");
        child_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("nested child did not start");

        let dependent_ran = Arc::new(AtomicUsize::new(0));
        let dependent_ran_worker = Arc::clone(&dependent_ran);
        let dependent = handle.submit_request(
            TaskRequest::new("nested-dependent")
                .with_task_id("graph.nested.dependent")
                .with_priority(TaskPriority::Critical)
                .after_task("graph.nested.parent"),
            move || {
                dependent_ran_worker.fetch_add(1, Ordering::SeqCst);
            },
        );

        let deadline = Instant::now() + Duration::from_secs(1);
        while parent.status().phase != EngineTaskPhase::Blocked && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(parent.status().phase, EngineTaskPhase::Blocked);
        assert!(!parent.is_complete());
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(dependent_ran.load(Ordering::SeqCst), 0);

        {
            let (lock, wake) = &*gate;
            *lock.lock().unwrap_or_else(|e| e.into_inner()) = true;
            wake.notify_all();
        }

        let deadline = Instant::now() + Duration::from_secs(1);
        while !parent.is_complete() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(
            parent.is_complete(),
            "parent did not finalize after nested child"
        );
        assert_eq!(parent.status().phase, EngineTaskPhase::Completed);
        parent.wait();
        dependent.wait();
        assert_eq!(dependent_ran.load(Ordering::SeqCst), 1);

        jobs.shutdown_and_join();
    }
    #[test]
    fn shutdown_drains_ready_work_even_when_last_frame_is_over_budget() {
        use std::sync::mpsc;
        use std::time::Duration;

        let mut jobs = ThreadPoolCore::new(ThreadPoolCoreConfig {
            worker_threads: 1,
            frame_cpu_budget_ms: 1,
        });
        let handle = jobs.handle();
        handle.begin_frame_budget(Duration::from_millis(1));

        let burn = handle.submit_request(
            TaskRequest::new("shutdown-budget-burn")
                .with_lane(TaskLane::Background)
                .with_priority(TaskPriority::Critical),
            || std::thread::sleep(Duration::from_millis(3)),
        );
        burn.wait();
        assert!(jobs.snapshot().frame_cpu_used_ns >= jobs.snapshot().frame_cpu_budget_ns);

        let (tx, rx) = mpsc::channel();
        let _deferred = handle.submit_request(
            TaskRequest::new("shutdown-deferred-asset")
                .with_lane(TaskLane::AssetIo)
                .with_priority(TaskPriority::Interactive),
            move || {
                let _ = tx.send(());
            },
        );
        assert!(rx.recv_timeout(Duration::from_millis(50)).is_err());
        assert!(jobs.snapshot().pending_jobs > 0);

        // Shutdown must ignore the exhausted frame budget and drain ready work;
        // otherwise worker join deadlocks forever with pending > 0.
        jobs.shutdown_and_join();

        assert!(rx.recv_timeout(Duration::from_secs(1)).is_ok());
        assert_eq!(jobs.snapshot().pending_jobs, 0);
        assert_eq!(jobs.snapshot().running_jobs, 0);
    }
}
