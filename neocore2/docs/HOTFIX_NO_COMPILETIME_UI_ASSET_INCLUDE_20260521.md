# Hotfix: no compile-time UI asset include

## Decision

Runtime UI documents are authored assets. Engine runtime code must not load them with
`include_str!`, `include_bytes!`, or relative paths into `assets/`.

The pause menu now loads `ui/menus/engine.pause_menu.menu.json` through
`engine.assets` / AssetManager VFS using the logical path exported by
`newengine-ui-navigation-api`.

## Boundary

- `newengine-engine-runtime` owns runtime state and input routing.
- `engine.assets` owns VFS lookup and package/filesystem delivery.
- `engine.ui` owns presentation/layout rendering.
- `assets/ui/menus/*.menu.json` remains data, not compiled Rust code.

## Rule

No new runtime feature may embed authored assets from `assets/` at compile time.
A missing authored asset is a runtime VFS/content error, not a Rust compilation error.
