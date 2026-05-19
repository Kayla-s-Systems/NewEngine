# UI Navigation Menu Runtime

Status: implemented for `engine.pause_menu`.

## Boundary

Pause menu presentation is now data-driven:

```text
assets/ui/menus/engine.pause_menu.menu.json
  -> newengine-ui-navigation-api::MenuDocument
  -> newengine-ui-menu-runtime::MenuRuntime
  -> engine-runtime command router
  -> UiPauseMenuState
  -> engine.ui provider
```

`newengine-ui-menu-runtime` owns navigation state, selected/hovered item tracking, page transitions and route dispatch. It does not execute engine side effects.

Engine side effects are handled by a command router that consumes `MenuActionRoute` values:

```json
{
  "id": "engine.quit",
  "source": "engine.pause_menu.exit",
  "target": "SystemCommand",
  "event": "engine.shutdown.request"
}
```

The runtime does not branch on item ids such as `root.exit` or `settings.reset_bindings`. It branches only on command target/event pairs.

## Crates

```text
newengine-ui-navigation-api
  MenuDocument
  MenuPage
  MenuItem
  MenuActionRoute
  MenuSelectionState
  MenuTransition
  MenuFeedbackEvent

newengine-ui-menu-runtime
  generic state machine
  input navigation
  hover/hit-test contract
  action-route dispatch
```

## Current asset

```text
assets/ui/menus/engine.pause_menu.menu.json
```

It contains root/settings/bindings pages, labels, details, tones, transitions, audio intents and action routes.
