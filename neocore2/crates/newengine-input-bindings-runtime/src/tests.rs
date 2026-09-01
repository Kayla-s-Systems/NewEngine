use super::*;

fn test_path(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    std::env::temp_dir().join(format!(
        "northstar-{name}-{}-{}.json",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn persistence_roundtrips_canonical_profile() {
    let path = test_path("input-bindings");
    let profile = InputBindingsProfile::empty("test.profile").canonicalized();
    save_profile_to_config(&path, &profile).unwrap();
    let loaded = load_profile_from_config(&path).unwrap();
    assert_eq!(loaded.id, profile.id);
    let _ = std::fs::remove_file(path);
}

#[test]
fn profile_mutation_persists_once_and_returns_snapshot() {
    let path = test_path("input-bindings-mutation");
    let defaults = InputBindingsProfile::empty("defaults").canonicalized();
    let mut state = InputBindingsGatewayState {
        profile: defaults.clone(),
        default_profile: defaults,
        profile_path: path.clone(),
    };
    let result = mutate_profile_state_result(&mut state, |profile| {
        profile.register_action(InputActionDefinition::new("game.test"))
    })
    .unwrap();
    assert!(result.actions.iter().any(|action| action.id == "game.test"));
    assert!(load_profile_from_config(&path).is_some());
    let _ = std::fs::remove_file(path);
}

#[test]
fn acceptance_held_action_is_semantic_down_without_press_edge() {
    let mut frame = InputActionFrame::default();
    let aim = "player.aim".to_owned();
    crate::api::apply_acceptance_held_actions(&mut frame, std::slice::from_ref(&aim));

    assert!(frame.contains_action(&aim));
    let commands = frame.command_actions();
    assert_eq!(commands.held, vec![aim]);
    assert!(commands.pressed.is_empty());
    assert!(commands.released.is_empty());
}
