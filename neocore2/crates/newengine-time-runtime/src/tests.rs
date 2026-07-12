use newengine_time_api::TimeBeginFrameRequestV1;

use crate::state::RuntimeHostedTimeState;

fn request(frame_index: u64, fixed_delta_ns: u64) -> TimeBeginFrameRequestV1 {
    TimeBeginFrameRequestV1 {
        frame_index,
        fixed_delta_ns,
    }
}

#[test]
fn slow_render_frame_schedules_only_the_required_fixed_steps() {
    let mut state = RuntimeHostedTimeState {
        max_fixed_ticks_per_frame: 4,
        ..RuntimeHostedTimeState::default()
    };

    let snapshot = state.begin_frame_with_raw_delta(request(1, 16_000_000), 38_000_000);

    assert_eq!(snapshot.simulation.ticks_to_run, 2);
    assert_eq!(snapshot.simulation.accumulator_ns, 38_000_000);
    assert_eq!(snapshot.real.clamped_delta_ns, 38_000_000);
}

#[test]
fn fixed_advances_preserve_fractional_remainder() {
    let mut state = RuntimeHostedTimeState {
        max_fixed_ticks_per_frame: 4,
        ..RuntimeHostedTimeState::default()
    };
    state.begin_frame_with_raw_delta(request(1, 16_000_000), 38_000_000);

    let first = state.advance_fixed();
    let second = state.advance_fixed();

    assert_eq!(first.simulation.tick, 1);
    assert_eq!(first.simulation.ticks_to_run, 1);
    assert_eq!(first.simulation.accumulator_ns, 22_000_000);
    assert_eq!(second.simulation.tick, 2);
    assert_eq!(second.simulation.ticks_to_run, 0);
    assert_eq!(second.simulation.accumulator_ns, 6_000_000);
}

#[test]
fn pathological_stall_is_capped_to_the_configured_budget() {
    let mut state = RuntimeHostedTimeState {
        max_fixed_ticks_per_frame: 4,
        ..RuntimeHostedTimeState::default()
    };

    let snapshot = state.begin_frame_with_raw_delta(request(120, 16_000_000), 250_000_000);

    assert_eq!(snapshot.simulation.ticks_to_run, 4);
    assert_eq!(snapshot.simulation.accumulator_ns, 64_000_000);
    assert_eq!(snapshot.real.delta_ns, 250_000_000);
    assert_eq!(snapshot.real.clamped_delta_ns, 64_000_000);
}

#[test]
fn large_game_time_jump_uses_constant_time_day_rollover() {
    let mut state = RuntimeHostedTimeState {
        game_time_scale: 2_000_000.0,
        ..RuntimeHostedTimeState::default()
    };

    state.begin_frame_with_raw_delta(request(1, 16_000_000), 64_000_000);

    assert!(state.seconds_of_day < 86_400.0);
    assert!(state.day_index > 0);
}
