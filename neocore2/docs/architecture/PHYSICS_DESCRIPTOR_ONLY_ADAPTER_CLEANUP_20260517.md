# Physics descriptor-only adapter cleanup — 2026-05-17

## Intent

Physics backend discovery must not bind a provider by DLL filename, canonical stem,
plugin folder name, or backend-specific id. Runtime only knows the stable
`physics.api` service and the generic `physics.backend` capability.

## Changes

- Removed dead plugin discovery manifest code that matched DLLs by
  `file_pattern`.
- Removed filename/profile parsing from runtime plugin scan. DLL names are not
  interpreted as dev/release/backend identity during discovery.
- Provider selection is descriptor-first:

```text
DLL -> ABI probe -> PluginDescriptor -> service/capability metadata -> selected provider
```

- Physics backend selection uses only:

```text
provides service physics.api
provides capability physics.backend
backend_priority from capability describe_json
```

- Generic plugin build sync now resolves the actual plugin package inside a
  plugin workspace by inspecting Cargo package metadata and plugin exports. This
  prevents a provider workspace helper crate from being copied as the runtime
  plugin DLL.

## Non-goals

The engine still has a physics sync adapter. That adapter converts ECS state into
`PhysicsFrameInput` and applies `PhysicsFrameOutput`. It does not know whether the
backend is deterministic, null, native, Jolt, Bullet, PhysX, Havok, or anything
else.

## Verification target

Engine-side grep should not contain concrete physics provider ids or crate names.
Runtime logs should show the selected provider only after the descriptor probe
has read the plugin's own metadata.
