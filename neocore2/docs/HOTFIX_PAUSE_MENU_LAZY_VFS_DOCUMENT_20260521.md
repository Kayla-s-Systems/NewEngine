# Hotfix: pause menu MenuDocument lazy VFS load

Date: 2026-05-21

## Problem

The previous fix removed `include_str!` for `engine.pause_menu.menu.json`, but still loaded the
runtime UI document inside `RenderPauseMenuRuntimeState::new()`.

That constructor runs before plugin-owned services are guaranteed to be registered. In that phase
`engine.assets` may not exist yet, so a strict VFS read can panic during app construction.

## Rule

Runtime/authored UI assets must never be embedded into Rust code and must never be eagerly loaded in
constructors that run before service availability.

## Fix

- `RenderPauseMenuRuntimeState::new()` is now side-effect free with respect to VFS and `engine.assets`.
- The pause menu document is loaded lazily through `AssetManager.text_v1` when the menu is opened.
- Load failure is represented as runtime state and UI feedback, not as a panic.
- The authored document remains a VFS asset at `ui/menus/engine.pause_menu.menu.json`.
- No hidden compile-time fallback is introduced.

## Data-driven boundary

```text
render_controller.pause_menu
  -> newengine_ui_navigation_api::ENGINE_PAUSE_MENU_ASSET_PATH
  -> engine.assets / AssetManager VFS
  -> MenuDocument JSON
  -> MenuRuntime
```

The render controller owns presentation/runtime state only. The menu document remains content.
