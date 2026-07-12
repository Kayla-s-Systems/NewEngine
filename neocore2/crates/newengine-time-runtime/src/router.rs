use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, JsonServiceRouter,
};
use newengine_time_api::{
    time_method, TimeBeginFrameRequestV1, TimeCancelEventRequestV1, TimeDueEventsV1,
    TimeFixedStepRequestV1, TimeGameClockSetRequestV1, TimePauseRequestV1,
    TimeReplayClockSetRequestV1, TimeScaleRequestV1, TimeScheduledEventV1, TimeServiceInfoV1,
    TimeSnapshotV1, TIME_BACKEND_CAPABILITY_ID, TIME_RUNTIME_CONTRACT, TIME_SERVICE_ID,
    TIME_SERVICE_METHODS,
};

use crate::{
    constants::{OWNER, TIME_FEATURES},
    global::state,
    invoke::invoke,
};

pub(crate) fn service() -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        TIME_SERVICE_ID,
        OWNER,
        TIME_BACKEND_CAPABILITY_ID,
        TIME_SERVICE_METHODS.iter().copied(),
    )
    .protocol(TIME_RUNTIME_CONTRACT)
    .features(TIME_FEATURES.iter().copied())
    .gateway("engine.time.astrolabe baseline provider")
    .notes("Owns runtime clock state. Domains and AI providers consume TimeSnapshotV1/TimeAiContextV1 instead of calling Instant::now().");

    JsonServiceRouter::with_shared_state(TIME_SERVICE_ID, state())
        .describe_json(&description)
        .info(TimeServiceInfoV1::default)
        .get_json(time_method::SNAPSHOT_V1, |state| state.snapshot())
        .get_json(time_method::FRAME_V1, |state| state.snapshot())
        .post_json::<TimeBeginFrameRequestV1, TimeSnapshotV1, _>(
            time_method::BEGIN_FRAME_V1,
            |state, request| state.begin_frame(request),
        )
        .get_json(time_method::ADVANCE_FIXED_V1, |state| state.advance_fixed())
        .get_json(time_method::FIXED_TICK_V1, |state| state.advance_fixed())
        .get_json(time_method::GAME_CLOCK_V1, |state| state.game_clock())
        .post_json::<TimePauseRequestV1, TimeSnapshotV1, _>(
            time_method::PAUSE_DOMAIN_V1,
            |state, request| state.set_pause(request),
        )
        .get_json(time_method::TIMELINE_V1, |state| state.timeline())
        .get_json(time_method::REPLAY_CLOCK_V1, |state| state.replay_clock())
        .post_json::<TimeScaleRequestV1, TimeSnapshotV1, _>(
            time_method::SET_SCALE_V1,
            |state, request| state.set_scale(request),
        )
        .post_json::<TimePauseRequestV1, TimeSnapshotV1, _>(
            time_method::SET_PAUSE_V1,
            |state, request| state.set_pause(request),
        )
        .post_json::<TimeGameClockSetRequestV1, TimeSnapshotV1, _>(
            time_method::SET_GAME_CLOCK_V1,
            |state, request| state.set_game_clock(request),
        )
        .post_json::<TimeFixedStepRequestV1, TimeSnapshotV1, _>(
            time_method::SET_FIXED_STEP_V1,
            |state, request| state.set_fixed_step(request),
        )
        .post_json::<TimeReplayClockSetRequestV1, TimeSnapshotV1, _>(
            time_method::SET_REPLAY_CLOCK_V1,
            |state, request| state.set_replay_clock(request),
        )
        .get_json(time_method::AI_CONTEXT_V1, |state| state.ai_context())
        .post_json::<TimeScheduledEventV1, TimeScheduledEventV1, _>(
            time_method::SCHEDULE_EVENT_V1,
            |state, event| state.schedule_event(event),
        )
        .post_json::<TimeCancelEventRequestV1, TimeDueEventsV1, _>(
            time_method::CANCEL_EVENT_V1,
            |state, request| state.cancel_event(request),
        )
        .get_json(time_method::DUE_EVENTS_V1, |state| state.due_events())
        .get_json(time_method::DESCRIBE_CLOCK_V1, |_state| {
            TimeServiceInfoV1::default()
        })
        .blob(time_method::INVOKE_JSON, invoke)
        .blob(time_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}
