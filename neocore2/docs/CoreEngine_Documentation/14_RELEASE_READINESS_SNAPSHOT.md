# 14 — Release readiness snapshot

## What moved closer to release

- Render backend is replaceable through `render.api`.
- Physics backend is replaceable through `physics.api`.
- Plugin discovery is descriptor-first and no longer relies on filenames for provider identity.
- Runtime texture and archive formats are explicit: `.neytd` and `.nepak`.
- Engine-runtime is thinner: it orchestrates APIs and packet sync instead of owning backend implementation.
- ECS service access is routed through `engine.ecs`.
- Entity identity/lifecycle service access is routed through `engine.entity`.
- Platform window/surface data is routed through `engine.platform`.

## Runtime evidence

Healthy launch shape:

```text
plugins loaded: 7
physics provider registered
physics backend service bridge bound
runtime contract ok: physics.api
render backend service bridge bound
runtime contract ok: engine.render
runtime contract ok: engine.ecs
runtime contract ok: engine.entity
runtime contract ok: engine.platform
engine state: running
```

## Release risks still open

- Rust warnings in render internals.
- Provider conformance/replay tests missing.
- Input/UI APIs less formal than render/physics.
- Provider reload/shutdown hardening still incomplete.
- Binary hot path packets not yet implemented for large physics workloads.

## Release posture

The architecture is now much closer to an AAA host/plugin model: the engine is not a renderer wrapper and not a physics SDK wrapper. It is a service host with strict, replaceable backend APIs.
