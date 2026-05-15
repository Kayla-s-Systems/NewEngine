# Plugin Warning Cleanup Pass — 2026-05-14

## Goal
Clean the warnings reported by the plugin/importer sync build, reduce duplicated build-script code, and keep DLL naming deterministic for runtime plugin discovery.

## Build-log findings
The uploaded `plugin-sync-latest.log` completed successfully but reported warning noise in these areas:

- `platform-winit`
  - unused loading image logical path field;
  - unused overlay frame title;
  - unused subsystem stage detail.
- `vulkan_renderer`
  - public-in-crate methods accepted render-api-private types;
  - one recorded-command replay helper was no longer used.
- `geometryImporter`
  - unused provider `name()` method;
  - non-snake-case library crate target name.
  - unused ICO helper;
  - ignored `embed_resource::CompilationResult` in build script;
  - non-snake-case library crate target name.
- importer build scripts
  - `cargo:warning=Setting DLL output name ...` was used as routine telemetry.

## Applied changes

### Shared build helpers
Runtime plugin build scripts now delegate to:

```text
Plugins/build_support/plugin_cdylib_build.rs
```

Importer build scripts now delegate to:

```text
Importers/build_support/importer_cdylib_build.rs
```

This removes duplicated version-resource/linker logic from every plugin/importer `build.rs` while keeping the runtime DLL convention:

```text
{package-name}-{package-version}-{build-type}.dll
```

### No routine cargo warnings from build scripts
The importer build helper no longer emits `cargo:warning` for normal DLL output naming. The build log should now reserve warnings for actual warnings.

### Platform loading overlay cleanup
`platform-winit` now uses the dynamic overlay title in the rendered heading and folds subsystem stage details into subsystem cards. The unused image `logical_path` payload field was removed from the draw-time BGRA image object.

### Vulkan render API visibility cleanup
`record_cmd` and `enqueue_texture_upload` now use render-api-scoped visibility that matches their private argument types. The unused recorded-phase replay helper was removed.

### Importer cleanup
`geometryImporter` no longer exposes an unused provider `name()` method. `fontImporter` and `geometryImporter` keep their package names for DLL identity, but use snake-case `[lib] name` values to avoid Rust crate-name warnings. `imageImporter` was removed from the runtime pipeline; texture runtime input is NEYTD-only.

## Expected result
The next plugin sync build should have fewer warnings and cleaner plugin logs without changing runtime plugin IDs or installed DLL names.
