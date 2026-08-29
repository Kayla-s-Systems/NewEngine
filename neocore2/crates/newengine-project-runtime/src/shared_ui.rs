use newengine_project_api::{
    ProjectManifest, ProjectUiInputFocusPolicy, ProjectUiPresentationFlowManifest,
    ProjectUiPresentationStateManifest, ProjectUiPresentationTransitionManifest,
};

pub const SHARED_UI_PAUSE_DOCUMENT_REF: &str = "ui/shared/runtime/pause_menu.neui@surface";
pub const SHARED_UI_HUD_DOCUMENT_REF: &str = "ui/shared/runtime/hud.neui@surface";
pub const SHARED_UI_PAUSE_SURFACE_ID: &str = "shared.ui.pause";
pub const SHARED_UI_HUD_SURFACE_ID: &str = "game.hud";
pub const SHARED_UI_PRIMARY_TOGGLE_ACTION: &str = "engine.ui.primary.toggle";
pub const SHARED_UI_RESUME_ACTION: &str = "game.resume";

fn non_blank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn shared_ui_document_ref(configured: Option<&str>, fallback: &'static str) -> String {
    non_blank(configured).unwrap_or(fallback).to_owned()
}

fn flow_has_state(flow: &ProjectUiPresentationFlowManifest, state_id: &str) -> bool {
    flow.states.iter().any(|state| state.id.trim() == state_id)
}

fn ensure_flow_transition(
    flow: &mut ProjectUiPresentationFlowManifest,
    from: &str,
    to: &str,
    action: &str,
) {
    if flow.transitions.iter().any(|transition| {
        transition.from.trim() == from
            && transition
                .on_action
                .as_deref()
                .is_some_and(|value| value.trim() == action)
    }) {
        return;
    }
    flow.transitions
        .push(ProjectUiPresentationTransitionManifest {
            from: from.to_owned(),
            to: to.to_owned(),
            on_action: Some(action.to_owned()),
            on_runtime_ready: false,
            reset_runtime_ready: false,
        });
}

fn rewrite_state_as_shared_pause(
    flow: &mut ProjectUiPresentationFlowManifest,
    state_id: &str,
    pause_document_ref: &str,
) -> bool {
    let Some(state) = flow
        .states
        .iter_mut()
        .find(|state| state.id.trim() == state_id)
    else {
        return false;
    };
    if state.input_focus_policy == ProjectUiInputFocusPolicy::GameViewport
        && !state.blocks_gameplay_input
    {
        return false;
    }
    state.document_ref = Some(pause_document_ref.to_owned());
    state.surface_id = Some(SHARED_UI_PAUSE_SURFACE_ID.to_owned());
    state.input_focus_policy = ProjectUiInputFocusPolicy::UiSurface;
    state.blocks_world_bootstrap = false;
    state.blocks_gameplay_input = true;
    true
}

fn unique_shared_pause_state_id(
    flow: &ProjectUiPresentationFlowManifest,
    gameplay_state: &str,
) -> String {
    let stem = gameplay_state
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let base = format!("__northstar_shared_pause.{stem}");
    if !flow_has_state(flow, &base) {
        return base;
    }
    for suffix in 2..=1024 {
        let candidate = format!("{base}.{suffix}");
        if !flow_has_state(flow, &candidate) {
            return candidate;
        }
    }
    format!("{base}.overflow")
}

fn augment_flow_with_shared_pause(
    flow: &mut ProjectUiPresentationFlowManifest,
    pause_document_ref: &str,
) {
    let gameplay_states = flow
        .states
        .iter()
        .filter(|state| {
            state.input_focus_policy == ProjectUiInputFocusPolicy::GameViewport
                && !state.blocks_gameplay_input
        })
        .map(|state| state.id.trim().to_owned())
        .filter(|state| !state.is_empty())
        .collect::<Vec<_>>();

    for gameplay_state in gameplay_states {
        let existing_target = flow.transitions.iter().find_map(|transition| {
            (transition.from.trim() == gameplay_state
                && transition
                    .on_action
                    .as_deref()
                    .is_some_and(|value| value.trim() == SHARED_UI_PRIMARY_TOGGLE_ACTION))
            .then(|| transition.to.trim().to_owned())
        });

        if let Some(target) = existing_target {
            if rewrite_state_as_shared_pause(flow, &target, pause_document_ref) {
                ensure_flow_transition(
                    flow,
                    &target,
                    &gameplay_state,
                    SHARED_UI_PRIMARY_TOGGLE_ACTION,
                );
                ensure_flow_transition(flow, &target, &gameplay_state, SHARED_UI_RESUME_ACTION);
            }
            continue;
        }

        let pause_state = unique_shared_pause_state_id(flow, &gameplay_state);
        flow.states.push(ProjectUiPresentationStateManifest {
            id: pause_state.clone(),
            document_ref: Some(pause_document_ref.to_owned()),
            surface_id: Some(SHARED_UI_PAUSE_SURFACE_ID.to_owned()),
            input_focus_policy: ProjectUiInputFocusPolicy::UiSurface,
            blocks_world_bootstrap: false,
            blocks_gameplay_input: true,
        });
        ensure_flow_transition(
            flow,
            &gameplay_state,
            &pause_state,
            SHARED_UI_PRIMARY_TOGGLE_ACTION,
        );
        ensure_flow_transition(
            flow,
            &pause_state,
            &gameplay_state,
            SHARED_UI_PRIMARY_TOGGLE_ACTION,
        );
        ensure_flow_transition(flow, &pause_state, &gameplay_state, SHARED_UI_RESUME_ACTION);
    }
}

pub fn effective_project_ui_presentation_flow(
    manifest: &ProjectManifest,
    requested_initial_state: Option<&str>,
) -> Option<ProjectUiPresentationFlowManifest> {
    let shared = &manifest.ui.shared;
    let mut flow = manifest
        .ui
        .presentation_flow
        .as_ref()
        .filter(|flow| flow.enabled)
        .cloned();

    if shared.enabled {
        let pause_document_ref = shared_ui_document_ref(
            shared.pause_document_ref.as_deref(),
            SHARED_UI_PAUSE_DOCUMENT_REF,
        );
        if let Some(existing) = flow.as_mut() {
            if shared.hud_fallback {
                let hud_document_ref = non_blank(manifest.ui.document.as_deref())
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        shared_ui_document_ref(
                            shared.hud_document_ref.as_deref(),
                            SHARED_UI_HUD_DOCUMENT_REF,
                        )
                    });
                let hud_surface_id = non_blank(manifest.ui.root_surface.as_deref())
                    .unwrap_or(SHARED_UI_HUD_SURFACE_ID)
                    .to_owned();
                for state in existing.states.iter_mut().filter(|state| {
                    state.input_focus_policy == ProjectUiInputFocusPolicy::GameViewport
                        && !state.blocks_gameplay_input
                        && non_blank(state.document_ref.as_deref()).is_none()
                }) {
                    state.document_ref = Some(hud_document_ref.clone());
                    state.surface_id = Some(hud_surface_id.clone());
                }
            }
            if shared.pause_menu {
                augment_flow_with_shared_pause(existing, &pause_document_ref);
            }
        } else {
            let project_document = non_blank(manifest.ui.document.as_deref());
            let needs_shared_hud = project_document.is_none() && shared.hud_fallback;
            if shared.pause_menu || needs_shared_hud {
                let hud_document_ref = project_document.map(str::to_owned).or_else(|| {
                    needs_shared_hud.then(|| {
                        shared_ui_document_ref(
                            shared.hud_document_ref.as_deref(),
                            SHARED_UI_HUD_DOCUMENT_REF,
                        )
                    })
                });
                let hud_surface_id = non_blank(manifest.ui.root_surface.as_deref())
                    .unwrap_or(SHARED_UI_HUD_SURFACE_ID)
                    .to_owned();
                let gameplay_state = ProjectUiPresentationStateManifest {
                    id: "gameplay".to_owned(),
                    document_ref: hud_document_ref.clone(),
                    surface_id: hud_document_ref.as_ref().map(|_| hud_surface_id),
                    input_focus_policy: ProjectUiInputFocusPolicy::GameViewport,
                    blocks_world_bootstrap: false,
                    blocks_gameplay_input: false,
                };
                let mut synthesized = ProjectUiPresentationFlowManifest {
                    enabled: true,
                    id: format!("{}.shared-ui", manifest.id.trim()),
                    initial_state: "gameplay".to_owned(),
                    states: vec![gameplay_state],
                    transitions: Vec::new(),
                };
                if shared.pause_menu {
                    augment_flow_with_shared_pause(&mut synthesized, &pause_document_ref);
                }
                flow = Some(synthesized);
            }
        }
    }

    let mut flow = flow?;
    if let Some(initial_state) = non_blank(requested_initial_state) {
        if flow_has_state(&flow, initial_state) {
            flow.initial_state = initial_state.to_owned();
        }
    }
    Some(flow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_game_project_gets_shared_hud_and_pause_flow() {
        let manifest = ProjectManifest {
            id: "room".to_owned(),
            ..ProjectManifest::default()
        };
        let flow = effective_project_ui_presentation_flow(&manifest, None)
            .expect("default Shared UI flow");
        assert_eq!(flow.initial_state, "gameplay");
        let gameplay = flow
            .states
            .iter()
            .find(|state| state.id == "gameplay")
            .expect("gameplay state");
        assert_eq!(
            gameplay.document_ref.as_deref(),
            Some(SHARED_UI_HUD_DOCUMENT_REF)
        );
        assert_eq!(
            gameplay.surface_id.as_deref(),
            Some(SHARED_UI_HUD_SURFACE_ID)
        );
        let pause = flow
            .states
            .iter()
            .find(|state| state.surface_id.as_deref() == Some(SHARED_UI_PAUSE_SURFACE_ID))
            .expect("shared pause state");
        assert_eq!(
            pause.document_ref.as_deref(),
            Some(SHARED_UI_PAUSE_DOCUMENT_REF)
        );
        assert!(flow.transitions.iter().any(|transition| {
            transition.from == "gameplay"
                && transition.to == pause.id
                && transition.on_action.as_deref() == Some(SHARED_UI_PRIMARY_TOGGLE_ACTION)
        }));
    }

    #[test]
    fn shared_pause_is_injected_into_existing_gameplay_flow() {
        let manifest = ProjectManifest {
            id: "forest-road".to_owned(),
            ui: newengine_project_api::ProjectUiManifest {
                presentation_flow: Some(ProjectUiPresentationFlowManifest {
                    enabled: true,
                    id: "forest-road.frontend".to_owned(),
                    initial_state: "game".to_owned(),
                    states: vec![ProjectUiPresentationStateManifest {
                        id: "game".to_owned(),
                        document_ref: Some("ui/game/game_hud.neui@surface".to_owned()),
                        surface_id: Some("game.hud".to_owned()),
                        input_focus_policy: ProjectUiInputFocusPolicy::GameViewport,
                        blocks_world_bootstrap: false,
                        blocks_gameplay_input: false,
                    }],
                    transitions: Vec::new(),
                }),
                ..newengine_project_api::ProjectUiManifest::default()
            },
            ..ProjectManifest::default()
        };
        let flow = effective_project_ui_presentation_flow(&manifest, None)
            .expect("augmented frontend flow");
        let pause = flow
            .states
            .iter()
            .find(|state| state.surface_id.as_deref() == Some(SHARED_UI_PAUSE_SURFACE_ID))
            .expect("shared pause state");
        assert!(flow.transitions.iter().any(|transition| {
            transition.from == "game"
                && transition.to == pause.id
                && transition.on_action.as_deref() == Some(SHARED_UI_PRIMARY_TOGGLE_ACTION)
        }));
        assert!(flow.transitions.iter().any(|transition| {
            transition.from == pause.id
                && transition.to == "game"
                && transition.on_action.as_deref() == Some(SHARED_UI_RESUME_ACTION)
        }));
    }

    #[test]
    fn existing_gameplay_state_without_document_gets_shared_hud() {
        let manifest = ProjectManifest {
            id: "existing-flow".to_owned(),
            ui: newengine_project_api::ProjectUiManifest {
                presentation_flow: Some(ProjectUiPresentationFlowManifest {
                    enabled: true,
                    id: "existing-flow.frontend".to_owned(),
                    initial_state: "gameplay".to_owned(),
                    states: vec![ProjectUiPresentationStateManifest {
                        id: "gameplay".to_owned(),
                        document_ref: None,
                        surface_id: None,
                        input_focus_policy: ProjectUiInputFocusPolicy::GameViewport,
                        blocks_world_bootstrap: false,
                        blocks_gameplay_input: false,
                    }],
                    transitions: Vec::new(),
                }),
                ..newengine_project_api::ProjectUiManifest::default()
            },
            ..ProjectManifest::default()
        };
        let flow = effective_project_ui_presentation_flow(&manifest, None)
            .expect("augmented frontend flow");
        let gameplay = flow
            .states
            .iter()
            .find(|state| state.id == "gameplay")
            .expect("gameplay state");
        assert_eq!(
            gameplay.document_ref.as_deref(),
            Some(SHARED_UI_HUD_DOCUMENT_REF)
        );
        assert_eq!(
            gameplay.surface_id.as_deref(),
            Some(SHARED_UI_HUD_SURFACE_ID)
        );
    }

    #[test]
    fn existing_escape_pause_state_is_rebound_to_shared_document() {
        let manifest = ProjectManifest {
            id: "game-ready-fps".to_owned(),
            ui: newengine_project_api::ProjectUiManifest {
                presentation_flow: Some(ProjectUiPresentationFlowManifest {
                    enabled: true,
                    id: "game-ready-fps.frontend".to_owned(),
                    initial_state: "gameplay".to_owned(),
                    states: vec![
                        ProjectUiPresentationStateManifest {
                            id: "gameplay".to_owned(),
                            document_ref: Some("ui/game/game_hud.neui@surface".to_owned()),
                            surface_id: Some("game.hud".to_owned()),
                            input_focus_policy: ProjectUiInputFocusPolicy::GameViewport,
                            blocks_world_bootstrap: false,
                            blocks_gameplay_input: false,
                        },
                        ProjectUiPresentationStateManifest {
                            id: "pause".to_owned(),
                            document_ref: Some("ui/frontend/pause_menu.neui@surface".to_owned()),
                            surface_id: Some("game.frontend.pause".to_owned()),
                            input_focus_policy: ProjectUiInputFocusPolicy::UiSurface,
                            blocks_world_bootstrap: false,
                            blocks_gameplay_input: true,
                        },
                    ],
                    transitions: vec![ProjectUiPresentationTransitionManifest {
                        from: "gameplay".to_owned(),
                        to: "pause".to_owned(),
                        on_action: Some(SHARED_UI_PRIMARY_TOGGLE_ACTION.to_owned()),
                        on_runtime_ready: false,
                        reset_runtime_ready: false,
                    }],
                }),
                ..newengine_project_api::ProjectUiManifest::default()
            },
            ..ProjectManifest::default()
        };
        let flow = effective_project_ui_presentation_flow(&manifest, None)
            .expect("rewritten frontend flow");
        let pause = flow
            .states
            .iter()
            .find(|state| state.id == "pause")
            .expect("pause state");
        assert_eq!(
            pause.document_ref.as_deref(),
            Some(SHARED_UI_PAUSE_DOCUMENT_REF)
        );
        assert_eq!(
            pause.surface_id.as_deref(),
            Some(SHARED_UI_PAUSE_SURFACE_ID)
        );
    }

    #[test]
    fn shared_ui_can_be_disabled_per_project() {
        let manifest = ProjectManifest {
            id: "custom-ui".to_owned(),
            ui: newengine_project_api::ProjectUiManifest {
                shared: newengine_project_api::ProjectUiSharedManifest {
                    enabled: false,
                    ..newengine_project_api::ProjectUiSharedManifest::default()
                },
                ..newengine_project_api::ProjectUiManifest::default()
            },
            ..ProjectManifest::default()
        };
        assert!(effective_project_ui_presentation_flow(&manifest, None).is_none());
    }
}
