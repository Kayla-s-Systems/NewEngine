# AureliaUiTest

Minimal North Star Engine application that proves the `engine.ui` path with the AureliaUI runtime provider.

Runtime path:

```text
AureliaUiTest surface module
  -> engine.ui / ui.apply_node_request_v1
  -> AureliaUI provider route
  -> ui.draw_frame_bin_v1
  -> UiDrawList
  -> EngineUiDrawListBridgeProvider
  -> engine.render
```

Boot declaration:

```text
RuntimePlugins
PlatformWindow
RenderBackend
UiBackend
```

`PreStartConfigWindow` is intentionally not declared, so the generic runtime-host config/pre-start window is skipped for this app. If an app does not override `RuntimeHostAppProfile::boot_options()`, the runtime host keeps the old full boot behavior and loads everything.

The app intentionally does not draw native UI and does not import the Aurelia provider crate. It relies on the runtime plugin descriptor and gateway registry to bind `engine.ui`.

File layout:

```text
src/main.rs          thin entrypoint
src/app.rs           launch spec
src/options.rs       declared boot options and env defaults
src/profile.rs       engine module composition
src/surface_module.rs runtime UI surface publisher/action consumer
src/ui_document.rs   generated UiNodeTreeRequest
config.json          app-local startup config
```

Run from `NewEngine/neocore2` after plugin sync:

```bat
cargo run -p aurelia-ui-test
```

Required runtime plugins in `pluginsRuntime`:

```text
winit-platform-*-dev.dll
vulkan-renderer-*-dev.dll
aurelia-ui-*-dev.dll
```
