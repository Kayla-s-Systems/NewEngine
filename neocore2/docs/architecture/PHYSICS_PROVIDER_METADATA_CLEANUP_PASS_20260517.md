# Physics provider metadata cleanup pass — 2026-05-17

## Goal

Remove stale provider-specific checks from the plugin build path and keep physics
backend discovery metadata outside engine runtime code.

## Changes

- Removed the stale build preflight that expected a provider-specific physics API
  constructor.
- Runtime plugin metadata is now installed from `Plugins/runtime_plugins.manifest.json`
  into the runtime plugin directory as `plugins.manifest.json`.
- Plugin-host still selects physics backends only by generic metadata:

```text
service: physics.api
capability: physics.backend
backend_priority: provider-owned priority
```

## Boundary

Engine crates do not encode concrete physics provider ids, filenames, SDK names or
adapter constructors. Concrete provider metadata lives with the plugin build
pipeline and provider plugin sources.
