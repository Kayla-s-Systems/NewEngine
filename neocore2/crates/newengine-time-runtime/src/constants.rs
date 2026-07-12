pub(crate) const OWNER: &str = "newengine-time-runtime.engine-runtime-provider";
pub(crate) const PROVIDER_NAME: &str = "AstrolabeTimeProvider";
pub(crate) const PROVIDER_ROUTE: &str = "engine.time.astrolabe";
pub(crate) const DEFAULT_FIXED_DELTA_NS: u64 = 16_666_667;
pub(crate) const DEFAULT_MAX_FIXED_TICKS_PER_FRAME: u32 = 4;
pub(crate) const HARD_MAX_FIXED_TICKS_PER_FRAME: u32 = 8;
pub(crate) const DEFAULT_AI_TICK_BUDGET_NS: u64 = 1_000_000;
pub(crate) const DEFAULT_AI_DECISION_INTERVAL: u32 = 4;
pub(crate) const SECONDS_PER_DAY: f64 = 86_400.0;

pub(crate) const TIME_FEATURES: &[&str] = &[
    "frame-clock",
    "fixed-timestep",
    "game-clock",
    "pause-domain",
    "timeline",
    "scheduler-clock",
    "ai-context-clock",
    "deterministic-replay-clock",
];
