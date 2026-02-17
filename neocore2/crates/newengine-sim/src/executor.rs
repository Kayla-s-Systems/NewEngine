use crate::sort::sort_stage;
use crate::{Schedule, SimStage};

pub struct SimExecutor;

impl SimExecutor {
    pub fn run_stage(
        schedule: &mut Schedule,
        stage: SimStage,
        ctx: &mut dyn std::any::Any,
    ) {
        let idx = stage.as_usize();

        if !schedule.is_sorted[idx] {
            sort_stage(&mut schedule.stages[idx]);
            schedule.is_sorted[idx] = true;
        }

        for sys in &schedule.stages[idx] {
            (sys.f)(ctx);
        }
    }
}