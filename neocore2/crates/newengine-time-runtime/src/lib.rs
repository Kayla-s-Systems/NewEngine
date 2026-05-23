#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::time::Instant;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json,
    payload_json, register_engine_owned_gateway_service_best_effort, EngineOwnedGatewayDecl,
    JsonServiceRouter,
};
use newengine_time_api::{
    time_method, TimeBeginFrameRequestV1, TimeCancelEventRequestV1, TimeDueEventsV1,
    TimeGameClockSetRequestV1, TimePauseRequestV1, TimeRealClockV1, TimeReplayClockV1,
    TimeScaleRequestV1, TimeScheduledEventV1, TimeServiceInfoV1, TimeSimulationClockV1,
    TimeSnapshotV1, ENGINE_TIME_SERVICE_ID, TIME_BACKEND_CAPABILITY_ID, TIME_RUNTIME_CONTRACT,
    TIME_SERVICE_ID, TIME_SERVICE_METHODS,
};
use parking_lot::Mutex;
use std::sync::{Arc, OnceLock};

const OWNER: &str = "newengine-time-runtime.engine-owned-provider";
const MAX_DELTA_NS: u64 = 250_000_000;
const MAX_FIXED_TICKS_PER_FRAME: u32 = 8;

static TIME_GATEWAY: OnceLock<Arc<Mutex<EngineOwnedTimeState>>> = OnceLock::new();

#[derive(Debug)]
struct EngineOwnedTimeState {
    start: Instant,
    last: Instant,
    frame_index: u64,
    last_raw_delta_ns: u64,
    last_clamped_delta_ns: u64,
    fixed_delta_ns: u64,
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
    scheduled_events: BTreeMap<String, TimeScheduledEventV1>,
}

impl Default for EngineOwnedTimeState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last: now,
            frame_index: 0,
            last_raw_delta_ns: 0,
            last_clamped_delta_ns: 0,
            fixed_delta_ns: 16_666_667,
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
            scheduled_events: BTreeMap::new(),
        }
    }
}

impl EngineOwnedTimeState {
    fn snapshot(&self) -> TimeSnapshotV1 {
        let normalized_day = if self.seconds_per_game_day <= f64::EPSILON {
            0.0
        } else {
            (self.seconds_of_day / 86_400.0).rem_euclid(1.0)
        };
        TimeSnapshotV1 {
            schema: TIME_RUNTIME_CONTRACT.to_owned(),
            provider: "EngineOwnedTimeProvider".to_owned(),
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
        self.last_clamped_delta_ns = raw_delta_ns.min(MAX_DELTA_NS);

        let scaled_delta_ns = if self.paused {
            0
        } else {
            ((self.last_clamped_delta_ns as f64) * self.scale.max(0.0)) as u64
        };
        self.accumulator_ns = self.accumulator_ns.saturating_add(scaled_delta_ns).min(self.fixed_delta_ns.saturating_mul(64));
        self.ticks_to_run = if self.fixed_delta_ns == 0 {
            0
        } else {
            (self.accumulator_ns / self.fixed_delta_ns).min(u64::from(MAX_FIXED_TICKS_PER_FRAME)) as u32
        };

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
        log::debug!(
            "time gateway: begin_frame frame={} delta_ns={} clamped_ns={} ticks_to_run={} paused={} scale={:.3}",
            self.frame_index,
            self.last_raw_delta_ns,
            self.last_clamped_delta_ns,
            self.ticks_to_run,
            self.paused,
            self.scale
        );
        self.snapshot()
    }

    fn advance_fixed(&mut self) -> TimeSnapshotV1 {
        if self.accumulator_ns >= self.fixed_delta_ns {
            self.accumulator_ns -= self.fixed_delta_ns;
        } else {
            self.accumulator_ns = 0;
        }
        self.tick = self.tick.wrapping_add(1);
        self.ticks_to_run = if self.fixed_delta_ns == 0 {
            0
        } else {
            (self.accumulator_ns / self.fixed_delta_ns).min(u64::from(MAX_FIXED_TICKS_PER_FRAME)) as u32
        };
        log::debug!(
            "time gateway: advance_fixed tick={} frame={} accumulator_ns={}",
            self.tick,
            self.frame_index,
            self.accumulator_ns
        );
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

fn state() -> Arc<Mutex<EngineOwnedTimeState>> {
    Arc::clone(TIME_GATEWAY.get_or_init(|| Arc::new(Mutex::new(EngineOwnedTimeState::default()))))
}

fn info() -> TimeServiceInfoV1 {
    TimeServiceInfoV1::default()
}

fn invoke(state: &mut EngineOwnedTimeState, payload: Blob) -> RResult<Blob, RString> {
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
        other => RResult::RErr(RString::from(format!("engine.time: unknown invoke method '{other}'"))),
    }
}

fn service() -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        TIME_SERVICE_ID,
        OWNER,
        TIME_BACKEND_CAPABILITY_ID,
        TIME_SERVICE_METHODS.iter().copied(),
    )
    .protocol(TIME_RUNTIME_CONTRACT)
    .features(["frame-clock", "fixed-timestep", "game-clock", "scheduler-clock"])
    .gateway("engine-owned engine.time baseline provider")
    .notes("Owns runtime clock state. Domains consume TimeSnapshotV1 instead of calling Instant::now().");

    JsonServiceRouter::with_shared_state(TIME_SERVICE_ID, state())
        .describe_json(&description)
        .info(info)
        .get_json(time_method::SNAPSHOT_V1, |state| state.snapshot())
        .post_json::<TimeBeginFrameRequestV1, TimeSnapshotV1, _>(time_method::BEGIN_FRAME_V1, |state, request| state.begin_frame(request))
        .get_json(time_method::ADVANCE_FIXED_V1, |state| state.advance_fixed())
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
    register_engine_owned_gateway_service_best_effort(EngineOwnedGatewayDecl {
        gateway: ENGINE_TIME_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Time,
        provider_service: TIME_SERVICE_ID,
        capability: TIME_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: OWNER,
        service: service(),
    })
}
