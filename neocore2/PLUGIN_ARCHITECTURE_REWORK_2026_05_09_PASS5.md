# Plugin Architecture Rework — Pass 5

## Problem

The plugin repositories had local copies of engine ABI/API crates:

- `newengine-plugin-api`
- `newengine-platform-api`
- `newengine-render-api`
- `newengine-assets-api`
- `newengine-assets`
- `newengine-ui-draw`
- `newengine-math`

This created ABI drift: one plugin could compile against a stale macro/contract while the host expected the current symbol layout.

The Vulkan renderer failure was exactly this class of bug:

```text
vulkan_renderer-0.3.2-dev.dll: symbol export_plugin_root not found
```

The DLL had `newengine_plugin_signature_v1`, but did not expose the host-facing `export_plugin_root` symbol that `newengine-plugin-host` loads through `libloading`.

## Fix

Plugin sources now depend on the central engine crates through relative path dependencies:

```text
NewEngine/neocore2/crates/newengine-plugin-api
NewEngine/neocore2/crates/newengine-platform-api
NewEngine/neocore2/crates/newengine-render-api
NewEngine/neocore2/crates/newengine-assets-api
NewEngine/neocore2/crates/newengine-assets
NewEngine/neocore2/crates/newengine-ui-draw
NewEngine/neocore2/crates/newengine-math
```

Vendored copies were removed from plugin source workspaces.

## Export policy

`newengine-plugin-api::export_plugin_root!` now emits the exact plain C export expected by the host:

```rust
#[no_mangle]
pub extern "C" fn export_plugin_root() -> PluginRootV1Ref
```

This avoids MinGW/abi_stable builds where only the generated loader symbol is exported.

## Runtime policy

`newengine.renderer.vulkan` remains optional in `plugins/plugins.manifest.json`.

If a stale/broken optional renderer DLL is present and lacks `export_plugin_root`, discovery skips it as an unsupported backend instead of failing engine startup.

## Commands

Check runtime DLL layout:

```cmd
tools\diagnose_plugins.cmd plugins
```

Check plugin source layout:

```cmd
tools\diagnose_plugin_sources.cmd
```

Build plugin sources:

```cmd
cd ..\..\Plugins
build_all_plugins.cmd
```

Sync built DLLs:

```cmd
cd ..\NewEngine\neocore2
tools\sync_runtime_plugins.cmd
```
