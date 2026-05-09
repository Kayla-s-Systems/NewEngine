# NewEngine runtime plugins

This directory is the runtime plugin deployment root used by `config.json`.

Required runtime plugins:

- `logging-*.dll` — bootstrap logging sink
- `input-*.dll` — input service
- `platform-winit-*.dll` — window/platform runtime
- `assetManager-*.dll` — asset manager service

Optional runtime/tooling plugins:

- `vulkan_renderer-*.dll` — GPU renderer; the engine must fall back to null/headless if absent or invalid
- `importers/*.dll` — asset importers for editor/tooling

Diagnostics:

```cmd
tools\diagnose_plugins.cmd plugins
```

Copy DLLs from the sibling `Plugins/` source checkout:

```cmd
tools\sync_runtime_plugins.cmd
```
