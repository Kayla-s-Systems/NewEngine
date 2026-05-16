# Physics heightfield/mesh collider and control ownership pass — 2026-05-17

## Goal

Move terrain collision out of temporary per-body box contact proxies and into the stable `physics.api` packet contract.

The engine remains the host-side extractor/sync layer:

```text
ECS / ProceduralTerrain / controlled player state
  -> PhysicsFrameInput DTO
  -> physics.api provider plugin
  -> PhysicsFrameOutput DTO
  -> ECS apply policy
```

No backend receives `&mut World`.

## API additions

`newengine-physics-api` now includes:

```text
HeightfieldColliderDto
MeshColliderDto
PhysicsColliderDto
PhysicsFrameColliderSnapshot
PhysicsFrameInput.colliders
PhysicsFeature::HeightfieldColliders
PhysicsFeature::MeshColliders
```

`PhysicsFrameInput.bodies` remains for rigid/character bodies. Static large geometry is now represented by `colliders`, not fake physics bodies.

## Runtime-host extraction

`newengine-engine-runtime/src/gameplay/physics.rs` was split into focused modules:

```text
physics.rs                 thin service sync entrypoint
physics/frame_input.rs     ECS -> body snapshots + collider packets
physics/terrain_colliders.rs ProceduralTerrain -> HeightfieldColliderDto packets
physics/frame_output.rs    PhysicsFrameOutput -> ECS with control ownership policy
physics/util.rs            conversion helpers
```

The previous ad-hoc terrain box proxy path was removed.

## Control ownership fix

The previous Jolt sync wrote full pose and full velocity back to every ECS body. It also let the default ECS velocity integrator run before the service backend, causing controlled physics bodies to be integrated once by ECS and again by Jolt. That made the active FPS player lose mouse look and WASD authority because the backend could overwrite character yaw/pitch and lateral velocity.

The new host apply policy is:

```text
uncontrolled bodies:
  physics owns pose + velocity

CharacterMotor bodies:
  physics owns position and vertical velocity
  character motor owns rotation/yaw/pitch and lateral velocity
```

The gameplay schedule now runs Input -> Controllers -> ApplyIntents -> `physics.api` -> Derived. The old in-process `SimStage::Physics` velocity integration is skipped in the service-backed runtime path so the provider is the only physics integrator.

This preserves player control while still letting Jolt resolve gravity/floor contact.

## Jolt mapping

`newengine-physics-jolt` was split so `packet_backend.rs` is no longer responsible for all shape creation.

New modules:

```text
raw.rs      Jolt C type conversion helpers
shapes.rs   body/collider signatures and native Jolt shape creation
```

`JoltPacketPhysicsBackend` now syncs both:

```text
PhysicsFrameBodySnapshot      -> native Jolt rigid body
PhysicsFrameColliderSnapshot  -> native static Jolt collider body
```

Native Jolt shapes now include:

```text
Box / Sphere / Capsule
HeightFieldShape
MeshShape
```

## joltc-sys additions

The vendored `joltc-sys` C ABI now exposes:

```text
JPC_MeshShapeSettings
JPC_MeshShapeSettings_default
JPC_MeshShapeSettings_Create

JPC_HeightFieldShapeSettings
JPC_HeightFieldShapeSettings_default
JPC_HeightFieldShapeSettings_Create
```

This keeps native Jolt details inside the Jolt provider stack. The engine-facing protocol remains DTO-based.

## Remaining work

```text
- contact listener -> PhysicsEventDto contact begin/persist/end
- streaming terrain collider cache/chunking instead of per-controlled-body local heightfield packets
- direct native mesh colliders from scene/prefab static geometry
- provider conformance tests for HeightfieldColliderDto and MeshColliderDto
```
