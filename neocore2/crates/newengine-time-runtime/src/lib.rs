#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::time::Instant;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    payload_json, register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
    JsonServiceRouter,
};
use newengine_time_api::{
    time_method, TimeAiClockV1, TimeAiContextV1, TimeBeginFrameRequestV1, TimeCancelEventRequestV1,
    TimeDueEventsV1, TimeFixedStepRequestV1, TimeGameClockSetRequestV1, TimePauseRequestV1,
    TimeRealClockV1, TimeReplayClockSetRequestV1, TimeReplayClockV1, TimeScaleRequestV1,
    TimeScheduledEventV1, TimeServiceInfoV1, TimeSimulationClockV1, TimeSnapshotV1, TimeTimelineV1,
    ENGINE_TIME_SERVICE_ID, TIME_BACKEND_CAPABILITY_ID, TIME_RUNTIME_CONTRACT,
    TIME_SERVICE_ID, TIME_SERVICE_METHODS,
};
use parking_lot::Mutex;
use std::sync::{Arc, OnceLock};

const OWNER: &str = "newengine-time-runtime.engine-runtime-provider";
const MAX_FIXED_TICKS_PER_FRAME: u32 = 1;

static TIME_GATEWAY: OnceLock<Arc<Mutex<RuntimeHostedTimeState>>> = OnceLock::new();

#[derive(Debug)]
struct RuntimeHostedTimeState {
    start: Instant,
    last: Instant,
    frame_index: u64,
    last_raw_delta_ns: u64,
    last_clamped_delta_ns: u64,
    fixed_delta_ns: u64,
    max_fixed_ticks_per_frame: u32,
    accumulator_ns: u64,
    tick: u64,
    ticks_to_run: u32,
    paused: bool,
    scale: f64,
    day_index: u64,
    seconds_of_day: f64,
    seconds_per_game_day: f64,
    game_time_scale: f64,
    replay_deterministic: bool,
    replay_seed: u64,
    replay_frame: u64,
    ai_tick_budget_ns: u64,
    ai_decision_tick_interval: u32,
    scheduled_events: BTreeMap<String, TimeScheduledEventV1>,
}

impl Default for RuntimeHostedTimeState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last: now,
            frame_index: 0,
            last_raw_delta_ns: 0,
            last_clamped_delta_ns: 0,
            fixed_delta_ns: 16_666_667,
            max_fixed_ticks_per_frame: MAX_FIXED_TICKS_PER_FRAME,
            accumulator_ns: 0,
            tick: 0,
            ticks_to_run: 0,
            paused: false,
            scale: 1.0,
            day_index: 0,
            seconds_of_day: 0.0,
            seconds_per_game_day: 86_400.0,
            game_time_scale: 1.0,
            replay_deterministic: false,
            replay_seed: 0,
            replay_frame: 0,
            ai_tick_budget_ns: 1_000_000,
            ai_decision_tick_interval: 4,
            scheduled_events: BTreeMap::new(),
        }
    }
}

impl RuntimeHostedTimeState {
    fn normalized_day(&self) -> f64 {
        if self.seconds_per_game_day <= f64::EPSILON {
            0.0
        } else {
            (self.seconds_of_day / 86_400.0).rem_euclid(1.0)
        }
    }

    fn time_of_day_phase(&self) -> String {
        let hour = (self.normalized_day() * 24.0).rem_euclid(24.0);
        match hour {
            h if h < 5.0 => "night",
            h if h < 8.0 => "dawn",
            h if h < 17.0 => "day",
            h if h < 20.0 => "dusk",
            _ => "night",
        }.to_owned()
    }

    fn next_ai_decision_tick(&self) -> u64 {
        let interval = self.ai_decision_tick_interval.max(1) as u64;
        let rem = self.tick % interval;
        if rem == 0 { self.tick } else { self.tick + (interval - rem) }
    }

    fn ai_deterministic_key(&self) -> String {
        format!(
            "ai-time:v1:seed={:016x}:day={}:tick={}:frame={}",
            self.replay_seed,
            self.day_index,
            self.tick,
            self.replay_frame,
        )
    }

    fn ai_clock(&self) -> TimeAiClockV1 {
        TimeAiClockV1 {
            tick_budget_ns: self.ai_tick_budget_ns,
            decision_tick_interval: self.ai_decision_tick_interval.max(1),
            next_decision_tick: self.next_ai_decision_tick(),
            time_of_day_phase: self.time_of_day_phase(),
            normalized_day: self.normalized_day(),
            deterministic_key: self.ai_deterministic_key(),
        }
    }

    fn timeline(&self) -> TimeTimelineV1 {
        TimeTimelineV1 {
            frame_index: self.frame_index,
            fixed_tick: self.tick,
            game_day_index: self.day_index,
            game_seconds_of_day: self.seconds_of_day,
            replay_frame: self.replay_frame,
            paused: self.paused,
            scale: self.scale,
            ..Default::default()
        }
    }

    fn ai_context(&self) -> TimeAiContextV1 {
        TimeAiContextV1 {
            frame_index: self.frame_index,
            simulation_tick: self.tick,
            fixed_delta_ns: self.fixed_delta_ns,
            game_day_index: self.day_index,
            game_seconds_of_day: self.seconds_of_day,
            normalized_day: self.normalized_day(),
            time_of_day_phase: self.time_of_day_phase(),
            deterministic: self.replay_deterministic,
            replay_seed: self.replay_seed,
            replay_frame: self.replay_frame,
            decision_tick_interval: self.ai_decision_tick_interval.max(1),
            next_decision_tick: self.next_ai_decision_tick(),
            tick_budget_ns: self.ai_tick_budget_ns,
            deterministic_key: self.ai_deterministic_key(),
            ..Default::default()
        }
    }
    fn snapshot(&self) -> TimeSnapshotV1 {
        let normalized_day = self.normalized_day();
        TimeSnapshotV1 {
            schema: TIME_RUNTIME_CONTRACT.to_owned(),
            provider: "AstrolabeTimeProvider".to_owned(),
            frame_index: self.frame_index,
            real: TimeRealClockV1 {
                monotonic_ns: self.start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
                delta_ns: self.last_raw_delta_ns,
                clamped_delta_ns: self.last_clamped_delta_ns,
            },
            simulation: TimeSimulationClockV1 {
                tick: self.tick,
                fixed_delta_ns: self.fixed_delta_ns,
                accumulator_ns: self.accumulator_ns,
                ticks_to_run: self.ticks_to_run,
                paused: self.paused,
                scale: self.scale,
            },
            game: newengine_time_api::TimeGameClockV1 {
                day_index: self.day_index,
                seconds_of_day: self.seconds_of_day,
                normalized_day,
                seconds_per_game_day: self.seconds_per_game_day,
                time_scale: self.game_time_scale,
            },
            replay: TimeReplayClockV1 {
                deterministic: self.replay_deterministic,
                seed: self.replay_seed,
                replay_frame: self.replay_frame,
            },
            ai: self.ai_clock(),
        }
    }

    fn begin_frame(&mut self, request: TimeBeginFrameRequestV1) -> TimeSnapshotV1 {
        let now = Instant::now();
        let raw_delta_ns = now.duration_since(self.last).as_nanos().min(u128::from(u64::MAX)) as u64;
        self.last = now;
        self.frame_index = request.frame_index;
        if request.fixed_delta_ns > 0 {
            self.fixed_delta_ns = request.fixed_delta_ns;
        }
        self.last_raw_delta_ns = raw_delta_ns;
        let max_ticks = self.max_fixed_ticks_per_frame.max(1);
        let accumulator_cap = self
            .fixed_delta_ns
            .saturating_mul(u64::from(max_ticks))
            .max(self.fixed_delta_ns);
        self.last_clamped_delta_ns = raw_delta_ns.min(accumulator_cap);

        let scaled_delta_ns = if self.paused {
            0
        } else {
            ((self.last_clamped_delta_ns as f64) * self.scale.max(0.0)) as u64
        };

        // Realtime frame policy: never accumulate a simulation backlog on the
        // render thread. If a frame is slow, the engine advances at most the
        // configured fixed-step budget and drops excess wall-clock debt instead
        // of trying to "catch up" with multiple plugin lifecycle passes on the
        // next visible frame. This does not cap FPS; it prevents fixed-update
        // work from multiplying when FPS is already low.
        self.accumulator_ns = if self.paused {
            0
        } else {
            self.accumulator_ns.saturating_add(scaled_delta_ns).min(accumulator_cap)
        };
        self.ticks_to_run = if self.fixed_delta_ns == 0 || self.paused {
            0
        } else if self.accumulator_ns >= self.fixed_delta_ns {
            max_ticks
        } else {
            0
        };

        if self.ticks_to_run >= max_ticks && self.accumulator_ns > accumulator_cap {
            self.accumulator_ns = accumulator_cap;
        }

        if !self.paused && self.seconds_per_game_day > f64::EPSILON {
            let game_delta_seconds = (self.last_clamped_delta_ns as f64 / 1_000_000_000.0)
                * self.game_time_scale.max(0.0)
                * (86_400.0 / self.seconds_per_game_day);
            self.seconds_of_day += game_delta_seconds;
            while self.seconds_of_day >= 86_400.0 {
                self.seconds_of_day -= 86_400.0;
                self.day_index = self.day_index.wrapping_add(1);
            }
        }
        self.replay_frame = self.replay_frame.wrapping_add(1);
        if self.ticks_to_run >= self.max_fixed_ticks_per_frame.max(1)
            && self.frame_index % 120 == 0
            && !self.paused
        {
            newengine_ulog_api::ulog::warn!(
                "time gateway: realtime fixed-step debt dropped frame={} raw_delta_ns={} clamped_ns={} ticks_to_run={} max_ticks={} accumulator_ns={} scale={:.3}",
                self.frame_index,
                self.last_raw_delta_ns,
                self.last_clamped_delta_ns,
                self.ticks_to_run,
                self.max_fixed_ticks_per_frame,
                self.accumulator_ns,
                self.scale
            );
        }
        self.snapshot()
    }

    fn advance_fixed(&mut self) -> TimeSnapshotV1 {
        if self.accumulator_ns >= self.fixed_delta_ns {
            self.accumulator_ns -= self.fixed_delta_ns;
        } else {
            self.accumulator_ns = 0;
        }
        self.tick = self.tick.wrapping_add(1);
        // Realtime mode drops remaining debt after the visible-frame fixed step.
        // Separate simulation workers may later own high-frequency catch-up, but
        // plugin lifecycle fixed_update must not multiply on the render thread.
        self.accumulator_ns = 0;
        self.ticks_to_run = 0;
        self.snapshot()
    }

    fn due_events(&mut self) -> TimeDueEventsV1 {
        let now_ns = self.start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let due_ids = self
            .scheduled_events
            .iter()
            .filter_map(|(id, event)| {
                let due_tick = event.due_simulation_tick.map(|tick| tick <= self.tick).unwrap_or(false);
                let due_time = event.due_monotonic_ns.map(|ns| ns <= now_ns).unwrap_or(false);
                (due_tick || due_time).then_some(id.clone())
            })
            .collect::<Vec<_>>();
        let mut events = Vec::with_capacity(due_ids.len());
        for id in due_ids {
            if let Some(event) = self.scheduled_events.remove(&id) {
                events.push(event);
            }
        }
        TimeDueEventsV1 { events }
    }
}

fn state() -> Arc<Mutex<RuntimeHostedTimeState>> {
    Arc::clone(TIME_GATEWAY.get_or_init(|| Arc::new(Mutex::new(RuntimeHostedTimeState::default()))))
}

fn info() -> TimeServiceInfoV1 {
    TimeServiceInfoV1::default()
}

fn invoke(state: &mut RuntimeHostedTimeState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let method = value
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or(time_method::SNAPSHOT_V1);
    match method {
        time_method::SNAPSHOT_V1 => ok_json(state.snapshot()),
        time_method::DESCRIBE_CLOCK_V1 => ok_json(info()),
        time_method::AI_CONTEXT_V1 => ok_json(state.ai_context()),
        other => RResult::RErr(RString::from(format!("engine.time: unknown invoke method '{other}'"))),
    }
}

fn service() -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        TIME_SERVICE_ID,
        OWNER,
        TIME_BACKEND_CAPABILITY_ID,
        TIME_SERVICE_METHODS.iter().copied(),
    )
    .protocol(TIME_RUNTIME_CONTRACT)
    .features(["frame-clock", "fixed-timestep", "game-clock", "pause-domain", "timeline", "scheduler-clock", "ai-context-clock", "deterministic-replay-clock"])
    .gateway("engine.time.astrolabe baseline provider")
    .notes("Owns runtime clock state. Domains and AI providers consume TimeSnapshotV1/TimeAiContextV1 instead of calling Instant::now().");

    JsonServiceRouter::with_shared_state(TIME_SERVICE_ID, state())
        .describe_json(&description)
        .info(info)
        .get_json(time_method::SNAPSHOT_V1, |state| state.snapshot())
        .get_json(time_method::FRAME_V1, |state| state.snapshot())
        .post_json::<TimeBeginFrameRequestV1, TimeSnapshotV1, _>(time_method::BEGIN_FRAME_V1, |state, request| state.begin_frame(request))
        .get_json(time_method::ADVANCE_FIXED_V1, |state| state.advance_fixed())
        .get_json(time_method::FIXED_TICK_V1, |state| state.advance_fixed())
        .get_json(time_method::GAME_CLOCK_V1, |state| state.snapshot().game)
        .post_json::<TimePauseRequestV1, TimeSnapshotV1, _>(time_method::PAUSE_DOMAIN_V1, |state, request| {
            state.paused = request.paused;
            state.snapshot()
        })
        .get_json(time_method::TIMELINE_V1, |state| state.timeline())
        .get_json(time_method::REPLAY_CLOCK_V1, |state| state.snapshot().replay)
        .post_json::<TimeScaleRequestV1, TimeSnapshotV1, _>(time_method::SET_SCALE_V1, |state, request| {
            state.scale = request.scale.clamp(0.0, 64.0);
            state.snapshot()
        })
        .post_json::<TimePauseRequestV1, TimeSnapshotV1, _>(time_method::SET_PAUSE_V1, |state, request| {
            state.paused = request.paused;
            state.snapshot()
        })
        .post_json::<TimeGameClockSetRequestV1, TimeSnapshotV1, _>(time_method::SET_GAME_CLOCK_V1, |state, request| {
            state.day_index = request.day_index;
            state.seconds_of_day = request.seconds_of_day.rem_euclid(86_400.0);
            if request.seconds_per_game_day > f64::EPSILON {
                state.seconds_per_game_day = request.seconds_per_game_day;
            }
            state.game_time_scale = request.time_scale.max(0.0);
            state.snapshot()
        })
        .post_json::<TimeFixedStepRequestV1, TimeSnapshotV1, _>(time_method::SET_FIXED_STEP_V1, |state, request| {
            if request.fixed_delta_ns > 0 {
                state.fixed_delta_ns = request.fixed_delta_ns;
            }
            state.max_fixed_ticks_per_frame = 1;
            state.ai_decision_tick_interval = request.ai_decision_tick_interval.max(1);
            state.ai_tick_budget_ns = request.ai_tick_budget_ns.max(1_000);
            state.snapshot()
        })
        .post_json::<TimeReplayClockSetRequestV1, TimeSnapshotV1, _>(time_method::SET_REPLAY_CLOCK_V1, |state, request| {
            state.replay_deterministic = request.deterministic;
            state.replay_seed = request.seed;
            state.replay_frame = request.replay_frame;
            state.snapshot()
        })
        .get_json(time_method::AI_CONTEXT_V1, |state| state.ai_context())
        .post_json::<TimeScheduledEventV1, TimeScheduledEventV1, _>(time_method::SCHEDULE_EVENT_V1, |state, event| {
            let mut event = event;
            if event.id.trim().is_empty() {
                event.id = format!("time.event.{}", state.scheduled_events.len() + 1);
            }
            state.scheduled_events.insert(event.id.clone(), event.clone());
            event
        })
        .post_json::<TimeCancelEventRequestV1, TimeDueEventsV1, _>(time_method::CANCEL_EVENT_V1, |state, request| {
            let mut events = Vec::new();
            if let Some(event) = state.scheduled_events.remove(request.id.trim()) {
                events.push(event);
            }
            TimeDueEventsV1 { events }
        })
        .get_json(time_method::DUE_EVENTS_V1, |state| state.due_events())
        .get_json(time_method::DESCRIBE_CLOCK_V1, |_state| info())
        .blob(time_method::INVOKE_JSON, invoke)
        .blob(time_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

pub fn register_time_gateway_best_effort() -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_TIME_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Time,
        provider_service: TIME_SERVICE_ID,
        provider_route: "engine.time.astrolabe",
        capability: TIME_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: OWNER,
        service: service(),
    })
}
