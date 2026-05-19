# Pause Menu UI System Pass — 2026-05-19

## Goal

Pause menu is an engine UI surface backed by a declarative menu document, not a one-off imperative overlay.

```text
assets/ui/menus/engine.pause_menu.menu.json
  -> MenuDocument / MenuPage / MenuItem / MenuActionRoute
  -> newengine-ui-menu-runtime generic state machine
  -> engine-runtime command router
  -> UiPauseMenuState
  -> engine.ui provider draw-list
  -> engine.render backdrop blur/postfx + ui composite
```

## Changes

- Shared pause menu layout still lives in `newengine-ui-api` so runtime hit testing and provider drawing use the same geometry.
- Root/settings/bindings pages now live in `assets/ui/menus/engine.pause_menu.menu.json`.
- Menu actions are `MenuActionRoute` data, not Rust item-id branches.
- `Exit` is declared as:

```json
{
  "id": "engine.quit",
  "source": "engine.pause_menu.exit",
  "target": "SystemCommand",
  "event": "engine.shutdown.request"
}
```

- `newengine-ui-navigation-api` owns document DTOs.
- `newengine-ui-menu-runtime` owns selection, page transitions, hover/hit-test input contract and route dispatch.
- Runtime command routing executes side effects from target/event pairs; UI menu runtime does not request engine shutdown directly.
- UI provider manifests now expose `ui/menus/engine.pause_menu.menu.json` as the pause menu document.
- Pause state still carries theme and transient severity feedback.

## Invariants

- ESC remains a semantic input action from `engine.input.bindings`.
- Platform root shortcuts do not own pause-menu behavior.
- UI provider draws; command routers own side effects.
- `pause_menu.rs` must not branch on item ids such as `root.exit`, `settings.reset_bindings` or `binding:<action>`.
- Runtime hit-testing and provider visual layout must use the same `pause_menu_layout(...)` helper until layout geometry moves fully into provider-owned document data.
