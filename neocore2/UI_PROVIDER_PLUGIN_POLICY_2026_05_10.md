# NewEngine UI Provider Policy — 2026-05-10

## Rule

The engine/runtime host must not compile a concrete UI backend directly.

Built into engine crates:

- `UiProviderKind::Null`
- `ui.backend = "none"`

Runtime/plugin-owned:

- `ui.backend = "egui"` resolves to plugin service id `newengine.ui.provider.egui`
- custom values resolve as plugin service ids

## Current contract

`newengine-ui` remains the neutral UI contract crate:

- frame descriptor
- input snapshot
- draw list contract
- markup model
- null provider

It does not select egui as a default provider anymore.

## Game/standalone policy

Standalone game builds use `newengine-game-runtime` with:

```toml
newengine-editor-runtime = { default-features = false, features = ["runtime-core"] }
newengine-ui = { default-features = false }
```

This prevents editor UI/egui from being pulled into `game-ready-fps` through the shared runtime profile.

## Plugin build policy

`Plugins/EguiUiProvider` is synchronized by `Plugins/build_all_plugins.cmd` like other runtime plugins. If the plugin is missing, the host logs the missing provider and continues without UI.
