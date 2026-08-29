use super::{CpuTaskDto, CpuTaskPriority, ThreadPoolConfig, ThreadPoolManager};

#[test]
fn thread_pool_manager_returns_cpu_task_result() {
    let mut manager = ThreadPoolManager::new(ThreadPoolConfig::fixed(1));
    let ticket = manager.handle().submit(
        CpuTaskDto::new("engine.assets").with_priority(CpuTaskPriority::High),
        |_task, ctx| {
            assert!(ctx.checkpoint());
            vec![1, 2, 3]
        },
    );
    let result = ticket.wait_result();
    assert_eq!(result.output, vec![1, 2, 3]);
    assert_eq!(result.status.as_str(), "completed");
    assert!(manager.snapshot().total_cpu_time_ns > 0);
    manager.shutdown_and_join();
}
