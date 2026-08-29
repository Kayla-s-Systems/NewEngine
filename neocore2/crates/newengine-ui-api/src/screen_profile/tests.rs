use super::*;

#[test]
fn presentation_flow_gates_bootstrap_and_input_independently() {
    let state = UiPresentationFlowState {
        blocks_world_bootstrap: true,
        blocks_gameplay_input: true,
        ..UiPresentationFlowState::default()
    };
    assert!(!state.allows_world_bootstrap());
    assert!(!state.allows_gameplay_input());
}

#[test]
fn runtime_ready_signal_is_provider_neutral() {
    let mut state = UiPresentationFlowState::default();
    state.mark_runtime_ready(42, "launch gate released");
    assert!(state.runtime_ready);
    assert_eq!(state.frame_index, 42);
    assert_eq!(state.reason, "launch gate released");
}
