use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::queue::TaskCoreShared;

pub(super) fn worker_loop(shared: Arc<TaskCoreShared>) {
    loop {
        if let Some(job) = shared.pop_next() {
            job.run(&shared);
            continue;
        }

        if shared.shutdown.load(Ordering::Acquire) && shared.pending.load(Ordering::Acquire) == 0 {
            break;
        }

        shared.wait_for_work_or_shutdown();
    }
}
