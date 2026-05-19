# Pause Menu UI System Pass — 2026-05-19

## Goal

Pause menu is treated as an engine UI surface, not a one-off overlay.

```text
engine.input.bindings -> semantic menu actions
RenderPauseMenuRuntimeState -> declarative UiPauseMenuState
engine.ui provider -> themed draw-list
engine.render -> backdrop blur/postfx + ui composite
```

## Changes

- Shared pause menu layout now lives in `newengine-ui-api` so runtime hit testing and provider drawing use the same geometry.
- Root menu actions are declared as item specs.
- `Exit` is a first-class root item and requests shutdown through the engine lifecycle token.
- Pause state now carries theme and transient message feedback.
- UI provider draws severity-aware accent strips for menu events:
  - info: gold accent;
  - success: green accent;
  - warning: gold alert;
  - danger: red accent.
- Backdrop contrast is stronger and remains full-screen even when native blur is unavailable.

## Invariants

- ESC remains a semantic input action from `engine.input.bindings`.
- Platform root shortcuts do not own pause-menu behavior.
- UI provider draws; runtime owns action side effects.
- Runtime hit-testing and provider visual layout must use the same `pause_menu_layout(...)` helper.
