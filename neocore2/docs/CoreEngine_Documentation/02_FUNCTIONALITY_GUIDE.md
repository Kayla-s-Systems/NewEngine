# Functionality Guide

## Workspace

Main workspace:

```text
NewEngine/neocore2/Cargo.toml
```

Runtime plugins and tooling live outside the main workspace:

```text
Plugins/
Importers/
tools/
```

Engine crates own contracts and routing vocabulary. Provider plugins own implementation.

## Core engine

`newengine-core` provides lifecycle, module ordering, typed `Resources`, startup graph events, declarative runtime contract validation, degradation policy, and plugin-manager integration.

## Plugin system

`newengine-plugin-api` defines the stable ABI. `newengine-plugin-host` performs descriptor-first discovery and metadata-driven gateway routing.

## Gateway service access

Consumer-facing service ids are engine-owned:

```text
engine.assets
engine.render
engine.physics
engine.input
engine.ecs
engine.entity
engine.platform
```

Provider-facing service ids are plugin-owned:

```text
asset_manager.api
render.api
physics.api
vendor-specific equivalents
```

## Render

Render is gateway-routed through `engine.render`. Native GPU ownership belongs to the renderer provider plugin.

## Physics

Physics is gateway-routed through `engine.physics`. `PhysicsSyncModule` translates ECS state into `PhysicsFrameInput`, calls the provider, and applies `PhysicsFrameOutput` back to ECS.

## Assets

Asset access is routed through `engine.assets`. AssetManager owns VFS layers, `.nepak` packages, importer work, and `.neytd` texture dictionary selection.

## ECS and Entity

ECS world-level service access is routed through `engine.ecs` for summaries, snapshots and coarse commands. Entity identity/lifecycle access is routed through `engine.entity` and uses opaque stable handles instead of exposing `EntityId` over service boundaries.

## Platform

Native window/surface data is routed through `engine.platform`. Window snapshot is a gateway method, not a separate platform-window service id.

## Input

Input is gateway-routable through `engine.input`. The next hardening step is a first-class `newengine-input-api` crate with typed envelopes and a host adapter.
