# World authority adapter

## Purpose

The world authority adapter makes plugin replaceability observable at the runtime boundary.

`FlecsECS` proves that `engine.ecs` and `engine.entity` can be provided by a plugin. The remaining risk is that scene, physics sync, render extraction and gameplay keep assuming the in-process `newengine_ecs::World` is always the source of truth. The adapter fixes that assumption by publishing the selected gateway authority into the runtime world every frame.

## Current invariant

```text
plugin host gateway registry
  -> engine.ecs active route
  -> engine.entity active route
  -> WorldAuthoritySnapshot
  -> RuntimeWorldAuthorityResource in native World
  -> scene / physics / render / gameplay diagnostics and policy
```

The adapter does not create a hidden mirror. If `engine.ecs` and `engine.entity` are plugin-owned, the native world is treated as a typed component cache / hot-path staging surface until scene and component packet APIs are fully provider-neutral.

## Modes

```text
EngineOwnedWorld
  engine.ecs and engine.entity are engine-owned or missing.

PluginEcsEntityAuthority
  engine.ecs and engine.entity are plugin-owned by the same provider.

SplitAuthority
  ecs/entity are plugin-owned but scene remains owned by a different provider,
  or ECS and entity disagree on owner.
```

## Runtime touch points

```text
newengine-runtime-host::world_authority
  WorldAuthorityClient
  WorldAuthoritySnapshot

newengine-engine-runtime::authority
  RuntimeWorldAuthorityBridge
  RuntimeWorldAuthorityResource
  RuntimeWorldAuthorityFrame

SceneBridge
  owns RuntimeWorldAuthorityBridge
  logs bootstrap boundary

RuntimeRenderController::tick_world_for_render
  publishes RuntimeWorldAuthorityResource before gameplay/physics/render extraction

PhysicsSyncModule / gameplay schedule
  read RuntimeWorldAuthorityResource for policy/diagnostics
```

## Next hardening pass

1. Add provider-neutral component packet API.
2. Move scene load/spawn/write operations through authority commands.
3. Replace `EntityId` in service-facing scene commands with `EntityHandle`.
4. Add render extraction DTOs that do not require direct `World` query access.
5. Add conformance tests for split-authority detection and strict-authority mode.
