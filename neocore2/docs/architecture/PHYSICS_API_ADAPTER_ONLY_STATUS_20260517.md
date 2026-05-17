# 05 — Physics provider audit

## Verdict

Physics is now replaceable as a service backend plugin.

The old direct shape:

```text
engine-runtime -> concrete backend world service -> direct ECS mutation
```

has been replaced by:

```text
engine-runtime PhysicsSyncModule
  -> PhysicsFrameInput
  -> physics.api service provider
  -> PhysicsFrameOutput
  -> ECS apply
```

## Current clean boundary

```text
newengine-physics-api
  stable DTO/service protocol

newengine-core::physics
  PhysicsApi trait and PhysicsApiRef

newengine-runtime-host::physics_runtime
  service client + typed adapter + provider resolver

newengine-engine-runtime
  ECS packet extraction/application only

Plugins/*
  backend implementations behind physics.api
```

## What the engine no longer does

- Does not import provider implementation crates.
- Does not store concrete backend service objects in ECS resources.
- Does not pass `&mut World` to backend code.
- Does not know native backend handles.
- Does not pick physics backend from filename.
- Does not branch on concrete physics provider names.

## Packet surface

```text
PhysicsFrameInput
  bodies[]
  colliders[]
  commands[]
  queries[]

PhysicsFrameOutput
  pose_updates[]
  velocity_updates[]
  events[]
  query_hits[]
  report
```

Static world/terrain data is transported as `HeightfieldColliderDto` and `MeshColliderDto`.

## Still needed

- contact event fidelity;
- replay/conformance tests;
- stress tests for streamed collider updates;
- rollback/snapshot protocol if deterministic networking is required;
- binary hot path packet option if JSON frame packets become a measured bottleneck.
