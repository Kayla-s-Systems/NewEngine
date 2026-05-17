# Physics heightfield/mesh and control ownership pass — 2026-05-17

## Goal

Physics terrain/static geometry must cross the backend boundary as stable
`physics.api` DTOs, not as engine world pointers or backend-native handles.
Controlled gameplay bodies must also keep gameplay/input ownership over camera
orientation and lateral motor intent.

## API additions

`newengine-physics-api` now carries formal collider frame packets:

```text
PhysicsFrameInput
  bodies
  colliders
  commands
  queries

PhysicsFrameColliderSnapshot
  entity
  transform
  collider: PhysicsColliderDto

PhysicsColliderDto
  Shape(CollisionShapeDesc)
  Heightfield(HeightfieldColliderDto)
  Mesh(MeshColliderDto)
```

The backend contract remains:

```text
ECS/world data -> PhysicsFrameInput -> physics.api provider -> PhysicsFrameOutput -> ECS apply
```

No backend provider receives `&mut World`, component storage, renderer handles or
native terrain objects.

## Terrain/static geometry extraction

Terrain extraction is now a host-side packet builder concern. The host samples or
serializes terrain/static geometry into backend-neutral collider DTOs:

```text
Procedural/static scene source
  -> HeightfieldColliderDto or MeshColliderDto
  -> PhysicsFrameColliderSnapshot
  -> physics.api
```

Backends are free to map those DTOs to their own native shape representation.
The engine does not know which native shape type a provider will use.

## Controlled body ownership

Gameplay/input owns character intent and camera orientation. The physics provider
owns collision response, ground contact and vertical/gravity integration.

Host-side apply policy preserves the controlled body's yaw/pitch and lateral
motor velocity while accepting provider output for collision-corrected position
and vertical motion.

This prevents the service-backed physics step from erasing mouse look / WASD
authority while still keeping collision and floor contact provider-driven.

## Module split

The engine-side physics sync path is split by responsibility:

```text
PhysicsSyncModule         frame orchestration
physics packet builders   ECS/static scene -> physics.api DTOs
physics output appliers   PhysicsFrameOutput -> ECS mutation policy
```

Provider-native body, shape and query mapping belongs under the concrete plugin
provider, not under engine/runtime/core crates.
