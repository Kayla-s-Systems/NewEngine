#[inline]
pub(super) fn format_task_id(sequence: u64) -> String {
    format!("engine.job.{sequence}")
}
