# 10 — Physics replaceability record

## Summary

Physics is now replaceable in the same architectural sense as render.

```text
provider plugin -> physics.api service -> host client -> typed adapter -> PhysicsApiRef -> PhysicsSyncModule
```

## Acceptance criteria status

| Requirement | Status |
|---|---:|
| Stable `newengine-physics-api` service crate | yes |
| Provider plugin declares `physics.backend` | yes |
| Provider plugin registers `physics.api` | yes |
| Host-side `PhysicsServiceClient` | yes |
| `PhysicsApiRef` registered in resources | yes |
| Runtime systems avoid provider implementation imports | yes |
| Backend receives packets instead of `&mut World` | yes |
| Static colliders are DTOs | yes |
| Descriptor-first provider discovery | yes |
| Filename-independent identity | yes |
| Conformance/replay tests | not yet |

## Current frame boundary

```text
PhysicsFrameInput
  bodies
  colliders
  commands
  queries

physics.api provider
  owns backend world/resources
  consumes DTOs
  returns DTOs

PhysicsFrameOutput
  pose updates
  velocity updates
  events
  query hits
  step report
```

## Hard invariant

The engine owns the API. Providers own implementation. The engine must not become an adapter over a specific native physics SDK.
