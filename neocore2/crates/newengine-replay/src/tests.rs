use super::*;

#[test]
fn replay_bake_pause_resume_flow_is_valid() {
    let mut fsm = ReplayCoordinatorFsm::new();
    assert!(fsm.activate().valid);
    assert!(fsm.start_bake().valid);
    assert!(fsm.mark_bake_ready().valid);
    assert!(fsm.request_bake_pause().valid);
    assert!(fsm.commit_bake_pause().valid);
    assert!(fsm.resume_bake().valid);
    assert_eq!(fsm.state(), ReplayCoordinatorState::VideoBake);
}

#[test]
fn invalid_transition_forces_faulted_state() {
    let mut fsm = ReplayCoordinatorFsm::new();
    let transition = fsm.mark_bake_ready();
    assert!(!transition.valid);
    assert_eq!(transition.next, ReplayCoordinatorState::Faulted);
}

#[test]
fn playback_clock_applies_jump_before_advance() {
    let mut clock = ReplayPlaybackClock::new();
    clock.jump_to_clip(7, 1200);
    let snapshot = clock.advance_fixed(16);
    assert_eq!(snapshot.clip_index, 7);
    assert_eq!(snapshot.clip_time_ms, 1200);
    assert_eq!(snapshot.frame_index, 1);
}
