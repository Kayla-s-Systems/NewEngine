use super::*;

#[test]
fn context_stack_canonicalizes_priority_order() {
    let stack = InputContextStack {
        contexts: vec![
            InputContext::new("gameplay", "game").with_priority(10),
            InputContext::new("menu", "ui")
                .with_priority(100)
                .consuming(),
        ],
    }
    .canonicalized();
    assert_eq!(stack.contexts[0].id, "menu");
    assert_eq!(stack.contexts[1].id, "gameplay");
}

#[test]
fn modal_capture_blocks_gameplay_but_keeps_ui_active() {
    let capture = InputCaptureStateV1::modal_ui("inventory", "open");
    assert!(capture.ui_pointer_capture);
    assert!(capture.gameplay_navigation_blocked);
    assert_eq!(capture.owner, "inventory");
}
